//! Dock'un ayar dosyasına yazdığı alanlar (yalnız ilgili alan; Koy penceresinin kaydettiği öteki ayarlar ezilmez).
//! Karşılığı: `DockFeature.SavePinnedOrder`, `SaveAppIconOverrides`, `SaveItems`.

use koy_core::dock::DockFolder;
use koy_core::dock::items::DockItemEntry;
use koy_core::settings::{AppSettings, SettingsStore, ValueDictionary, ValueList};

fn log(source: &str, e: &std::io::Error) {
    koy_core::diagnostics::CrashLog::write(source, &koy_core::diagnostics::CrashError::exception("IOException", &e.to_string(), ""));
}

/// Sabitli sırayı (ve verildiyse klasörleri) yazar; bellekteki ayar da güncellenir.
pub fn save_pinned_order(order: &[String], folders: Option<&[DockFolder]>, in_memory: Option<&mut AppSettings>) {
    let store = SettingsStore::new(SettingsStore::default_path());
    let mut current = store.load();
    current.dock_items.pinned_order = ValueList::from(order.to_vec());
    if let Some(f) = folders {
        current.dock_items.folders = ValueList::from(f.to_vec());
    }
    if let Err(e) = store.save(&current) {
        log("Dock sırası kaydedilemedi", &e);
    }
    if let Some(s) = in_memory {
        if let Some(f) = folders {
            s.dock_items.folders = ValueList::from(f.to_vec());
        }
        s.dock_items.pinned_order = ValueList::from(order.to_vec());
    }
}

/// Uygulama başına simge değişimlerini yazar. Bellekteki ayar değiştirilmez: dock yeniden kurulurken eski / yeni
/// farkı (DockSettingsDiff) uygulanır.
pub fn save_app_icon_overrides(overrides: ValueDictionary<String>) {
    let store = SettingsStore::new(SettingsStore::default_path());
    let mut current = store.load();
    current.appearance.app_icon_overrides = overrides;
    if let Err(e) = store.save(&current) {
        log("Simge değişimi kaydedilemedi", &e);
    }
}

/// Dock öğe dizisini (ayırıcı, boşluk, saat, çöp kutusu, özel düğme dahil) yazar. Boş liste: dock PinnedOrder'dan türer.
pub fn save_items(items: Vec<DockItemEntry>, in_memory: Option<&mut AppSettings>) {
    let store = SettingsStore::new(SettingsStore::default_path());
    let mut current = store.load();
    current.dock_items.items = ValueList::from(items.clone());
    if let Err(e) = store.save(&current) {
        log("Dock öğeleri kaydedilemedi", &e);
    }
    if let Some(s) = in_memory {
        s.dock_items.items = ValueList::from(items);
    }
}
