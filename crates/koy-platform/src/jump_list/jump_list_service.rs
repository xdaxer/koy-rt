//! Windows görev çubuğundaki uygulama bazlı sağ tık menüsünün (atlama listesi) verisini okur.
//! - Görevler ve uygulamanın kendi kategorileri: `CustomDestinations\<hash>.customDestinations-ms`
//!   (`CustomDestinationsReader`) ve her öğe Windows'un kendi ShellLink'iyle (`IPersistStream.Load`) çözülür.
//! - Son / sık kullanılanlar: belgeli `IApplicationDocumentLists`.
//!
//! Boşta hiçbir şey çalışmaz: izleyici, zamanlayıcı, yoklama, kalıcı iş parçacığı yoktur. Her okuma, bitince biten kısa
//! ömürlü bir STA iş parçacığında yapılır. Sonuçlar iki kaynak dosyanın boyut + yazılma zamanı damgasıyla, en çok
//! `JumpListCache::DEFAULT_CAPACITY` uygulama için önbelleğe alınır. Aynı uygulama için süren okuma paylaşılır.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

use koy_core::jump_list::{
    CustomDestinationsCategoryKind, CustomDestinationsFile, CustomDestinationsReader, ExeAppIdCache, FileStamp, IJumpListService,
    JumpListAppId, JumpListCache, JumpListEntry, JumpListLaunchPlan, JumpListLayout, JumpListLimits, JumpListSection, JumpListSectionKind,
};
use koy_core::text;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Shell::Common::IObjectArray;
use windows::Win32::UI::Shell::{ADLT_FREQUENT, ADLT_RECENT, APPDOCLISTTYPE, ApplicationDocumentLists, IApplicationDocumentLists, IShellItem, IShellLinkW};
use windows::core::HSTRING;

use super::shell_link_entry_reader::ShellLinkEntryReader;
use super::start_menu_app_id_resolver::StartMenuAppIdResolver;
use crate::search::com_apartment::ComApartment;
use crate::search::process_start::{ShellExecuteRequest, shell_execute};
use crate::search::win_env::{expand_environment_variables, last_write_utc_ticks};

type Pending = Arc<(Mutex<Option<Vec<JumpListSection>>>, Condvar)>;

pub struct JumpListService {
    cache: JumpListCache,
    exe_app_ids: ExeAppIdCache,
    // Anahtar büyük harfli AppID (OrdinalIgnoreCase).
    in_flight: Mutex<HashMap<String, Pending>>,
}

impl Default for JumpListService {
    fn default() -> Self {
        JumpListService {
            cache: JumpListCache::new(JumpListCache::DEFAULT_CAPACITY),
            exe_app_ids: ExeAppIdCache::new(ExeAppIdCache::DEFAULT_CAPACITY),
            in_flight: Mutex::new(HashMap::new()),
        }
    }
}

fn recent_root() -> PathBuf {
    PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join(r"Microsoft\Windows\Recent")
}

fn stamp(path: &Path) -> FileStamp {
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() => FileStamp::new(m.len() as i64, last_write_utc_ticks(&m)),
        _ => FileStamp::missing(),
    }
}

/// İşi kısa ömürlü STA iş parçacığında çalıştırır; panik/hata menüyü bozmaz (`on_error`).
fn run_on_sta_thread<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static, on_error: T) -> T {
    std::thread::Builder::new()
        .name("Koy.JumpList".into())
        .spawn(move || {
            let _sta = ComApartment::sta();
            work()
        })
        .ok()
        .and_then(|h| h.join().ok())
        .unwrap_or(on_error)
}

impl IJumpListService for JumpListService {
    fn get(&self, app_user_model_id: &str) -> Vec<JumpListSection> {
        let Some(hash) = JumpListAppId::hash_name(Some(app_user_model_id)) else { return Vec::new() };
        let root = recent_root();
        let custom_path = root.join("CustomDestinations").join(format!("{hash}.customDestinations-ms"));
        let automatic_path = root.join("AutomaticDestinations").join(format!("{hash}.automaticDestinations-ms"));
        let custom = stamp(&custom_path);
        let automatic = stamp(&automatic_path);

        if let Some(cached) = self.cache.try_get(app_user_model_id, custom, automatic) {
            return cached;
        }

        let key = text::to_upper_invariant(app_user_model_id);
        let (pending, owner) = {
            let mut in_flight = self.in_flight.lock().unwrap_or_else(|e| e.into_inner());
            match in_flight.get(&key) {
                Some(p) => (Arc::clone(p), false),
                None => {
                    let p: Pending = Arc::new((Mutex::new(None), Condvar::new()));
                    in_flight.insert(key.clone(), Arc::clone(&p));
                    (p, true)
                }
            }
        };

        if owner {
            let app_id = app_user_model_id.to_string();
            let sections = run_on_sta_thread(move || read(&app_id, &custom_path), None).map(|s| {
                self.cache.set(app_user_model_id, custom, automatic, s.clone());
                s
            });
            let result = sections.unwrap_or_default();
            *pending.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(result.clone());
            pending.1.notify_all();
            // Devam eden işi, bittiğinde (başarısız olsa da) listeden çıkar.
            self.in_flight.lock().unwrap_or_else(|e| e.into_inner()).remove(&key);
            return result;
        }

        let mut slot = pending.0.lock().unwrap_or_else(|e| e.into_inner());
        while slot.is_none() {
            slot = pending.1.wait(slot).unwrap_or_else(|e| e.into_inner());
        }
        slot.clone().unwrap_or_default()
    }

    fn resolve_app_id(&self, exe_path: &str) -> Option<String> {
        if exe_path.trim().is_empty() {
            return None;
        }
        if let Some(cached) = self.exe_app_ids.try_get(exe_path) {
            return cached;
        }
        // Başlat menüsü taraması yalnız sağ tıkta, kısa ömürlü STA iş parçacığında; sonuç (bulunamadı dahil) önbelleğe girer.
        let exe = exe_path.to_string();
        let id = run_on_sta_thread(move || Some(StartMenuAppIdResolver::resolve(&exe)), None)?;
        self.exe_app_ids.set(exe_path, id.clone());
        id
    }

    fn launch(&self, entry: &JumpListEntry, app_exe_path: Option<&str>) {
        let Some(launch) = entry.launch.as_ref() else { return };
        let plan = JumpListLaunchPlan::for_launch(launch, app_exe_path);
        let file = expand_environment_variables(&plan.file);
        let dir = plan.working_directory.filter(|d| !d.is_empty() && Path::new(d).is_dir());
        // Hedef silinmiş / yetki yok / kullanıcı iptali: menü öğesi sessizce işlemsiz kalır.
        let _ = shell_execute(ShellExecuteRequest {
            file: &file,
            arguments: plan.arguments.as_deref().filter(|a| !a.is_empty()),
            working_directory: dir.as_deref(),
            verb: None,
        });
    }

    fn clear(&self) {
        self.cache.clear();
        self.exe_app_ids.clear();
    }
}

/// `None`: okuma hatası (sonuç önbelleğe alınmaz).
fn read(app_id: &str, custom_path: &Path) -> Option<Vec<JumpListSection>> {
    let file = read_custom_destinations(custom_path);
    let mut raw = Vec::new();
    let mut custom_sections = 0;

    for category in &file.categories {
        match category.kind {
            CustomDestinationsCategoryKind::Custom if custom_sections < JumpListLimits::MAX_CUSTOM_SECTIONS => {
                custom_sections += 1;
                raw.push(JumpListSection::new(
                    JumpListSectionKind::Custom,
                    ShellLinkEntryReader::resolve(category.name.as_deref()),
                    resolve_links(&category.items, JumpListLayout::cap_for(JumpListSectionKind::Custom)),
                ));
            }
            CustomDestinationsCategoryKind::Tasks => {
                raw.push(JumpListSection::new(
                    JumpListSectionKind::Tasks,
                    None,
                    resolve_links(&category.items, JumpListLayout::cap_for(JumpListSectionKind::Tasks)),
                ));
            }
            _ => {}
        }
    }

    if JumpListLayout::shows_frequent(Some(&file)) {
        raw.push(JumpListSection::new(JumpListSectionKind::Frequent, None, read_document_list(app_id, ADLT_FREQUENT, JumpListLimits::MAX_FREQUENT)));
    }
    if JumpListLayout::shows_recent(Some(&file)) {
        raw.push(JumpListSection::new(JumpListSectionKind::Recent, None, read_document_list(app_id, ADLT_RECENT, JumpListLimits::MAX_RECENT)));
    }
    Some(JumpListLayout::arrange(&raw))
}

fn read_custom_destinations(path: &Path) -> CustomDestinationsFile {
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() && m.len() <= JumpListLimits::MAX_FILE_BYTES as u64 => {}
        _ => return CustomDestinationsFile::empty(),
    }
    // Uygulama dosyayı yazıyor olabilir: std paylaşımlı açar. Üst sınır yukarıda denetlendi.
    match std::fs::read(path) {
        Ok(bytes) => {
            let n = bytes.len().min(JumpListLimits::MAX_FILE_BYTES as usize);
            CustomDestinationsReader::parse(&bytes[..n])
        }
        Err(_) => CustomDestinationsFile::empty(),
    }
}

/// Kategori öğelerini sırayla çözer; `cap` girdiye ulaşınca kalanını hiç açmaz.
fn resolve_links(items: &[Vec<u8>], cap: i32) -> Vec<JumpListEntry> {
    let mut entries = Vec::new();
    let mut count = 0;
    for item in items {
        if count >= cap {
            break;
        }
        let Some(entry) = ShellLinkEntryReader::from_stream(item) else { continue };
        if !entry.is_separator() {
            count += 1;
        }
        entries.push(entry);
    }
    entries
}

/// IApplicationDocumentLists ile bir AppID'nin son ya da sık kullanılanları.
fn read_document_list(app_id: &str, kind: APPDOCLISTTYPE, max: i32) -> Vec<JumpListEntry> {
    let mut entries = Vec::new();
    // SAFETY: STA'da; liste yok / AppID tanınmıyorsa bölüm boş kalır.
    unsafe {
        let Ok(lists) = CoCreateInstance::<_, IApplicationDocumentLists>(&ApplicationDocumentLists, None, CLSCTX_INPROC_SERVER) else {
            return entries;
        };
        if lists.SetAppID(&HSTRING::from(app_id)).is_err() {
            return entries;
        }
        let Ok(objects) = lists.GetList::<IObjectArray>(kind, max as u32) else { return entries };
        let count = objects.GetCount().unwrap_or(0);
        for i in 0..count.min(max as u32) {
            if let Some(entry) = read_document_entry(&objects, i) {
                entries.push(entry);
            }
        }
    }
    entries
}

fn read_document_entry(objects: &IObjectArray, index: u32) -> Option<JumpListEntry> {
    // SAFETY: STA'da; öğeler IShellItem olarak gelir, kısayol (IShellLink) gelirse onu da çözeriz.
    unsafe {
        if let Ok(item) = objects.GetAt::<IShellItem>(index) {
            return ShellLinkEntryReader::from_shell_item(&item);
        }
        let link = objects.GetAt::<IShellLinkW>(index).ok()?;
        // Kısayol olarak gelen son kullanılan, olduğu gibi çalışır.
        ShellLinkEntryReader::from_link(&link).filter(|e| !e.is_separator())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_app_returns_empty_and_resolve_handles_missing_exe() {
        let service = JumpListService::default();
        let sections = service.get("Koy.Test.DoesNotExist.App");
        assert!(sections.iter().all(|s| s.entries.is_empty()));
        assert_eq!(service.resolve_app_id(r"C:\nope\missing.exe"), None);
        assert_eq!(service.resolve_app_id("  "), None);
    }

    #[test]
    fn resolves_indirect_strings_and_passthrough() {
        assert_eq!(ShellLinkEntryReader::resolve(Some("Düz başlık")).as_deref(), Some("Düz başlık"));
        let s = ShellLinkEntryReader::resolve(Some("@%SystemRoot%\\system32\\shell32.dll,-21769"));
        assert!(s.is_some_and(|t| !t.is_empty()));
    }
}
