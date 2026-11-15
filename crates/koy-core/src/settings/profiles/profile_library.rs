//! Karşılığı: `koy/src/Core/Settings/Profiles/ProfileLibrary.cs`.

use super::{ImportedProfile, ProfileSection, SavedProfile, SettingsProfileCodec};
use crate::settings::AppSettings;
use crate::text;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ProfileLibraryError {
    #[default]
    None,
    InvalidName,
    DuplicateName,
    NoSections,
    NotFound,
    LimitReached,
    /// Kayıtlı metin artık doğrulamadan geçmiyor (dosya elle bozulmuş).
    CorruptProfile,
    /// İşlem geçerliydi ama profil dosyası yazılamadı (disk dolu, kilit). Kütüphane üretmez, çağıran ekler.
    SaveFailed,
}

/// Kütüphane işleminin sonucu: hata yoksa `profiles` yeni liste, `profile` etkilenen kayıt.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileLibraryResult {
    pub error: ProfileLibraryError,
    pub profiles: Vec<SavedProfile>,
    pub profile: Option<SavedProfile>,
}

impl ProfileLibraryResult {
    pub fn is_success(&self) -> bool {
        self.error == ProfileLibraryError::None
    }
}

/// Kayıtlı profili ayarlara uygulama sonucu. Hata yoksa `settings` kaydedilecek yeni ayarlardır.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileApplyResult {
    pub error: ProfileLibraryError,
    pub profile: Option<SavedProfile>,
    pub settings: Option<AppSettings>,
    pub changed_sections: Vec<ProfileSection>,
}

impl ProfileApplyResult {
    pub fn is_success(&self) -> bool {
        self.error == ProfileLibraryError::None && self.settings.is_some()
    }
}

/// Kayıtlı profil listesi üzerindeki saf işlemler: oluştur, içe al, yeniden adlandır, çoğalt, sil, bul, uygula.
/// Dosya G/Ç yok (bkz. `ProfileStore`); girdi liste değiştirilmez, her işlem yeni liste döner.
/// Bağlam kuralları (uygulamaya göre profil) profili `find` + `apply` ile kullanır.
pub struct ProfileLibrary;

impl ProfileLibrary {
    pub const MAX_PROFILES: i32 = 50;

    /// İçe aktarılan dosyanın en büyük bayt boyutu (UTF-8'de karakter başına en çok 4 bayt): dosya belleğe okunmadan reddedilir.
    pub const MAX_FILE_BYTES: i64 = SettingsProfileCodec::MAX_LENGTH as i64 * 4;

    /// Önce kimlik (birebir), sonra ad (büyük/küçük harf duyarsız, kırpılmış). Bulunamazsa `None`.
    pub fn find<'a>(profiles: &'a [SavedProfile], id_or_name: Option<&str>) -> Option<&'a SavedProfile> {
        let id_or_name = id_or_name?;
        if text::is_null_or_white_space(Some(id_or_name)) {
            return None;
        }
        profiles
            .iter()
            .find(|p| p.id == id_or_name)
            .or_else(|| profiles.iter().find(|p| text::eq_ignore_case(&p.name, id_or_name.trim())))
    }

    /// Profilin taşıdığı bölümler; metin geçersizse boş liste.
    pub fn sections_of(profile: &SavedProfile) -> Vec<ProfileSection> {
        let result = SettingsProfileCodec::read(&profile.json);
        match result.profile {
            Some(p) if result.error == super::ProfileImportError::None => p.sections,
            _ => Vec::new(),
        }
    }

    /// Geçerli ayarlardan seçili bölümlerle yeni profil. Ad benzersiz olmalı (büyük/küçük harf duyarsız).
    pub fn create(
        profiles: &[SavedProfile],
        current: &AppSettings,
        name: &str,
        sections: &[ProfileSection],
        new_id: &str,
    ) -> ProfileLibraryResult {
        if !SettingsProfileCodec::is_valid_name(Some(name)) {
            return fail(profiles, ProfileLibraryError::InvalidName);
        }
        let mut chosen: Vec<ProfileSection> = Vec::new();
        for s in sections {
            if !chosen.contains(s) {
                chosen.push(*s);
            }
        }
        // Tanımsız bölüm sayısı Rust'ta temsil edilemez (C#: !chosen.All(Enum.IsDefined) → NoSections).
        if chosen.is_empty() {
            return fail(profiles, ProfileLibraryError::NoSections);
        }
        if profiles.len() >= Self::MAX_PROFILES as usize {
            return fail(profiles, ProfileLibraryError::LimitReached);
        }
        if name_taken(profiles, name, None) {
            return fail(profiles, ProfileLibraryError::DuplicateName);
        }

        let saved = SavedProfile {
            id: new_id.to_string(),
            name: name.trim().to_string(),
            json: SettingsProfileCodec::export(current, name, &chosen),
        };
        let mut list = profiles.to_vec();
        list.push(saved.clone());
        ok(list, saved)
    }

    /// Doğrulanmış içe aktarılan profili listeye ekler. Ad çakışırsa "Ad (2)" gibi benzersiz ad verilir (içe aktarma
    /// mevcut profili sessizce ezmez).
    pub fn add_imported(profiles: &[SavedProfile], imported: &ImportedProfile, new_id: &str) -> ProfileLibraryResult {
        if profiles.len() >= Self::MAX_PROFILES as usize {
            return fail(profiles, ProfileLibraryError::LimitReached);
        }

        let name = Self::unique_name(profiles, &imported.name);
        let saved = SavedProfile {
            id: new_id.to_string(),
            json: SettingsProfileCodec::export(&imported.settings, &name, &imported.sections),
            name,
        };
        let mut list = profiles.to_vec();
        list.push(saved.clone());
        ok(list, saved)
    }

    pub fn rename(profiles: &[SavedProfile], id: &str, new_name: &str) -> ProfileLibraryResult {
        let Some(target) = profiles.iter().find(|p| p.id == id) else {
            return fail(profiles, ProfileLibraryError::NotFound);
        };
        if !SettingsProfileCodec::is_valid_name(Some(new_name)) {
            return fail(profiles, ProfileLibraryError::InvalidName);
        }
        if name_taken(profiles, new_name, Some(id)) {
            return fail(profiles, ProfileLibraryError::DuplicateName);
        }

        let read = SettingsProfileCodec::read(&target.json);
        let Some(read_profile) = read.profile.as_ref().filter(|_| read.is_valid()) else {
            return fail(profiles, ProfileLibraryError::CorruptProfile);
        };

        // Dosya içindeki ad da değişir: dışa aktarılan dosya listedeki adı taşısın.
        let renamed = SavedProfile {
            name: new_name.trim().to_string(),
            json: SettingsProfileCodec::export(&read_profile.settings, new_name, &read_profile.sections),
            ..target.clone()
        };
        let list = profiles.iter().map(|p| if p.id == id { renamed.clone() } else { p.clone() }).collect();
        ok(list, renamed)
    }

    pub fn duplicate(profiles: &[SavedProfile], id: &str, new_id: &str) -> ProfileLibraryResult {
        let Some(source) = profiles.iter().find(|p| p.id == id) else {
            return fail(profiles, ProfileLibraryError::NotFound);
        };
        if profiles.len() >= Self::MAX_PROFILES as usize {
            return fail(profiles, ProfileLibraryError::LimitReached);
        }

        let read = SettingsProfileCodec::read(&source.json);
        let Some(read_profile) = read.profile.as_ref().filter(|_| read.is_valid()) else {
            return fail(profiles, ProfileLibraryError::CorruptProfile);
        };

        let name = Self::unique_name(profiles, &source.name);
        let copy = SavedProfile {
            id: new_id.to_string(),
            json: SettingsProfileCodec::export(&read_profile.settings, &name, &read_profile.sections),
            name,
        };
        let mut list = profiles.to_vec();
        list.push(copy.clone());
        ok(list, copy)
    }

    pub fn delete(profiles: &[SavedProfile], id: &str) -> ProfileLibraryResult {
        let Some(target) = profiles.iter().find(|p| p.id == id) else {
            return fail(profiles, ProfileLibraryError::NotFound);
        };
        ok(profiles.iter().filter(|p| p.id != id).cloned().collect(), target.clone())
    }

    /// Profili geçerli ayarların üzerine uygular (yalnız profilin bölümleri değişir). Çağıran sonucu bilinen ayar
    /// kayıt yolundan geçirir; burada dosya yazılmaz.
    pub fn apply(profiles: &[SavedProfile], current: &AppSettings, id_or_name: Option<&str>) -> ProfileApplyResult {
        let Some(profile) = Self::find(profiles, id_or_name) else {
            return ProfileApplyResult { error: ProfileLibraryError::NotFound, profile: None, settings: None, changed_sections: Vec::new() };
        };

        let read = SettingsProfileCodec::read(&profile.json);
        let Some(imported) = read.profile.as_ref().filter(|_| read.is_valid()) else {
            return ProfileApplyResult {
                error: ProfileLibraryError::CorruptProfile,
                profile: Some(profile.clone()),
                settings: None,
                changed_sections: Vec::new(),
            };
        };

        let changed = SettingsProfileCodec::changed_sections(current, imported);
        let settings = SettingsProfileCodec::apply(current, imported, &imported.sections);
        ProfileApplyResult {
            error: ProfileLibraryError::None,
            profile: Some(profile.clone()),
            settings: Some(settings),
            changed_sections: changed,
        }
    }

    /// Listede olmayan ad: "Ad", yoksa "Ad (2)", "Ad (3)"… Ad sınırını aşmamak için taban kısaltılır.
    pub fn unique_name(profiles: &[SavedProfile], base_name: &str) -> String {
        let trimmed = base_name.trim();
        if !name_taken(profiles, trimmed, None) {
            return trimmed.to_string();
        }

        let mut n = 2;
        loop {
            let suffix = format!(" ({n})");
            let trimmed_len = trimmed.encode_utf16().count();
            let suffix_len = suffix.encode_utf16().count();
            let max = SettingsProfileCodec::MAX_NAME_LENGTH as usize;
            let stem = if trimmed_len + suffix_len > max {
                take_utf16(trimmed, max - suffix_len).trim_end().to_string()
            } else {
                trimmed.to_string()
            };
            let candidate = format!("{stem}{suffix}");
            if !name_taken(profiles, &candidate, None) {
                return candidate;
            }
            n += 1;
        }
    }
}

/// `s[..n]` (UTF-16 kod birimi sayısıyla). Vekil çiftini bölmemek için sınırdaki karakter dışarıda kalır.
fn take_utf16(s: &str, n: usize) -> &str {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        units += c.len_utf16();
        if units > n {
            return &s[..i];
        }
    }
    s
}

fn name_taken(profiles: &[SavedProfile], name: &str, except_id: Option<&str>) -> bool {
    profiles.iter().any(|p| Some(p.id.as_str()) != except_id && text::eq_ignore_case(&p.name, name.trim()))
}

fn ok(profiles: Vec<SavedProfile>, profile: SavedProfile) -> ProfileLibraryResult {
    ProfileLibraryResult { error: ProfileLibraryError::None, profiles, profile: Some(profile) }
}

fn fail(profiles: &[SavedProfile], error: ProfileLibraryError) -> ProfileLibraryResult {
    ProfileLibraryResult { error, profiles: profiles.to_vec(), profile: None }
}
