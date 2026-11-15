//! Karşılığı: `koy/src/Core/Settings/Profiles/ImportedProfile.cs`, `ProfileSection.cs`, `SavedProfile.cs`.

use serde::{Deserialize, Serialize};

use crate::settings::AppSettings;

/// Profil ve dışa aktarımda taşınabilen ayar bölümleri. Adları settings.json'daki bölüm anahtarlarıyla aynıdır.
/// Bilerek dışarıda kalanlar makineye özgüdür ya da kişisel veri taşır ve içe aktarılmaz:
/// General (dil, sihirbaz, güncelleme kanalı), Displays (monitör kimlikleri), Weather (konum), DockItems (sabitli
/// uygulama yolları, klasörler), QuickLaunch, Clipboard, Search (dosya yolları, geçmiş), Voice (çalışma klasörü, komutlar).
/// Dock içindeki StartIconPath (özel başlat simgesinin yolu) da makineye özgüdür: dışa aktarmada boşaltılır, içe aktarmada yerel değer korunur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[repr(i32)]
pub enum ProfileSection {
    #[default]
    Appearance,
    Dock,
    Notch,
    Music,
    Shortcuts,
    Focus,
}

impl ProfileSection {
    /// `Enum.GetValues<ProfileSection>()`.
    pub const ALL: [ProfileSection; 6] = [
        ProfileSection::Appearance,
        ProfileSection::Dock,
        ProfileSection::Notch,
        ProfileSection::Music,
        ProfileSection::Shortcuts,
        ProfileSection::Focus,
    ];

    /// `section.ToString()`: ayar dosyasındaki bölüm anahtarı.
    pub fn name(self) -> &'static str {
        match self {
            ProfileSection::Appearance => "Appearance",
            ProfileSection::Dock => "Dock",
            ProfileSection::Notch => "Notch",
            ProfileSection::Music => "Music",
            ProfileSection::Shortcuts => "Shortcuts",
            ProfileSection::Focus => "Focus",
        }
    }

    /// Büyük/küçük harf duyarlı ad eşleştirme (`TryParseSection`).
    pub fn try_parse(name: &str) -> Option<ProfileSection> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }
}

/// Doğrulanmış bir profil dosyası. `settings` yalnız `sections` içindeki bölümleri taşır, diğerleri varsayılandır;
/// bu yüzden uygulanırken yalnız bildirilen bölümlere bakılır.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedProfile {
    pub name: String,
    pub sections: Vec<ProfileSection>,
    pub settings: AppSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ProfileImportError {
    #[default]
    None,
    TooLarge,
    MalformedJson,
    NotAProfile,
    UnsupportedFormatVersion,
    InvalidName,
    UnknownSection,
    NoSections,
    InvalidValues,
}

/// Okuma sonucu: hata varsa `profile` `None`'dır. Hiçbir dosya yazılmaz; yazma çağıranın işidir.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileImportResult {
    pub error: ProfileImportError,
    pub profile: Option<ImportedProfile>,
}

impl ProfileImportResult {
    pub fn is_valid(&self) -> bool {
        self.error == ProfileImportError::None && self.profile.is_some()
    }
}

/// Kayıtlı bir profil: kalıcı kimlik (yeniden adlandırmada değişmez; kurallar bunu tutar), görünen ad ve
/// `SettingsProfileCodec` metni. Metin okunurken her seferinde yeniden doğrulanır; dosya elle bozulsa da
/// uygulanmadan reddedilir. JSON adları C# konumsal kaydının özellik adlarıdır (PascalCase).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SavedProfile {
    pub id: String,
    pub name: String,
    pub json: String,
}

impl SavedProfile {
    pub fn new(id: &str, name: &str, json: &str) -> Self {
        SavedProfile { id: id.to_string(), name: name.to_string(), json: json.to_string() }
    }
}
