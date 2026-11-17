//! Karşılığı: `koy/src/App/Features/Settings/ProfilesViewModel.cs`.
//!
//! Profiller bölümü (Genel sayfası): geçerli ayarlardan profil kaydet, kayıtlı profili uygula / yeniden adlandır /
//! çoğalt / sil, dosyaya dışa aktar, dosyadan içe aktar (önizleme ve onayla). Karar mantığı Core'dadır; bu sınıf
//! yalnız dosya pencereleri, metin ve durum. İçe aktarılan dosya güvenilmez girdidir: doğrulama Core'da.

use std::cell::RefCell;
use std::rc::Rc;

use koy_core::settings::{ImportedProfile, ProfileLibrary, ProfileLibraryError, ProfileSection, SavedProfile, SettingsProfileCodec};

use crate::infra::observable::{LocalEvent, Notifier};
use crate::resources::strings::Strings;

use super::profile_service::ProfileService;

const FILE_EXTENSION: &str = ".koyprofile.json";

#[derive(Clone, Debug)]
pub struct ProfileItem {
    pub id: String,
    pub name: String,
    pub sections_text: String,
    pub is_renaming: bool,
    pub edit_name: String,
    pub is_delete_confirming: bool,
}

#[derive(Clone, Debug)]
pub struct SectionOption {
    pub section: ProfileSection,
    pub is_selected: bool,
}

#[derive(Default)]
struct State {
    items: Vec<ProfileItem>,
    sections: Vec<SectionOption>,
    new_name: String,
    status_text: String,
    status_is_error: bool,
    pending: Option<ImportedProfile>,
    pending_name: String,
    pending_sections_text: String,
    pending_changes_text: String,
}

pub struct ProfilesViewModel {
    service: Rc<ProfileService>,
    st: RefCell<State>,
    pub changed: Notifier,
    /// Liste yeniden okundu: profil seçen başka bölümler (kurallar) yenilenir.
    pub list_changed: LocalEvent<()>,
}

pub fn describe_sections(sections: &[ProfileSection]) -> String {
    let names: Vec<String> = sections.iter().map(|s| Strings::get(&format!("ProfileSection{}", s.name()))).collect();
    if names.is_empty() { Strings::get("ProfileCorrupt") } else { names.join(", ") }
}

pub fn section_label(section: ProfileSection) -> String {
    Strings::get(&format!("ProfileSection{}", section.name()))
}

impl ProfilesViewModel {
    pub fn new(service: Rc<ProfileService>) -> Rc<Self> {
        let me = Rc::new(ProfilesViewModel {
            service,
            st: RefCell::new(State {
                sections: ProfileSection::ALL.iter().map(|s| SectionOption { section: *s, is_selected: true }).collect(),
                ..Default::default()
            }),
            changed: Notifier::default(),
            list_changed: LocalEvent::default(),
        });
        me.refresh();
        me
    }

    pub fn items(&self) -> Vec<ProfileItem> {
        self.st.borrow().items.clone()
    }

    pub fn sections(&self) -> Vec<SectionOption> {
        self.st.borrow().sections.clone()
    }

    pub fn status(&self) -> (String, bool) {
        let st = self.st.borrow();
        (st.status_text.clone(), st.status_is_error)
    }

    pub fn new_name(&self) -> String {
        self.st.borrow().new_name.clone()
    }

    /// Önizleme: (ad, bölümler, değişenler).
    pub fn pending(&self) -> Option<(String, String, String)> {
        let st = self.st.borrow();
        st.pending.as_ref()?;
        Some((st.pending_name.clone(), st.pending_sections_text.clone(), st.pending_changes_text.clone()))
    }

    pub fn set_new_name(&self, name: &str) {
        self.st.borrow_mut().new_name = name.to_string();
    }

    pub fn set_section_selected(&self, index: usize, selected: bool) {
        if let Some(s) = self.st.borrow_mut().sections.get_mut(index) {
            s.is_selected = selected;
        }
        self.changed.notify();
    }

    pub fn refresh(&self) {
        let items = self
            .service
            .list()
            .into_iter()
            .map(|p| ProfileItem {
                sections_text: describe_sections(&ProfileLibrary::sections_of(&p)),
                edit_name: p.name.clone(),
                id: p.id,
                name: p.name,
                is_renaming: false,
                is_delete_confirming: false,
            })
            .collect();
        self.st.borrow_mut().items = items;
        self.changed.notify();
        self.list_changed.invoke(&());
    }

    fn selected_sections(&self) -> Vec<ProfileSection> {
        self.st.borrow().sections.iter().filter(|s| s.is_selected).map(|s| s.section).collect()
    }

    fn report(&self, key: &str, error: bool, args: &[&str]) {
        let text = if args.is_empty() { Strings::get(key) } else { Strings::format(key, args) };
        {
            let mut st = self.st.borrow_mut();
            st.status_text = text;
            st.status_is_error = error;
        }
        self.changed.notify();
    }

    fn report_library_error(&self, error: ProfileLibraryError) {
        self.report(&format!("ProfileError{error:?}"), true, &[]);
    }

    pub fn save_current(&self) {
        let name = self.new_name();
        let result = self.service.create_from_current(&name, &self.selected_sections());
        if !result.is_success() {
            self.report_library_error(result.error);
            return;
        }
        self.st.borrow_mut().new_name.clear();
        self.refresh();
        let n = result.profile.map(|p| p.name).unwrap_or_default();
        self.report("ProfileSaved", false, &[&n]);
    }

    pub fn export_current(&self) {
        let name = self.new_name().trim().to_string();
        if !SettingsProfileCodec::is_valid_name(Some(&name)) {
            self.report_library_error(ProfileLibraryError::InvalidName);
            return;
        }
        let sections = self.selected_sections();
        if sections.is_empty() {
            self.report_library_error(ProfileLibraryError::NoSections);
            return;
        }
        // Dosya, geçerli (kaydedilmiş) ayarlardan üretilir; kütüphaneye eklenmez.
        let text = SettingsProfileCodec::export(&self.service.current_settings(), &name, &sections);
        self.write_export_file(&name, &text);
    }

    fn find_item(&self, index: usize) -> Option<SavedProfile> {
        let id = self.st.borrow().items.get(index).map(|i| i.id.clone())?;
        let found = self.service.find(Some(&id));
        if found.is_none() {
            self.report_library_error(ProfileLibraryError::NotFound);
            self.refresh();
        }
        found
    }

    pub fn export(&self, index: usize) {
        let Some(profile) = self.find_item(index) else { return };
        self.write_export_file(&profile.name, &profile.json);
    }

    fn write_export_file(&self, name: &str, text: &str) {
        let file_name = format!("{}{}", safe_file_name(name), FILE_EXTENSION);
        let Some(path) = crate::infra::file_dialog::save_file(
            None,
            &Strings::get("ProfileExportDialogTitle"),
            &Strings::get("ProfileFileFilter"),
            &file_name,
            "json",
        ) else {
            return;
        };
        match std::fs::write(&path, text.as_bytes()) {
            Ok(()) => {
                let short = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                self.report("ProfileExported", false, &[&short]);
            }
            Err(_) => self.report("ProfileExportFailed", true, &[]),
        }
    }

    pub fn import(&self) {
        let Some(path) =
            crate::infra::file_dialog::pick_file(None, &Strings::get("ProfileImportDialogTitle"), &Strings::get("ProfileFileFilter"), true)
        else {
            return;
        };
        // Çok büyük dosya belleğe okunmadan reddedilir (güvenilmez girdi).
        match std::fs::metadata(&path) {
            Ok(m) if m.len() as i64 > ProfileLibrary::MAX_FILE_BYTES => {
                self.report("ProfileImportErrorTooLarge", true, &[]);
                return;
            }
            Err(_) => {
                self.report("ProfileImportReadFailed", true, &[]);
                return;
            }
            _ => {}
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            self.report("ProfileImportReadFailed", true, &[]);
            return;
        };
        let read = SettingsProfileCodec::read(text.trim_start_matches('\u{feff}'));
        if !read.is_valid() {
            self.clear_pending();
            self.report(&format!("ProfileImportError{:?}", read.error), true, &[]);
            return;
        }
        let Some(imported) = read.profile else { return };
        let changed = SettingsProfileCodec::changed_sections(&self.service.current_settings(), &imported);
        {
            let mut st = self.st.borrow_mut();
            st.pending_name = imported.name.clone();
            st.pending_sections_text = describe_sections(&imported.sections);
            st.pending_changes_text = if changed.is_empty() {
                Strings::get("ProfileImportNoChanges")
            } else {
                Strings::format("ProfileImportChanges", &[&describe_sections(&changed)])
            };
            st.pending = Some(imported);
            st.status_text.clear();
        }
        self.changed.notify();
    }

    pub fn confirm_import_save(&self) {
        let Some(pending) = self.st.borrow().pending.clone() else { return };
        let result = self.service.add_imported(&pending);
        if !result.is_success() {
            self.report_library_error(result.error);
            return;
        }
        self.clear_pending();
        self.refresh();
        let n = result.profile.map(|p| p.name).unwrap_or_default();
        self.report("ProfileSaved", false, &[&n]);
    }

    pub fn confirm_import_apply(&self) {
        let Some(pending) = self.st.borrow().pending.clone() else { return };
        let added = self.service.add_imported(&pending);
        if !added.is_success() {
            self.report_library_error(added.error);
            return;
        }
        self.clear_pending();
        self.refresh();
        if let Some(p) = added.profile {
            self.apply_profile(&p);
        }
    }

    pub fn cancel_import(&self) {
        self.clear_pending();
    }

    fn clear_pending(&self) {
        {
            let mut st = self.st.borrow_mut();
            st.pending = None;
            st.pending_name.clear();
            st.pending_sections_text.clear();
            st.pending_changes_text.clear();
        }
        self.changed.notify();
    }

    pub fn apply(&self, index: usize) {
        let Some(profile) = self.find_item(index) else { return };
        self.apply_profile(&profile);
    }

    fn apply_profile(&self, profile: &SavedProfile) {
        let result = self.service.apply(Some(&profile.id));
        if !result.is_success() {
            self.report_library_error(result.error);
            return;
        }
        let key = if result.changed_sections.is_empty() { "ProfileAppliedNoChange" } else { "ProfileApplied" };
        self.report(key, false, &[&profile.name]);
    }

    pub fn begin_rename(&self, index: usize) {
        if let Some(item) = self.st.borrow_mut().items.get_mut(index) {
            item.edit_name = item.name.clone();
            item.is_renaming = true;
        }
        self.changed.notify();
    }

    pub fn set_edit_name(&self, index: usize, name: &str) {
        if let Some(item) = self.st.borrow_mut().items.get_mut(index) {
            item.edit_name = name.to_string();
        }
    }

    pub fn cancel_rename(&self, index: usize) {
        if let Some(item) = self.st.borrow_mut().items.get_mut(index) {
            item.is_renaming = false;
        }
        self.changed.notify();
    }

    pub fn commit_rename(&self, index: usize) {
        let Some((id, name)) = self.st.borrow().items.get(index).map(|i| (i.id.clone(), i.edit_name.clone())) else { return };
        let result = self.service.rename(&id, &name);
        if !result.is_success() {
            self.report_library_error(result.error);
            return;
        }
        self.refresh();
        let n = result.profile.map(|p| p.name).unwrap_or_default();
        self.report("ProfileRenamed", false, &[&n]);
    }

    pub fn duplicate(&self, index: usize) {
        let Some(id) = self.st.borrow().items.get(index).map(|i| i.id.clone()) else { return };
        let result = self.service.duplicate(&id);
        if !result.is_success() {
            self.report_library_error(result.error);
            return;
        }
        self.refresh();
        let n = result.profile.map(|p| p.name).unwrap_or_default();
        self.report("ProfileSaved", false, &[&n]);
    }

    pub fn request_delete(&self, index: usize, on: bool) {
        if let Some(item) = self.st.borrow_mut().items.get_mut(index) {
            item.is_delete_confirming = on;
        }
        self.changed.notify();
    }

    pub fn delete(&self, index: usize) {
        let Some(id) = self.st.borrow().items.get(index).map(|i| i.id.clone()) else { return };
        let result = self.service.delete(&id);
        if !result.is_success() {
            self.report_library_error(result.error);
            return;
        }
        self.refresh();
        let n = result.profile.map(|p| p.name).unwrap_or_default();
        self.report("ProfileDeleted", false, &[&n]);
    }
}

fn safe_file_name(name: &str) -> String {
    const INVALID: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let cleaned: String = name.chars().map(|c| if INVALID.contains(&c) || c.is_control() { '_' } else { c }).collect();
    let cleaned = cleaned.trim_matches(|c| c == ' ' || c == '.').to_string();
    if cleaned.is_empty() { "koy-profile".to_string() } else { cleaned }
}
