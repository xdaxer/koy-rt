//! Karşılığı: `Extensions/Store/ExtensionPackageInstaller.cs`.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, TimeDelta, Utc};

use super::zip_reader::{ZipArchive, ZipError};
use super::{
    ExtensionInstallError, ExtensionKind, ExtensionPaths, ExtensionSource, InstallResult, InstalledEntry, InstalledRegistryError,
    InstalledRegistryStore, PackageEntry, PackageEntryPolicy, PackageManifestParser, StoreItem,
};
use crate::appearance::UserStyleParser;
use crate::extensions::InstalledExtensionList;
use crate::settings::SettingsProfileCodec;
use crate::text;
use crate::time::DateTimeOffset;
use crate::win_path;

/// Paket (zip) açma ve kurma mantığı; ağ yok, bu yüzden geçici klasörde test edilir. Sıra: giriş listesi denetimi
/// ([`PackageEntryPolicy`]) → manifest → tür denetimi → geçici klasöre sınırlı açma → içerik doğrulama →
/// eski sürümü yedekleyip tek klasör taşımasıyla `extensions\<id>` → kayıt. Hatada geçici klasör silinir; yarım
/// kurulum kalmaz. Asla beklenen hatada fırlatmaz.
pub struct ExtensionPackageInstaller {
    extensions_folder: PathBuf,
    registry: Arc<InstalledRegistryStore>,
    clock: Box<dyn Fn() -> DateTimeOffset + Send + Sync>,
}

/// Bir saatten eski yarım kalmış geçici öğeler silinir.
const STALE_AFTER_HOURS: i64 = 1;

impl ExtensionPackageInstaller {
    pub const TEMP_PREFIX: &'static str = ".tmp-";
    pub const OLD_PREFIX: &'static str = ".old-";

    /// `clock`: `None` iken `DateTimeOffset.UtcNow`.
    pub fn new(
        extensions_folder: impl Into<PathBuf>,
        registry: Arc<InstalledRegistryStore>,
        clock: Option<Box<dyn Fn() -> DateTimeOffset + Send + Sync>>,
    ) -> Self {
        ExtensionPackageInstaller {
            extensions_folder: extensions_folder.into(),
            registry,
            clock: clock.unwrap_or_else(|| Box::new(|| Utc::now().fixed_offset())),
        }
    }

    pub fn extensions_folder(&self) -> &Path {
        &self.extensions_folder
    }

    /// `zip` aranabilir bir akış olmalı. `expected` doluysa (mağaza kurulumu) paketin kimlik/sürüm/türü indeksle
    /// aynı olmalı; boşsa (dosyadan) manifest kendi başına doğrulanır. `download_size` paketin bayt boyutu.
    pub fn install<R: Read + Seek>(&self, zip: R, download_size: i64, expected: Option<&StoreItem>, source: ExtensionSource) -> InstallResult {
        // C# `finally`: taşınmamış geçici klasör ne olursa olsun silinir.
        let mut temp: Option<PathBuf> = None;
        let result = self.install_core(zip, download_size, expected, source, &mut temp);
        if let Some(temp) = temp {
            try_delete(&temp);
        }

        match result {
            Ok(result) => result,
            Err(error) => InstallResult::fail(error),
        }
    }

    fn install_core<R: Read + Seek>(
        &self,
        zip: R,
        download_size: i64,
        expected: Option<&StoreItem>,
        source: ExtensionSource,
        temp_slot: &mut Option<PathBuf>,
    ) -> Result<InstallResult, ExtensionInstallError> {
        if download_size > PackageEntryPolicy::MAX_DOWNLOAD_BYTES {
            return Err(ExtensionInstallError::DownloadTooLarge);
        }

        io_result(fs::create_dir_all(&self.extensions_folder))?;
        self.cleanup_stale();

        let mut archive = zip_result(ZipArchive::new(zip))?;
        let entries: Vec<PackageEntry> = archive.entries().iter().map(|e| PackageEntry::new(e.full_name.clone(), e.length)).collect();

        let common = PackageEntryPolicy::check_common(&entries, download_size);
        if !common.is_ok() {
            return Err(common.error);
        }

        // `check_common` manifest.json girdisinin varlığını doğruladı.
        let manifest_index = archive.get_entry(PackageEntryPolicy::MANIFEST_NAME).ok_or(ExtensionInstallError::MissingManifest)?;
        let Some(manifest_bytes) = read_entry(&mut archive, manifest_index, PackageEntryPolicy::MAX_MANIFEST_BYTES)? else {
            return Err(ExtensionInstallError::FileTooLarge);
        };

        let parsed = PackageManifestParser::parse_bytes(&manifest_bytes);
        if !parsed.is_valid() {
            return Err(parsed.error);
        }

        let package = parsed.manifest.ok_or(ExtensionInstallError::InvalidManifest)?;
        if let Some(expected) = expected {
            let matched = PackageManifestParser::match_item(&package, expected);
            if matched != ExtensionInstallError::None {
                return Err(matched);
            }
        }

        let check = PackageEntryPolicy::check(&entries, package.kind, download_size);
        if !check.is_ok() {
            return Err(check.error);
        }

        let id = package.manifest.id().to_string();
        let previous = self.registry.load().find(Some(&id)).cloned();
        if previous.as_ref().is_some_and(|p| p.kind != package.kind) {
            return Err(ExtensionInstallError::KindConflict);
        }

        // Sınır dolmuşsa açmaya başlamadan söyle (yeni kimlik için).
        if previous.is_none() && self.registry.load().entries().len() >= InstalledExtensionList::MAX_INSTALLED {
            return Err(ExtensionInstallError::TooManyInstalled);
        }

        let temp = self.extensions_folder.join(format!("{}{}", Self::TEMP_PREFIX, new_guid_n()));
        *temp_slot = Some(temp.clone());
        io_result(fs::create_dir_all(&temp))?;

        Self::extract(&mut archive, &temp)?;

        let payload_file = Self::validate_payload(package.kind, &entries, &temp)?;

        let entry = InstalledEntry {
            id: id.clone(),
            version: package.manifest.version().to_string(),
            kind: package.kind,
            name: package.manifest.name().to_string(),
            author: package.manifest.author().to_string(),
            license: package.license.clone(),
            installed_at: (self.clock)(),
            source,
            applied: previous.as_ref().is_some_and(|p| p.applied),
            payload_file,
        };

        self.replace(&temp, &id)?;
        *temp_slot = None; // taşındı, temizlenecek bir şey yok

        let mut error = InstalledRegistryError::None;
        let saved = self.registry.update(|current| match current.try_upsert(entry.clone()) {
            Ok(next) => Some(if entry.applied { next.with_applied(&id) } else { next }),
            Err(e) => {
                error = e;
                None
            }
        });

        if !saved {
            // Kayıt yazılamadı: kayıtsız klasör yetim kalmasın.
            // C# ile aynı: güncellemede de yeni klasör silinir; önceki kayıt (varsa) eksik klasörü gösterir kalır.
            try_delete(&self.extensions_folder.join(&id));
            return Err(if error == InstalledRegistryError::TooManyExtensions {
                ExtensionInstallError::TooManyInstalled
            } else {
                ExtensionInstallError::IoError
            });
        }

        let previously_applied = previous.as_ref().is_some_and(|p| p.applied);
        Ok(InstallResult::new(ExtensionInstallError::None, Some(entry), previously_applied))
    }

    /// Yalnız kayıttaki kimliğin klasörünü siler (yol [`ExtensionPackageInstaller::extensions_folder`]'ın kesin altında olmalı) ve
    /// kayıttan çıkarır. Kayıtta yoksa ya da silinemezse false; silinemezse kayıt korunur.
    pub fn uninstall(&self, id: &str) -> bool {
        let Some(folder) = ExtensionPaths::folder_for(Some(id), Some(&self.extensions_folder)) else {
            return false;
        };
        if self.registry.load().find(Some(id)).is_none() || !ExtensionPaths::is_strictly_under(&self.extensions_folder, &folder) {
            return false;
        }

        if folder.is_dir() && fs::remove_dir_all(&folder).is_err() {
            return false;
        }

        self.registry.update(|current| Some(current.remove(id)))
    }

    /// Yarım kalmış (çökme, süreç öldürme) geçici klasör ve dosyaları, bir saatten eskiyse siler.
    pub fn cleanup_stale(&self) {
        // En iyi çaba: temizlik başarısızlığı kurulumu durdurmaz.
        let Ok(read_dir) = fs::read_dir(&self.extensions_folder) else {
            return;
        };

        for item in read_dir {
            let Ok(item) = item else {
                return;
            };
            let name = item.file_name().to_string_lossy().into_owned();
            if !name.starts_with(Self::TEMP_PREFIX) && !name.starts_with(Self::OLD_PREFIX) {
                continue;
            }

            let path = item.path();
            let Ok(modified) = fs::metadata(&path).and_then(|m| m.modified()) else {
                continue;
            };
            let last_write: DateTime<Utc> = modified.into();
            if (self.clock)().signed_duration_since(last_write) > TimeDelta::hours(STALE_AFTER_HOURS) {
                try_delete(&path);
            }
        }
    }

    // Sınırlı kopya: bildirilen boyuta ve toplam sınıra bakılmaz sadece, gerçekten okunan bayt sayılır.
    // Zip başlığı yalan söyleyebilir (zip bombası), bu yüzden sınır aşılınca akış kesilir.
    fn extract<R: Read + Seek>(archive: &mut ZipArchive<R>, temp: &Path) -> Result<(), ExtensionInstallError> {
        let temp_text = temp.to_string_lossy().into_owned();
        let root = match win_path::get_full_path(&temp_text) {
            Some(full) => format!("{}\\", full.trim_end_matches('\\')),
            None => return Err(ExtensionInstallError::UnsafePath),
        };
        let mut total: i64 = 0;
        let mut buffer = vec![0u8; 81920];

        for index in 0..archive.entries().len() {
            let (name, declared_length) = {
                let entry = &archive.entries()[index];
                (entry.full_name.clone(), entry.length)
            };
            let relative = name.trim_end_matches('/').replace('/', "\\");
            let target_text = match win_path::get_full_path(&win_path::combine(&temp_text, &relative)) {
                Some(full) => full,
                None => return Err(ExtensionInstallError::UnsafePath),
            };

            // Derinlemesine savunma: ilke zaten ".." ve mutlak yolu reddetti.
            if !text::starts_with_ignore_case(&target_text, &root) {
                return Err(ExtensionInstallError::UnsafePath);
            }

            let target = PathBuf::from(&target_text);
            if name.ends_with('/') {
                io_result(fs::create_dir_all(&target))?;
                continue;
            }

            if let Some(parent) = target.parent() {
                io_result(fs::create_dir_all(parent))?;
            }

            let mut input = zip_result(archive.open(index))?;
            let mut output = io_result(OpenOptions::new().write(true).create_new(true).open(&target))?;
            let mut written: i64 = 0;
            loop {
                let read = match input.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(error) => return Err(read_error(error)),
                };

                written += read as i64;
                total += read as i64;
                if written > declared_length || total > PackageEntryPolicy::MAX_EXTRACTED_BYTES {
                    return Err(if total > PackageEntryPolicy::MAX_EXTRACTED_BYTES {
                        ExtensionInstallError::ExtractedTooLarge
                    } else {
                        ExtensionInstallError::InvalidArchive
                    });
                }

                io_result(output.write_all(&buffer[..read]))?;
            }
        }

        Ok(())
    }

    /// Türe özgü içerik doğrulaması (açılmış dosya üzerinde). Tema: UserStyleParser (dosya düzeyi hata ya da hiç
    /// geçerli giriş yok → ret; tek tek bilinmeyen anahtar tanıları kabul, uygulamada yok sayılır). Profil:
    /// SettingsProfileCodec. Görseller: dosya imzası (magic bytes), uzantıya güvenilmez.
    /// Başarıda kayda yazılacak `payload_file` döner.
    fn validate_payload(kind: ExtensionKind, entries: &[PackageEntry], folder: &Path) -> Result<String, ExtensionInstallError> {
        match kind {
            ExtensionKind::Theme => {
                let payload_file = PackageEntryPolicy::THEME_NAME;
                let Some(content) = read_strict_utf8(&folder.join(payload_file)) else {
                    return Err(ExtensionInstallError::InvalidPayload);
                };

                let result = UserStyleParser::parse(&content);
                let file_level = result.diagnostics.iter().any(|d| d.key == UserStyleParser::FILE_KEY);
                if file_level || result.valid_entries.is_empty() {
                    Err(ExtensionInstallError::InvalidPayload)
                } else {
                    Ok(payload_file.to_string())
                }
            }

            ExtensionKind::Profile => {
                let payload_file = PackageEntryPolicy::PROFILE_NAME;
                match read_strict_utf8(&folder.join(payload_file)) {
                    Some(content) if SettingsProfileCodec::read(&content).is_valid() => Ok(payload_file.to_string()),
                    _ => Err(ExtensionInstallError::InvalidPayload),
                }
            }

            ExtensionKind::Background => {
                // `check` tam bir arka plan dosyası olduğunu doğruladı.
                let name = entries
                    .iter()
                    .find(|e| PackageEntryPolicy::is_background_name(&e.name))
                    .map(|e| e.name.clone())
                    .ok_or(ExtensionInstallError::MissingPayload)?;
                if PackageContent::looks_like_background(&folder.join(&name), win_path::get_extension(&name)) {
                    Ok(name)
                } else {
                    Err(ExtensionInstallError::InvalidPayload)
                }
            }

            ExtensionKind::IconPack => {
                for entry in entries.iter().filter(|e| PackageEntryPolicy::is_icon_path(&e.name)) {
                    let path = folder.join(entry.name.replace('/', "\\"));
                    if !PackageContent::looks_like_icon(&path, win_path::get_extension(&entry.name)) {
                        return Err(ExtensionInstallError::InvalidPayload);
                    }
                }

                Ok(PackageEntryPolicy::ICONS_FOLDER.to_string())
            }
        }
    }

    // Eski sürüm silinmeden önce kenara alınır; yeni taşıma başarısızsa geri konur (eski sürüm kaybolmaz).
    fn replace(&self, temp: &Path, id: &str) -> Result<(), ExtensionInstallError> {
        let final_path = PathBuf::from(win_path::combine(&self.extensions_folder.to_string_lossy(), id));
        if !ExtensionPaths::is_strictly_under(&self.extensions_folder, &final_path) {
            return Err(ExtensionInstallError::UnsafePath);
        }

        let mut old: Option<PathBuf> = None;
        let moved = (|| -> std::io::Result<()> {
            if final_path.is_dir() {
                let target = self.extensions_folder.join(format!("{}{}", Self::OLD_PREFIX, new_guid_n()));
                fs::rename(&final_path, &target)?;
                old = Some(target);
            }

            fs::rename(temp, &final_path)
        })();

        if moved.is_err() {
            if let Some(old) = &old
                && !final_path.is_dir()
            {
                // Geri koyma da başarısız: eski klasör ".old-" adıyla kalır, bir sonraki kurulum temizler.
                let _ = fs::rename(old, &final_path);
            }

            return Err(ExtensionInstallError::IoError);
        }

        if let Some(old) = old {
            try_delete(&old);
        }

        Ok(())
    }
}

/// `limit` baytı aşarsa `None`; aksi halde girdinin bütün içeriği.
fn read_entry<R: Read + Seek>(archive: &mut ZipArchive<R>, index: usize, limit: i64) -> Result<Option<Vec<u8>>, ExtensionInstallError> {
    let mut input = zip_result(archive.open(index))?;
    let mut content: Vec<u8> = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        let read = match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(error) => return Err(read_error(error)),
        };

        if content.len() as i64 + read as i64 > limit {
            return Ok(None);
        }

        content.extend_from_slice(&buffer[..read]);
    }

    Ok(Some(content))
}

// Windows editörlerinin eklediği BOM ayrıştırıcıyı şaşırtmasın.
fn read_strict_utf8(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let skip = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { 3 } else { 0 };
    std::str::from_utf8(&bytes[skip..]).ok().map(str::to_string)
}

fn try_delete(path: &Path) {
    // En iyi çaba.
    if path.is_dir() {
        let _ = fs::remove_dir_all(path);
    } else if path.exists() {
        let _ = fs::remove_file(path);
    }
}

/// `Guid.NewGuid().ToString("N")`: 32 onaltılık hane.
fn new_guid_n() -> String {
    crate::random::get_bytes(16).iter().map(|b| format!("{b:02x}")).collect()
}

fn io_result<T>(result: std::io::Result<T>) -> Result<T, ExtensionInstallError> {
    result.map_err(|_| ExtensionInstallError::IoError)
}

// `InvalidDataException` → InvalidArchive; G/Ç hataları → IoError.
fn zip_result<T>(result: Result<T, ZipError>) -> Result<T, ExtensionInstallError> {
    result.map_err(|error| match error {
        ZipError::InvalidData(_) => ExtensionInstallError::InvalidArchive,
        ZipError::Io(_) => ExtensionInstallError::IoError,
    })
}

// Girdi akışından okuma hatası: bozuk deflate/erken biten veri arşiv hatasıdır, diğerleri G/Ç.
fn read_error(error: std::io::Error) -> ExtensionInstallError {
    zip_result::<()>(Err(ZipError::from(error))).err().unwrap_or(ExtensionInstallError::IoError)
}

/// Görsel/video dosyalarının imza denetimi: uzantı yalan söyleyebilir (ör. .png adlı çalıştırılabilir).
pub(crate) struct PackageContent;

impl PackageContent {
    pub fn looks_like_background(path: &Path, extension: &str) -> bool {
        let Some(head) = Self::head(path, 16) else {
            return false;
        };

        match extension {
            ".gif" => head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a"),
            ".webp" => head.len() >= 12 && head.starts_with(b"RIFF") && &head[8..12] == b"WEBP",
            ".mp4" => head.len() >= 12 && &head[4..8] == b"ftyp",
            ".webm" => head.len() >= 4 && head[0] == 0x1A && head[1] == 0x45 && head[2] == 0xDF && head[3] == 0xA3,
            _ => false,
        }
    }

    pub fn looks_like_icon(path: &Path, extension: &str) -> bool {
        let lower = text::to_lower_invariant(extension);
        if lower == ".svg" {
            // Betik ya da dış kaynak taşıyan SVG kabul edilmez; WPF çizici güvenli olsa da dosya paylaşılabilir.
            let Some(head_text) = Self::head(path, 4096) else {
                return false;
            };

            let content = String::from_utf8_lossy(&head_text);
            return text::contains_ignore_case(&content, "<svg") && !text::contains_ignore_case(&content, "<script");
        }

        let Some(head) = Self::head(path, 8) else {
            return false;
        };

        match lower.as_str() {
            ".png" => head.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
            ".ico" => head.len() >= 4 && head[0] == 0 && head[1] == 0 && head[2] == 1 && head[3] == 0,
            _ => false,
        }
    }

    fn head(path: &Path, count: usize) -> Option<Vec<u8>> {
        let file = File::open(path).ok()?;
        let mut buffer = Vec::with_capacity(count);
        file.take(count as u64).read_to_end(&mut buffer).ok()?;
        if buffer.is_empty() { None } else { Some(buffer) }
    }
}
