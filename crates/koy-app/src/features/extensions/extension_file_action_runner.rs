//! Karşılığı: `koy/src/App/Features/Extensions/ExtensionFileActionRunner.cs`.
//!
//! `ExtensionApplyPlanner` planının dosya işlerini yapar (kararlar Core'da, burada yalnız G/Ç). Kullanıcı stil dosyası
//! (user-theme.json) ve yedeği aynı klasörde durur; her yazma geçici dosya + taşıma ile yapılır. Beklenen G/Ç hatası
//! panik yapmaz, resx anahtarı döner.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use koy_core::appearance::UserStyleFile;
use koy_core::extensions::store::{ExtensionApplyPlanner, ExtensionFileAction, ExtensionFileActionKind, UserStyleBackup};
use koy_core::settings::{ProfileLibrary, SavedProfile, SettingsProfileCodec};

use crate::features::settings::profile_service::ProfileService;

/// Dosya işlerinin sonucu: hata varsa resx anahtarı; profil içe alındıysa kayıt (zaten varsa `profile_already_present`).
#[derive(Clone, Debug, Default)]
pub struct ExtensionFileActionResult {
    pub error_key: Option<String>,
    pub imported_profile: Option<SavedProfile>,
    pub profile_already_present: bool,
}

impl ExtensionFileActionResult {
    fn error(key: impl Into<String>) -> Self {
        ExtensionFileActionResult { error_key: Some(key.into()), ..Default::default() }
    }

    pub fn is_success(&self) -> bool {
        self.error_key.is_none()
    }
}

/// Kullanıcı stil dosyalarının bellekteki kopyası; `None` alan: dosya yoktu.
#[derive(Clone, Debug)]
pub struct UserStyleSnapshot {
    style: Option<Vec<u8>>,
    backup: Option<Vec<u8>>,
    state: Option<Vec<u8>>,
}

pub struct ExtensionFileActionRunner {
    folder: PathBuf,
    profiles: Rc<ProfileService>,
    pub user_style_path: PathBuf,
    pub backup_path: PathBuf,
    pub state_path: PathBuf,
}

fn read_or_none(path: &Path) -> std::io::Result<Option<Vec<u8>>> {
    if path.exists() { std::fs::read(path).map(Some) } else { Ok(None) }
}

fn write_or_delete(path: &Path, data: &Option<Vec<u8>>) -> std::io::Result<()> {
    match data {
        None => {
            if path.exists() {
                std::fs::remove_file(path)?;
            }
            Ok(())
        }
        Some(bytes) => {
            let temp = tmp(path);
            std::fs::write(&temp, bytes)?;
            std::fs::rename(&temp, path)
        }
    }
}

fn tmp(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".tmp");
    PathBuf::from(s)
}

impl ExtensionFileActionRunner {
    pub fn new(user_style_folder: Option<PathBuf>, profiles: Rc<ProfileService>) -> Self {
        let folder = user_style_folder.unwrap_or_else(UserStyleFile::default_folder);
        ExtensionFileActionRunner {
            user_style_path: folder.join(UserStyleFile::FILE_NAME),
            backup_path: folder.join(ExtensionApplyPlanner::USER_STYLE_BACKUP_FILE_NAME),
            state_path: folder.join(UserStyleBackup::STATE_FILE_NAME),
            folder,
            profiles,
        }
    }

    /// Diskteki özgün durum yedeği; hiç yedek yoksa `None`. Durum dosyası yoksa (eski sürümün yedeği) özgün geçersiz
    /// kılma, yedek dosyasının varlığından çıkarılır.
    pub fn read_backup(&self) -> Option<UserStyleBackup> {
        let has_file = self.backup_path.exists();
        let state_exists = self.state_path.exists();
        let overrides = if state_exists {
            std::fs::read_to_string(&self.state_path).ok().and_then(|t| UserStyleBackup::try_parse_state(Some(&t)))
        } else {
            None
        };
        if !has_file && overrides.is_none() && !state_exists {
            return None;
        }
        Some(UserStyleBackup::new(has_file, overrides.unwrap_or(has_file)))
    }

    pub fn take_snapshot(&self) -> Option<UserStyleSnapshot> {
        Some(UserStyleSnapshot {
            style: read_or_none(&self.user_style_path).ok()?,
            backup: read_or_none(&self.backup_path).ok()?,
            state: read_or_none(&self.state_path).ok()?,
        })
    }

    /// Dosyaları `take_snapshot` anına döndürür (en iyi çaba).
    pub fn restore_snapshot(&self, snapshot: &UserStyleSnapshot) -> bool {
        let run = || -> std::io::Result<()> {
            std::fs::create_dir_all(&self.folder)?;
            write_or_delete(&self.user_style_path, &snapshot.style)?;
            write_or_delete(&self.backup_path, &snapshot.backup)?;
            write_or_delete(&self.state_path, &snapshot.state)
        };
        run().is_ok()
    }

    pub fn run(&self, actions: &[ExtensionFileAction]) -> ExtensionFileActionResult {
        let mut imported = None;
        let mut already = false;
        for action in actions {
            let step = || -> std::io::Result<Option<ExtensionFileActionResult>> {
                match action.kind {
                    ExtensionFileActionKind::BackupUserStyle => {
                        // İlk özgün durum kalsın: yedek ya da durum dosyası varsa üzerine yazılmaz. Önce dosya, sonra durum.
                        if self.backup_path.exists() || self.state_path.exists() {
                            return Ok(None);
                        }
                        std::fs::create_dir_all(&self.folder)?;
                        if self.user_style_path.exists() {
                            std::fs::copy(&self.user_style_path, &self.backup_path)?;
                        }
                        let temp = tmp(&self.state_path);
                        std::fs::write(&temp, UserStyleBackup::serialize_state(action.original_overrides_enabled))?;
                        std::fs::rename(&temp, &self.state_path)?;
                    }
                    ExtensionFileActionKind::CopyThemeToUserStyle => {
                        let Some(source) = action.source_path.as_deref().filter(|s| Path::new(s).exists()) else {
                            return Ok(Some(ExtensionFileActionResult::error("ExtApplyFileError")));
                        };
                        std::fs::create_dir_all(&self.folder)?;
                        let temp = tmp(&self.user_style_path);
                        std::fs::copy(source, &temp)?;
                        std::fs::rename(&temp, &self.user_style_path)?;
                    }
                    ExtensionFileActionKind::RestoreUserStyleBackup => {
                        if self.backup_path.exists() {
                            std::fs::rename(&self.backup_path, &self.user_style_path)?;
                        } else if self.user_style_path.exists() {
                            std::fs::remove_file(&self.user_style_path)?;
                        }
                        if self.state_path.exists() {
                            std::fs::remove_file(&self.state_path)?;
                        }
                    }
                    ExtensionFileActionKind::ImportProfile => {
                        let r = self.import_profile(action.source_path.as_deref());
                        if !r.is_success() {
                            return Ok(Some(r));
                        }
                        return Ok(Some(r));
                    }
                }
                Ok(None)
            };
            match step() {
                Err(_) => return ExtensionFileActionResult::error("ExtApplyFileError"),
                Ok(Some(r)) if !r.is_success() => return r,
                Ok(Some(r)) => {
                    imported = r.imported_profile;
                    already = r.profile_already_present;
                }
                Ok(None) => {}
            }
        }
        ExtensionFileActionResult { error_key: None, imported_profile: imported, profile_already_present: already }
    }

    fn import_profile(&self, path: Option<&str>) -> ExtensionFileActionResult {
        let Some(path) = path.filter(|p| Path::new(p).exists()) else {
            return ExtensionFileActionResult::error("ExtApplyFileError");
        };
        if std::fs::metadata(path).map_or(0, |m| m.len() as i64) > ProfileLibrary::MAX_FILE_BYTES {
            return ExtensionFileActionResult::error("ProfileImportErrorTooLarge");
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            return ExtensionFileActionResult::error("ExtApplyFileError");
        };
        let read = SettingsProfileCodec::read(text.trim_start_matches('\u{feff}'));
        if !read.is_valid() {
            return ExtensionFileActionResult::error(format!("ProfileImportError{:?}", read.error));
        }
        let Some(profile) = read.profile else { return ExtensionFileActionResult::error("ExtApplyFileError") };
        // Aynı ad kitaplıkta varsa bir daha eklenmez ("Ad (2)" yığılmasın): "Uygula" tekrar basılabilir.
        if let Some(existing) = ProfileLibrary::find(&self.profiles.list(), Some(&profile.name)) {
            return ExtensionFileActionResult { error_key: None, imported_profile: Some(existing.clone()), profile_already_present: true };
        }
        let added = self.profiles.add_imported(&profile);
        if added.is_success() {
            ExtensionFileActionResult { error_key: None, imported_profile: added.profile, profile_already_present: false }
        } else {
            ExtensionFileActionResult::error(format!("ProfileError{:?}", added.error))
        }
    }
}
