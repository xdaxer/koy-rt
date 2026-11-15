//! `koy/tests/Core.Tests` altındaki Settings testleri (+ ayara bağlı Appearance/Media/Voice testleri).
//!
//! Atlananlar:
//! - `Equals_HandlesNullListsGracefully` (SettingsStoreTests): Rust listeleri null olamaz.
//! - `Export_rejects_an_undefined_section`, `Create_rejects_no_sections_and_undefined_sections` (undefined kısmı):
//!   `(ProfileSection)99` Rust'ta temsil edilemez; boş liste kısmı taşındı.
//! - GetHashCode karşılaştırmaları: Rust ayar kayıtları `Hash` uygulamaz (f64 alanlar); eşitlik testleri korundu.
//! - `Lookup_ignores_a_null_stored_value` (AppIconOverrideTests): sözlük değeri `String`, null olamaz.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::settings::{AppSettings, SettingsStore};

mod advanced_settings_tests;
mod app_icon_override_settings_tests;
mod app_language_tests;
mod app_settings_equality_tests;
mod apply_revert_policy_tests;
mod clock_units_settings_tests;
mod feature_state_sync_tests;
mod first_run_policy_tests;
mod font_scale_settings_tests;
mod golden_v3_to_v4_tests;
mod legacy_value_migration_tests;
mod motion_settings_tests;
mod music_voice_setting_tests;
mod profile_library_tests;
mod settings_changes_tests;
mod settings_key_coverage_tests;
mod settings_live_reload_tests;
mod settings_migration_tests;
mod settings_page_reset_tests;
mod settings_profile_codec_tests;
mod settings_search_tests;
mod settings_store_robustness_tests;
mod settings_store_tests;
mod setup_wizard_logic_tests;
mod value_collections_tests;

/// Altın dosya: v0.2.6'nın yazdığı biçimde, her alanı varsayılandan farklı v3 ayarları.
pub(crate) const FIXTURE_V3: &str = include_str!("fixtures/settings-v3.json");

/// Testlerin geçici klasörü; düşünce silinir (C# `IDisposable` testlerindeki `Directory.Delete`).
pub(crate) struct TempDir(PathBuf);

impl TempDir {
    /// Klasör oluşturulmuş olarak döner.
    pub(crate) fn new(prefix: &str) -> TempDir {
        let dir = TempDir::unique(prefix);
        std::fs::create_dir_all(&dir.0).unwrap();
        dir
    }

    /// Yalnız yol üretir, klasörü oluşturmaz (C# `Path.Combine(GetTempPath(), prefix + Guid)`).
    pub(crate) fn unique(prefix: &str) -> TempDir {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        TempDir(std::env::temp_dir().join(format!("{prefix}{}-{nanos}-{n}", std::process::id())))
    }

    pub(crate) fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `SettingsFile.Load`: ham JSON'u gerçek `SettingsStore` yolundan (göç dahil) okur.
pub(crate) fn load_json(json: &str) -> AppSettings {
    let dir = TempDir::new("koy-sf-");
    let path = dir.file("settings.json");
    std::fs::write(&path, json).unwrap();
    SettingsStore::new(&path).load()
}
