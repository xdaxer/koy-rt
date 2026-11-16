//! Kullanıcının görev çubuğuna sabitlediği uygulamaların kısayollarını (.lnk) ve Mağaza (UWP / MSIX) pinlerini
//! (`Taskband\Favorites` AUMID'leri) okur ve çözümler.

use std::collections::HashSet;
use std::path::PathBuf;

use koy_core::dock::{IPinnedAppsService, PinnedApp, TaskbandPackagedPins};
use koy_core::{text, win_path};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree, IPersistFile, STGM};
use windows::Win32::UI::Shell::{IShellItem, IShellLinkW, SHCreateItemFromParsingName, SIGDN_NORMALDISPLAY, ShellLink};
use windows::core::{HSTRING, Interface};

use super::app_user_model_id_reader::AppUserModelIdReader;
use super::icon_helper::IconHelper;
use super::small::TaskbandFavoritesReader;
use crate::search::com_apartment::ComApartment;

fn taskbar_pinned_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join(r"Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar")
}

#[derive(Default)]
pub struct PinnedAppsService;

/// Kısayolun hedef yolu ve AppUserModelID'si. File Explorer gibi kabuk öğesi kısayollarında hedef boştur, AUMID dolu gelir.
fn resolve_shortcut(shortcut_path: &str) -> (Option<String>, Option<String>) {
    // SAFETY: COM nesneleri Drop ile bırakılır; arabellek boyutu verilir.
    unsafe {
        let Ok(link) = CoCreateInstance::<_, IShellLinkW>(&ShellLink, None, CLSCTX_INPROC_SERVER) else { return (None, None) };
        let Ok(persist) = link.cast::<IPersistFile>() else { return (None, None) };
        if persist.Load(&HSTRING::from(shortcut_path), STGM(0)).is_err() {
            return (None, None);
        }
        let mut buf = [0u16; 260];
        if link.GetPath(&mut buf, std::ptr::null_mut(), 0).is_err() {
            return (None, None);
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let target = String::from_utf16_lossy(&buf[..len]);
        let aumid = link.cast().ok().and_then(|unk| AppUserModelIdReader::for_shell_link(&unk));
        ((!target.trim().is_empty()).then_some(target), aumid)
    }
}

/// `shell:AppsFolder\<AUMID>` öğesinin görünen adı; uygulama kurulu değilse ya da öğe bulunamazsa `None`.
fn resolve_shell_item_display_name(shell_path: &str) -> Option<String> {
    // SAFETY: COM çağrıları; dönen dize CoTaskMemFree ile bırakılır.
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(shell_path), None).ok()?;
        let value = item.GetDisplayName(SIGDN_NORMALDISPLAY).ok()?;
        let name = value.to_string().ok();
        CoTaskMemFree(Some(value.0 as *const _));
        name.filter(|n| !n.trim().is_empty())
    }
}

fn read_packaged_pins(result: &mut Vec<PinnedApp>, seen: &mut HashSet<String>) {
    let favorites = TaskbandFavoritesReader::read();
    for aumid in TaskbandPackagedPins::extract_aumids(favorites.as_deref()) {
        if !seen.insert(text::to_upper_invariant(&aumid)) {
            continue;
        }
        let shell_path = TaskbandPackagedPins::apps_folder_path(&aumid);
        let Some(title) = resolve_shell_item_display_name(&shell_path) else { continue };
        let icon = IconHelper::get_icon_bytes(&shell_path);
        result.push(PinnedApp::new(title, shell_path.clone(), Some(shell_path), icon, Some(aumid)));
    }
}

impl IPinnedAppsService for PinnedAppsService {
    fn get_pinned_apps(&self) -> Vec<PinnedApp> {
        // Kabuk nesneleri COM ister; çağıran iş parçacığının kipinden bağımsız olsun diye kısa ömürlü STA'da çalışır.
        std::thread::spawn(|| {
            let _com = ComApartment::sta();
            let mut result = Vec::new();
            // Anahtar büyük harfli AUMID (OrdinalIgnoreCase).
            let mut seen = HashSet::new();
            let dir = taskbar_pinned_dir();
            // Klasör okuma ya da erişim hatasında kısmi listeyle devam edilir.
            if let Ok(read) = std::fs::read_dir(&dir) {
                let mut files: Vec<PathBuf> = read
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")))
                    .collect();
                files.sort();
                for file in files {
                    let file = file.to_string_lossy().into_owned();
                    let title = win_path::get_file_name_without_extension(&file).to_string();
                    let (target, aumid) = resolve_shortcut(&file);
                    if let Some(a) = aumid.as_deref().filter(|a| !a.is_empty()) {
                        seen.insert(text::to_upper_invariant(a));
                    }
                    let icon = IconHelper::get_icon_bytes(&file)
                        .or_else(|| target.as_deref().filter(|t| !t.is_empty()).and_then(IconHelper::get_icon_bytes));
                    result.push(PinnedApp::new(title, file, target, icon, aumid));
                }
            }
            read_packaged_pins(&mut result, &mut seen);
            result
        })
        .join()
        .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_pins_without_failing() {
        let apps = PinnedAppsService.get_pinned_apps();
        assert!(apps.iter().all(|a| !a.title.is_empty()));
    }
}
