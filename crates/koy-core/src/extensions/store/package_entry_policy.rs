//! Karşılığı: `Extensions/Store/PackageEntryPolicy.cs`.

use std::collections::HashSet;

use super::{ExtensionInstallError, ExtensionKind};
use crate::appearance::{AppIconOverrides, UserStyleParser};
use crate::notch::LiveBackgroundFileValidator;
use crate::settings::ProfileLibrary;
use crate::text;
use crate::win_path;

/// Zip içindeki bir girdi: adı (zip'te yazıldığı gibi) ve bildirilen açılmış boyutu.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageEntry {
    pub name: String,
    pub length: i64,
}

impl PackageEntry {
    pub fn new(name: impl Into<String>, length: i64) -> Self {
        PackageEntry { name: name.into(), length }
    }
}

/// Denetimin sonucu: hata yoksa `ExtensionInstallError::None`, hatalıysa ilk sorunlu girdi.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageCheck {
    pub error: ExtensionInstallError,
    pub entry: Option<String>,
}

impl PackageCheck {
    /// `new PackageCheck(error, entry = null)`.
    pub fn new(error: ExtensionInstallError, entry: Option<String>) -> Self {
        PackageCheck { error, entry }
    }

    pub fn is_ok(&self) -> bool {
        self.error == ExtensionInstallError::None
    }

    /// `PackageCheck.Ok`.
    pub fn ok() -> PackageCheck {
        PackageCheck { error: ExtensionInstallError::None, entry: None }
    }

    fn error(error: ExtensionInstallError) -> PackageCheck {
        PackageCheck { error, entry: None }
    }

    fn at(error: ExtensionInstallError, entry: &str) -> PackageCheck {
        PackageCheck { error, entry: Some(entry.to_string()) }
    }
}

/// Paket (.koyext, zip) girdilerinin saf kuralı: hiçbir dosyaya dokunmaz, yalnız ad ve boyut listesine bakar.
/// Zip slip (mutlak yol, "..", sürücü harfi, ters bölü, ":" yani alternatif veri akışı, boş ad), sayı ve boyut
/// sınırları, türe göre izinli dosya kümesi ve zorunlu dosyalar burada denetlenir. Büyük/küçük harfi farklı
/// aynı ad (Windows aynı dosyadır) tekrar sayılır.
pub struct PackageEntryPolicy;

impl PackageEntryPolicy {
    pub const MAX_DOWNLOAD_BYTES: i64 = 25 * 1024 * 1024;
    pub const MAX_EXTRACTED_BYTES: i64 = 50 * 1024 * 1024;
    pub const MAX_FILES: usize = 512;
    pub const MAX_ENTRY_NAME_LENGTH: usize = 200;

    pub const MAX_MANIFEST_BYTES: i64 = 16 * 1024;
    pub const MAX_PREVIEW_BYTES: i64 = 2 * 1024 * 1024;
    pub const MAX_ICON_BYTES: i64 = 2 * 1024 * 1024;

    pub const MANIFEST_NAME: &'static str = "manifest.json";
    pub const PREVIEW_NAME: &'static str = "preview.png";
    pub const THEME_NAME: &'static str = "theme.json";
    pub const PROFILE_NAME: &'static str = "profile.json";
    pub const ICONS_FOLDER: &'static str = "icons";
    pub const BACKGROUND_STEM: &'static str = "background";

    const ICON_EXTENSIONS: [&'static str; 3] = [".png", ".ico", ".svg"];

    /// Tür bilinmeden yapılabilen denetim: indirme boyutu, girdi sayısı, toplam açılmış boyut, ad güvenliği,
    /// tekrar ve manifest varlığı. Dosya yüklemede önce bu, sonra manifest okunup [`PackageEntryPolicy::check`] çağrılır.
    pub fn check_common(entries: &[PackageEntry], download_size: i64) -> PackageCheck {
        if download_size > Self::MAX_DOWNLOAD_BYTES {
            return PackageCheck::error(ExtensionInstallError::DownloadTooLarge);
        }

        if entries.len() > Self::MAX_FILES {
            return PackageCheck::error(ExtensionInstallError::TooManyFiles);
        }

        let mut total: i64 = 0;
        // OrdinalIgnoreCase: büyük harfe çevrilmiş ad anahtardır.
        let mut seen: HashSet<String> = HashSet::new();
        let mut has_manifest = false;
        for entry in entries {
            let name = entry.name.as_str();
            let is_directory = name.ends_with('/');
            let checked = if is_directory { &name[..name.len() - 1] } else { name };
            if !Self::is_safe_name(Some(checked)) {
                return PackageCheck::at(ExtensionInstallError::UnsafePath, name);
            }

            if entry.length < 0 {
                return PackageCheck::at(ExtensionInstallError::InvalidArchive, name);
            }

            if !seen.insert(text::to_upper_invariant(name)) {
                return PackageCheck::at(ExtensionInstallError::DuplicateEntry, name);
            }

            // C# `long` toplamı: sınır 50 MB olduğundan taşma yok.
            total += entry.length;
            if total > Self::MAX_EXTRACTED_BYTES {
                return PackageCheck::at(ExtensionInstallError::ExtractedTooLarge, name);
            }

            if name == Self::MANIFEST_NAME {
                has_manifest = true;
            }
        }

        if has_manifest { PackageCheck::ok() } else { PackageCheck::error(ExtensionInstallError::MissingManifest) }
    }

    /// Tam denetim: [`PackageEntryPolicy::check_common`] artı türe göre izinli dosyalar, tek tek boyut sınırları ve zorunlu dosya.
    /// `manifest.json` ve `preview.png` her türde serbest.
    pub fn check(entries: &[PackageEntry], kind: ExtensionKind, download_size: i64) -> PackageCheck {
        let common = Self::check_common(entries, download_size);
        if !common.is_ok() {
            return common;
        }

        let mut payloads = 0;
        for entry in entries {
            let name = entry.name.as_str();
            if name == Self::MANIFEST_NAME {
                if entry.length > Self::MAX_MANIFEST_BYTES {
                    return PackageCheck::at(ExtensionInstallError::FileTooLarge, name);
                }

                continue;
            }

            if name == Self::PREVIEW_NAME {
                if entry.length > Self::MAX_PREVIEW_BYTES {
                    return PackageCheck::at(ExtensionInstallError::FileTooLarge, name);
                }

                continue;
            }

            // Klasör girdisi (Gezgin ekler) yalnız simge paketinin "icons/" klasörü için serbest.
            if name.ends_with('/') {
                if kind == ExtensionKind::IconPack && name == format!("{}/", Self::ICONS_FOLDER) {
                    continue;
                }

                return PackageCheck::at(ExtensionInstallError::DisallowedFile, name);
            }

            match kind {
                ExtensionKind::Theme if name == Self::THEME_NAME => {
                    if entry.length > i64::from(UserStyleParser::MAX_BYTES) {
                        return PackageCheck::at(ExtensionInstallError::FileTooLarge, name);
                    }

                    payloads += 1;
                }

                ExtensionKind::Profile if name == Self::PROFILE_NAME => {
                    if entry.length > ProfileLibrary::MAX_FILE_BYTES {
                        return PackageCheck::at(ExtensionInstallError::FileTooLarge, name);
                    }

                    payloads += 1;
                }

                ExtensionKind::Background if Self::is_background_name(name) => {
                    payloads += 1;
                }

                ExtensionKind::IconPack if Self::is_icon_path(name) => {
                    if entry.length > Self::MAX_ICON_BYTES {
                        return PackageCheck::at(ExtensionInstallError::FileTooLarge, name);
                    }

                    payloads += 1;
                }

                _ => return PackageCheck::at(ExtensionInstallError::DisallowedFile, name),
            }
        }

        if kind == ExtensionKind::Background && payloads > 1 {
            return PackageCheck::error(ExtensionInstallError::MultiplePayloads);
        }

        // Tema ve profil tek dosya: ad sabit ve tekrar zaten reddedildi, yani payloads 0 ya da 1.
        if payloads == 0 { PackageCheck::error(ExtensionInstallError::MissingPayload) } else { PackageCheck::ok() }
    }

    /// Zip girdi adı güvenli mi (klasör girdisinde sondaki "/" atılmış hâli verilir): boş değil, kısa, denetim
    /// karakteri yok; mutlak yol, sürücü harfi/":" (ADS), ters bölü, "." ve ".." bileşeni, boş bileşen,
    /// nokta/boşlukla biten bileşen yok. Zip yolu ayırıcısı yalnız "/".
    pub fn is_safe_name(name: Option<&str>) -> bool {
        let Some(name) = name else {
            return false;
        };

        // C# `Length`: UTF-16 birimi.
        if name.is_empty() || name.encode_utf16().count() > Self::MAX_ENTRY_NAME_LENGTH {
            return false;
        }

        if name.chars().any(|c| c.is_control() || matches!(c, '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')) {
            return false;
        }

        for part in name.split('/') {
            if part.is_empty() || part == "." || part == ".." || part.ends_with(['.', ' ']) {
                return false;
            }
        }

        true
    }

    /// "background.gif|webp|mp4|webm" (küçük harf; kök klasörde).
    pub fn is_background_name(name: &str) -> bool {
        let dot = match name.rfind('.') {
            Some(dot) if dot > 0 => dot,
            _ => return false,
        };
        if name.contains('/') {
            return false;
        }

        &name[..dot] == Self::BACKGROUND_STEM && LiveBackgroundFileValidator::allowed_extensions().contains(&&name[dot..])
    }

    /// "icons/<dosya>" ve uzantı .png/.ico/.svg; alt klasör ve aygıt adı yok (IconPackFiles kuralı).
    pub fn is_icon_path(name: &str) -> bool {
        let prefix = format!("{}/", Self::ICONS_FOLDER);
        let Some(file) = name.strip_prefix(prefix.as_str()) else {
            return false;
        };

        if AppIconOverrides::try_validate_pack_file(Some(file)).is_none() {
            return false;
        }

        let extension = win_path::get_extension(file);
        Self::ICON_EXTENSIONS.iter().any(|e| text::eq_ignore_case(e, extension))
    }
}
