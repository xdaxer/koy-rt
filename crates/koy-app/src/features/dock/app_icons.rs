//! Dock simgeleri: uygulama başına simge değişimi, simge paketi klasörü ve Başlat düğmesinin simgesi.
//! Karşılığı: `DockAppIcons.cs` ve `DockStartIconLoader.cs`.

use std::collections::HashMap;
use std::path::Path;

use koy_core::appearance::{AppIconKey, AppIconOverrides, IconChoice, IconOverrideMap, IconPack, IconPackFiles, IconResolver, IconSourceKind};
use koy_core::dock::{DockItem, DockStartIcon, DockStartIconResolver};
use koy_core::settings::{AppSettings, ValueDictionary};

use crate::infra::image_decode;

// Dev bir klasör belleği şişirmesin: ilk bu kadar dosya adı listelenir.
const MAX_PACK_FILES: usize = 5000;
// Düğme en çok 18 DIP; %350 DPI'a kadar keskin kalsın diye 64 px yeter.
const DECODE_PIXELS: u32 = 64;
// Bu sınırın üstündeki dosya simge olmaz; bellekte açılmadan reddedilir.
const MAX_FILE_BYTES: u64 = 20 * 1024 * 1024;

/// Başlat düğmesinin simgesi: yerleşik Koy ya da Windows logosu (Slint'te çizilir) veya kullanıcının dosyası.
#[derive(Clone)]
pub enum StartIcon {
    Koy,
    Windows,
    Custom(slint::Image),
}

/// Başlat simgesini ayardan çözer. Özel dosya okunamazsa Koy simgesine düşülür.
pub fn load_start_icon(settings: &AppSettings) -> StartIcon {
    let path = settings.dock.start_icon_path.trim();
    let exists = |p: &str| Path::new(p).is_file();
    let source = DockStartIconResolver::resolve(settings.dock.start_icon, Some(path).filter(|p| !p.is_empty()), Some(&exists));
    if source.kind == DockStartIcon::Custom
        && let Some(p) = source.path.as_deref()
        && let Some(img) = load_file(p)
    {
        return StartIcon::Custom(img);
    }
    if source.kind == DockStartIcon::Windows { StartIcon::Windows } else { StartIcon::Koy }
}

/// Simge dosyasını (PNG, JPG, ICO) 64 px'e çözer. Boş, çok büyük ya da bozuk dosya `None`.
pub fn load_file(path: &str) -> Option<slint::Image> {
    let len = std::fs::metadata(path).ok()?.len();
    if len == 0 || len > MAX_FILE_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    image_decode::decode(&bytes, Some(DECODE_PIXELS)).map(|b| b.to_image())
}

/// Simge paketi ve uygulama başına değişim. Ayar değişince dock yeniden kurulur, örnek de yenilenir.
pub struct DockAppIcons {
    pack: IconPack,
    folder: String,
    overrides: ValueDictionary<String>,
    loaded: HashMap<String, Option<slint::Image>>,
    pack_files: Option<Vec<String>>,
}

impl DockAppIcons {
    pub fn new(settings: &AppSettings) -> Self {
        let folder = settings.appearance.icon_pack_folder.trim();
        DockAppIcons {
            pack: settings.appearance.icon_pack,
            folder: if IconPackFiles::is_safe_folder(Some(folder)) { folder.to_string() } else { String::new() },
            overrides: settings.appearance.app_icon_overrides.clone(),
            loaded: HashMap::new(),
            pack_files: None,
        }
    }

    pub fn is_active(&self) -> bool {
        (self.pack == IconPack::Folder && !self.folder.is_empty()) || self.overrides.len() > 0
    }

    fn keys(item: &DockItem) -> Vec<String> {
        AppIconKey::all(item.app_user_model_id.as_deref(), Some(&item.id))
    }

    pub fn has_override(&self, item: &DockItem) -> bool {
        Self::keys(item).iter().any(|k| AppIconOverrides::lookup(Some(&self.overrides), Some(k)).is_some())
    }

    pub fn resolve(&mut self, item: &DockItem) -> Option<slint::Image> {
        if !self.is_active() {
            return None;
        }
        let choice = self.choose(&Self::keys(item));
        let path = match choice.kind {
            IconSourceKind::AppOverride => self.to_full_path(choice.value.as_deref()),
            IconSourceKind::Pack if !self.folder.is_empty() => {
                choice.value.as_deref().map(|n| Path::new(&self.folder).join(n).to_string_lossy().into_owned())
            }
            _ => None,
        }?;
        if !is_raster_file(&path) {
            return None;
        }
        self.loaded.entry(path.to_lowercase()).or_insert_with(|| load_file(&path)).clone()
    }

    // Önce herhangi bir anahtarın kendi değişimi, sonra paket; ikisi de yoksa yerleşik.
    fn choose(&mut self, keys: &[String]) -> IconChoice {
        for key in keys {
            let overridden = IconResolver::resolve(Some(key), Some(&self.overrides as &dyn IconOverrideMap), IconPack::Builtin, None);
            if overridden.kind == IconSourceKind::AppOverride {
                return overridden;
            }
        }
        if self.pack != IconPack::Folder || self.folder.is_empty() {
            return IconChoice::new(IconSourceKind::Default, None);
        }
        let files = self.pack_files().to_vec();
        for key in keys {
            let pack_icon = IconPackFiles::find(files.iter(), Some(key), false);
            let choice = IconResolver::resolve(Some(key), None, self.pack, pack_icon.as_deref());
            if choice.kind == IconSourceKind::Pack {
                return choice;
            }
        }
        IconChoice::new(IconSourceKind::Default, None)
    }

    // Değer yalnız dosya adıysa paket klasörüne göre çözülür (Core klasör bilmez).
    fn to_full_path(&self, value: Option<&str>) -> Option<String> {
        let value = value?;
        if Path::new(value).is_absolute() {
            return Some(value.to_string());
        }
        (!self.folder.is_empty()).then(|| Path::new(&self.folder).join(value).to_string_lossy().into_owned())
    }

    fn pack_files(&mut self) -> &[String] {
        if self.pack_files.is_none() {
            // Klasör silinmiş, ağ yolu düşmüş ya da erişim yok: paket boş sayılır.
            let files = std::fs::read_dir(&self.folder)
                .map(|it| {
                    it.filter_map(Result::ok)
                        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                        .filter_map(|e| e.file_name().to_str().map(str::to_string))
                        .take(MAX_PACK_FILES)
                        .collect()
                })
                .unwrap_or_default();
            self.pack_files = Some(files);
        }
        self.pack_files.as_deref().unwrap_or(&[])
    }
}

// Yalnız PNG / ICO (seçici de yalnız bunları sunar; SVG yok).
fn is_raster_file(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("png") || e.eq_ignore_ascii_case("ico"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_files_are_png_or_ico() {
        assert!(is_raster_file(r"C:\a\b.PNG"));
        assert!(is_raster_file("x.ico"));
        assert!(!is_raster_file("x.svg"));
        assert!(!is_raster_file("x"));
    }

    #[test]
    fn inactive_without_pack_or_overrides() {
        let icons = DockAppIcons::new(&AppSettings::default());
        assert!(!icons.is_active());
    }
}
