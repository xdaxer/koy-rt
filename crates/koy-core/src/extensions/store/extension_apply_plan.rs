//! Karşılığı: `Extensions/Store/ExtensionApplyPlan.cs`.

use super::{ExtensionKind, InstalledEntry, PackageEntryPolicy};
use crate::appearance::{IconPack, IconPackFiles};
use crate::notch::{LiveBackgroundFileError, LiveBackgroundFileValidator, LiveBackgroundSourceKind, LiveBackgroundTarget};
use crate::settings::AppSettings;
use crate::text;
use crate::win_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ExtensionApplyError {
    #[default]
    None,

    /// Kurulum klasörü ya da ana dosya yolu güvenli bir yerel yol değil.
    InvalidPath,
}

/// Plana göre App/Platform'un yapacağı dosya işi. Plan dosyaya dokunmaz, yalnız ne yapılacağını söyler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ExtensionFileActionKind {
    /// Kullanıcının ÖZGÜN stil durumunu sakla: user-theme.json varsa user-theme.backup.json olarak kopyala, her durumda
    /// özgün `UserStyleOverridesEnabled` değerini (`ExtensionFileAction::original_overrides_enabled`) durum
    /// dosyasına yaz ("özgün dosya yoktu" da böyle kaydedilir). Yedek ya da durum dosyası zaten varsa dokunma (ilk özgün kalsın).
    #[default]
    BackupUserStyle,

    /// `ExtensionFileAction::source_path` dosyasını user-theme.json olarak kopyala.
    CopyThemeToUserStyle,

    /// Yedek varsa user-theme.json üzerine geri taşı; yoksa (özgün dosya yoktu) user-theme.json'u sil. Durum dosyasını da temizler.
    RestoreUserStyleBackup,

    /// `ExtensionFileAction::source_path` profil dosyasını oku, `SettingsProfileCodec::read` ile doğrula ve
    /// `ProfileLibrary::add_imported` ile kitaplığa ekle. Ayarlara uygulanmaz.
    ImportProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionFileAction {
    pub kind: ExtensionFileActionKind,
    pub source_path: Option<String>,
    pub original_overrides_enabled: bool,
}

impl ExtensionFileAction {
    /// `new ExtensionFileAction(kind, sourcePath = null, originalOverridesEnabled = false)`.
    pub fn new(kind: ExtensionFileActionKind, source_path: Option<String>, original_overrides_enabled: bool) -> Self {
        ExtensionFileAction { kind, source_path, original_overrides_enabled }
    }

    fn of(kind: ExtensionFileActionKind) -> Self {
        Self::new(kind, None, false)
    }
}

/// Tema uygulanmadan önceki kullanıcı stil durumu (diskteki yedekten okunur). `has_file`: özgün user-theme.json
/// vardı (yedeği duruyor); false ise özgün durumda dosya yoktu. `overrides_enabled`: özgün
/// `UserStyleOverridesEnabled`. Yedek de durum dosyası da yoksa çağıran `None` verir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserStyleBackup {
    pub has_file: bool,
    pub overrides_enabled: bool,
}

impl UserStyleBackup {
    /// Durum dosyasının adı (user-theme.json ile aynı klasörde).
    pub const STATE_FILE_NAME: &'static str = "user-theme.backup.state";

    pub fn new(has_file: bool, overrides_enabled: bool) -> Self {
        UserStyleBackup { has_file, overrides_enabled }
    }

    pub fn serialize_state(overrides_enabled: bool) -> String {
        format!("overrides={}", if overrides_enabled { "true" } else { "false" })
    }

    /// Metin tanınırsa özgün `UserStyleOverridesEnabled` değeri; tanınmazsa `None` (C# `false` döner, `out` false).
    pub fn try_parse_state(text: Option<&str>) -> Option<bool> {
        match text.map(str::trim) {
            Some("overrides=true") => Some(true),
            Some("overrides=false") => Some(false),
            _ => None,
        }
    }
}

/// Uygula/Kaldır kararı. `settings` yeni ayarlardır (çağıranın verdiği nesne değişmez); değişiklik yoksa
/// aynı değerler taşıyan bir kopyadır. Dosya işleri sırayla yapılır, sonra ayarlar kaydedilir.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionApplyPlan {
    pub error: ExtensionApplyError,
    pub settings: Option<AppSettings>,
    pub file_actions: Vec<ExtensionFileAction>,
}

impl ExtensionApplyPlan {
    pub fn new(error: ExtensionApplyError, settings: Option<AppSettings>, file_actions: Vec<ExtensionFileAction>) -> Self {
        ExtensionApplyPlan { error, settings, file_actions }
    }

    pub fn is_success(&self) -> bool {
        self.error == ExtensionApplyError::None && self.settings.is_some()
    }

    pub fn fail(error: ExtensionApplyError) -> ExtensionApplyPlan {
        ExtensionApplyPlan { error, settings: None, file_actions: Vec::new() }
    }

    fn ok(settings: AppSettings, file_actions: Vec<ExtensionFileAction>) -> ExtensionApplyPlan {
        ExtensionApplyPlan { error: ExtensionApplyError::None, settings: Some(settings), file_actions }
    }
}

/// "Uygula" ve kaldırmada ayar geri alma kararları (docs/EXTENSION-STORE.md › Uygulama). Saf: dosya sistemi ve
/// ayar dosyası yok; mevcut alanlar kullanılır (IconPack, IconPackFolder, UserStyleOverridesEnabled,
/// Notch.LiveBackground*).
pub struct ExtensionApplyPlanner;

impl ExtensionApplyPlanner {
    pub const USER_STYLE_BACKUP_FILE_NAME: &'static str = "user-theme.backup.json";

    /// Uygulanabilir ayar karşılığı olan türden eklentiyi uygulayan plan.
    /// `theme_already_applied` (C# varsayılanı `false`): tema için kayıtta zaten uygulanmış bir mağaza teması var
    /// (bu dahil). O zaman şu anki user-theme.json bir mağaza temasıdır, kullanıcının özgünü değil; yedek alınmaz
    /// (ilk uygulamadaki özgün yedek korunur).
    pub fn apply(current: &AppSettings, entry: &InstalledEntry, install_folder: &str, theme_already_applied: bool) -> ExtensionApplyPlan {
        if !IconPackFiles::is_safe_folder(Some(install_folder)) {
            return ExtensionApplyPlan::fail(ExtensionApplyError::InvalidPath);
        }

        match entry.kind {
            ExtensionKind::Theme => {
                let source = win_path::combine(install_folder, PackageEntryPolicy::THEME_NAME);
                let mut settings = current.clone();
                settings.appearance.user_style_overrides_enabled = true;
                let mut actions: Vec<ExtensionFileAction> = Vec::new();
                if !theme_already_applied {
                    actions.push(ExtensionFileAction::new(
                        ExtensionFileActionKind::BackupUserStyle,
                        None,
                        current.appearance.user_style_overrides_enabled,
                    ));
                }

                actions.push(ExtensionFileAction::new(ExtensionFileActionKind::CopyThemeToUserStyle, Some(source), false));
                ExtensionApplyPlan::ok(settings, actions)
            }

            ExtensionKind::IconPack => {
                let mut settings = current.clone();
                settings.appearance.icon_pack = IconPack::Folder;
                settings.appearance.icon_pack_folder = Self::icons_folder_of(install_folder);
                ExtensionApplyPlan::ok(settings, Vec::new())
            }

            ExtensionKind::Background => {
                let Some(path) = Self::background_path_of(entry, install_folder) else {
                    return ExtensionApplyPlan::fail(ExtensionApplyError::InvalidPath);
                };
                if LiveBackgroundFileValidator::validate(Some(&path), 1) != LiveBackgroundFileError::None {
                    return ExtensionApplyPlan::fail(ExtensionApplyError::InvalidPath);
                }

                // Hedef kapalıysa çentik: yoksa arka planı seçmek hiçbir şey göstermezdi.
                let target = if current.notch.live_background_target == LiveBackgroundTarget::None {
                    LiveBackgroundTarget::Notch
                } else {
                    current.notch.live_background_target
                };
                let mut settings = current.clone();
                settings.notch.live_background_source = LiveBackgroundSourceKind::LocalFile;
                settings.notch.live_background_file_path = path;
                settings.notch.live_background_target = target;
                ExtensionApplyPlan::ok(settings, Vec::new())
            }

            // Profil uygulanmaz, yalnız kitaplığa girer; kullanıcı Profiller bölümünden uygular.
            ExtensionKind::Profile => ExtensionApplyPlan::ok(
                current.clone(),
                vec![ExtensionFileAction::new(
                    ExtensionFileActionKind::ImportProfile,
                    Some(win_path::combine(install_folder, PackageEntryPolicy::PROFILE_NAME)),
                    false,
                )],
            ),
        }
    }

    /// Kaldırmadan önce ayarı geri alan plan: simge paketi → Yerleşik, arka plan → Ambiyans, tema → yedek geri
    /// yüklenir. Simge paketi ve arka plan için ayarın hâlâ bu eklentiyi göstermesine bakılır (kullanıcı sonradan
    /// başka bir şey seçtiyse dokunulmaz); tema için kayıttaki `applied` bayrağına. Hedef (Notch / Müzik kartı)
    /// değişmez. Profil türünde yapılacak bir şey yoktur (içe alınan profil kitaplıkta kalır).
    /// `backup` (C# varsayılanı `null`): tema için diskteki özgün durum yedeği. Özgün dosya varsa geri yüklenir, yoksa
    /// user-theme.json silinir; geçersiz kılma her iki durumda özgün değerine döner. Yedek hiç yoksa (`None`) dosya
    /// silinir ve kapatılır.
    pub fn revert_on_uninstall(
        current: &AppSettings,
        entry: &InstalledEntry,
        install_folder: &str,
        backup: Option<UserStyleBackup>,
    ) -> ExtensionApplyPlan {
        match entry.kind {
            ExtensionKind::Theme if entry.applied => {
                let mut settings = current.clone();
                settings.appearance.user_style_overrides_enabled = backup.map(|b| b.overrides_enabled).unwrap_or(false);
                ExtensionApplyPlan::ok(settings, vec![ExtensionFileAction::of(ExtensionFileActionKind::RestoreUserStyleBackup)])
            }

            ExtensionKind::IconPack if Self::points_at_icon_pack(current, install_folder) => {
                let mut settings = current.clone();
                settings.appearance.icon_pack = IconPack::Builtin;
                settings.appearance.icon_pack_folder = String::new();
                ExtensionApplyPlan::ok(settings, Vec::new())
            }

            ExtensionKind::Background if Self::points_at_background(current, entry, install_folder) => {
                let mut settings = current.clone();
                settings.notch.live_background_source = LiveBackgroundSourceKind::BuiltInAmbient;
                settings.notch.live_background_file_path = String::new();
                ExtensionApplyPlan::ok(settings, Vec::new())
            }

            _ => ExtensionApplyPlan::ok(current.clone(), Vec::new()),
        }
    }

    /// Eklenti şu an etkin mi (arayüzde "Uygulandı" rozeti). Simge paketi ve arka plan için ayarın gösterdiği yola,
    /// tema için kayıt bayrağı ve geçersiz kılmanın açık olmasına bakılır; profil hiçbir zaman etkin sayılmaz.
    pub fn is_active(current: &AppSettings, entry: &InstalledEntry, install_folder: &str) -> bool {
        match entry.kind {
            ExtensionKind::Theme => entry.applied && current.appearance.user_style_overrides_enabled,
            ExtensionKind::IconPack => Self::points_at_icon_pack(current, install_folder),
            ExtensionKind::Background => Self::points_at_background(current, entry, install_folder),
            _ => false,
        }
    }

    fn icons_folder_of(install_folder: &str) -> String {
        win_path::combine(install_folder, PackageEntryPolicy::ICONS_FOLDER)
    }

    fn background_path_of(entry: &InstalledEntry, install_folder: &str) -> Option<String> {
        if PackageEntryPolicy::is_background_name(&entry.payload_file) {
            Some(win_path::combine(install_folder, &entry.payload_file))
        } else {
            None
        }
    }

    fn points_at_icon_pack(s: &AppSettings, install_folder: &str) -> bool {
        s.appearance.icon_pack == IconPack::Folder
            && Self::same_path(Some(&s.appearance.icon_pack_folder), Some(&Self::icons_folder_of(install_folder)))
    }

    fn points_at_background(s: &AppSettings, entry: &InstalledEntry, install_folder: &str) -> bool {
        s.notch.live_background_source == LiveBackgroundSourceKind::LocalFile
            && Self::background_path_of(entry, install_folder)
                .is_some_and(|path| Self::same_path(Some(&s.notch.live_background_file_path), Some(&path)))
    }

    // Yol karşılaştırması: büyük/küçük harf duyarsız, sondaki ayırıcı yok sayılır.
    fn same_path(a: Option<&str>, b: Option<&str>) -> bool {
        if text::is_null_or_white_space(a) || text::is_null_or_white_space(b) {
            return false;
        }

        let trim = |s: &str| s.trim().trim_end_matches(['\\', '/']).to_string();
        text::eq_ignore_case(&trim(a.unwrap_or_default()), &trim(b.unwrap_or_default()))
    }
}
