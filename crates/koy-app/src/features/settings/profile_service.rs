//! Karşılığı: `koy/src/App/Features/Settings/ProfileService.cs`.
//!
//! Kayıtlı profillerin uygulama tarafı girişi: listeler, kaydeder, içe/dışa aktarır ve profili uygular. Mantığın hepsi
//! Core'daki `ProfileLibrary` ve `SettingsProfileCodec` içindedir; burası dosyaları ve çalışan özellikleri bağlar.
//! Ayarlar penceresinden bağımsızdır: bağlam kuralları da aynı `apply` çağrısını kullanır. Hepsi arayüz iş
//! parçacığında çağrılmalıdır (uygulama çalışan çentik ve dock'u yeniden kurar).

use std::cell::RefCell;
use std::rc::Rc;

use koy_core::settings::{
    AppSettings, ImportedProfile, ProfileApplyResult, ProfileLibrary, ProfileLibraryError, ProfileLibraryResult, ProfileSection,
    ProfileStore, SavedProfile, SettingsStore,
};

use crate::infra::observable::{LocalEvent, Notifier};
use crate::resources::theme_manager::ThemeManager;

pub struct ProfileService {
    settings: SettingsStore,
    profiles: ProfileStore,
    /// Ayar kaydedildikten sonra çalışan özelliklerin yeniden kurulması (App.ReloadAllFeatures ve anahtar eşitleme).
    reload_running: RefCell<Option<Box<dyn Fn(&AppSettings)>>>,
    /// Bir profil uygulandıktan sonra (açık Koy penceresi buradan yenilenir).
    pub applied: LocalEvent<SavedProfile>,
    /// Başka bir kaynağın (eklenti mağazası) ayarı doğrudan değiştirdiği an.
    pub settings_committed: Notifier,
}

thread_local! {
    static INSTANCE: RefCell<Option<Rc<ProfileService>>> = const { RefCell::new(None) };
}

impl ProfileService {
    pub fn new(settings: SettingsStore, profiles: ProfileStore) -> Rc<Self> {
        Rc::new(ProfileService {
            settings,
            profiles,
            reload_running: RefCell::new(None),
            applied: LocalEvent::default(),
            settings_committed: Notifier::default(),
        })
    }

    /// Uygulama genelindeki tek örnek (C# `ProfileService.Instance`).
    pub fn instance() -> Rc<ProfileService> {
        INSTANCE.with(|i| {
            i.borrow_mut()
                .get_or_insert_with(|| {
                    ProfileService::new(SettingsStore::new(SettingsStore::default_path()), ProfileStore::new(ProfileStore::default_path()))
                })
                .clone()
        })
    }

    pub fn set_reload_running(&self, f: impl Fn(&AppSettings) + 'static) {
        *self.reload_running.borrow_mut() = Some(Box::new(f));
    }

    pub fn list(&self) -> Vec<SavedProfile> {
        self.profiles.load()
    }

    /// Diskteki geçerli ayarlar (dışa aktarma ve içe aktarma önizlemesi için).
    pub fn current_settings(&self) -> AppSettings {
        self.settings.load()
    }

    /// Kimliğe ya da ada göre kayıtlı profil.
    pub fn find(&self, id_or_name: Option<&str>) -> Option<SavedProfile> {
        ProfileLibrary::find(&self.list(), id_or_name).cloned()
    }

    pub fn create_from_current(&self, name: &str, sections: &[ProfileSection]) -> ProfileLibraryResult {
        self.persist(ProfileLibrary::create(&self.list(), &self.settings.load(), name, sections, &new_id()))
    }

    pub fn add_imported(&self, imported: &ImportedProfile) -> ProfileLibraryResult {
        self.persist(ProfileLibrary::add_imported(&self.list(), imported, &new_id()))
    }

    pub fn rename(&self, id: &str, new_name: &str) -> ProfileLibraryResult {
        self.persist(ProfileLibrary::rename(&self.list(), id, new_name))
    }

    pub fn duplicate(&self, id: &str) -> ProfileLibraryResult {
        self.persist(ProfileLibrary::duplicate(&self.list(), id, &new_id()))
    }

    pub fn delete(&self, id: &str) -> ProfileLibraryResult {
        self.persist(ProfileLibrary::delete(&self.list(), id))
    }

    /// Profili uygular: ayar dosyası kaydedilir, tema yenilenir, çentik ve dock yeniden kurulur. Profil zaten geçerli
    /// ayarlarla aynıysa hiçbir şey yeniden kurulmaz.
    pub fn apply(&self, id_or_name: Option<&str>) -> ProfileApplyResult {
        self.apply_from(&self.list(), id_or_name)
    }

    /// Geçerli ayarların verilen bölümler için kayıtsız anlık görüntüsü (bağlam kuralları için).
    pub fn snapshot(&self, sections: &[ProfileSection]) -> Option<SavedProfile> {
        let result = ProfileLibrary::create(&[], &self.settings.load(), "snapshot", sections, &new_id());
        if result.is_success() { result.profile } else { None }
    }

    /// Kütüphanede olmayan bir profili (`snapshot`) aynı yoldan uygular.
    pub fn apply_unsaved(&self, profile: &SavedProfile) -> ProfileApplyResult {
        self.apply_from(std::slice::from_ref(profile), Some(&profile.id))
    }

    fn apply_from(&self, library: &[SavedProfile], id_or_name: Option<&str>) -> ProfileApplyResult {
        let result = ProfileLibrary::apply(library, &self.settings.load(), id_or_name);
        if !result.is_success() || result.changed_sections.is_empty() {
            return result;
        }
        if let Some(s) = &result.settings {
            let _ = self.settings.save(s);
        }
        self.reload_running();
        if let Some(p) = &result.profile {
            self.applied.invoke(p);
        }
        result
    }

    /// Hazır bir ayar nesnesini kaydeder ve profil uygulamayla aynı yoldan canlı uygular. Ayar değişmediyse özellikler
    /// yeniden kurulmaz; kullanıcı stil dosyası ise (eklenti teması) yine baştan okunur.
    pub fn commit_settings(&self, next: &AppSettings) -> bool {
        let unchanged = self.settings.load() == *next;
        if !unchanged {
            if self.settings.save(next).is_err() {
                return false;
            }
            self.reload_running();
        }
        // Dosya ayardan bağımsız değişmiş olabilir (yeni tema dosyası, geri yüklenen yedek).
        ThemeManager::reload_user_style();
        if !unchanged {
            self.settings_committed.notify();
        }
        true
    }

    fn reload_running(&self) {
        let saved = self.settings.load();
        ThemeManager::apply(&saved);
        if let Some(f) = self.reload_running.borrow().as_ref() {
            f(&saved);
        }
    }

    fn persist(&self, mut result: ProfileLibraryResult) -> ProfileLibraryResult {
        if !result.is_success() {
            return result;
        }
        if !self.profiles.save(&result.profiles) {
            result.error = ProfileLibraryError::SaveFailed;
        }
        result
    }
}

/// `Guid.NewGuid().ToString("N")`: 32 küçük onaltılık hane (rastgele baytlar).
fn new_id() -> String {
    let bytes = koy_core::random::get_bytes(16);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
