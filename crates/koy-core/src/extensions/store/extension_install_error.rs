//! Karşılığı: `Extensions/Store/ExtensionInstallError.cs`.

use serde::{Deserialize, Serialize};

use super::InstalledEntry;

/// Paket denetimi ve kurulumunun hata nedeni. Metin değil kod: çeviri App'te yapılır.
/// Hata yoksa `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ExtensionInstallError {
    #[default]
    None,

    // Ağ ve indirme
    Network,
    Timeout,
    Cancelled,
    DownloadTooLarge,
    HashMismatch,
    FileNotFound,

    // Arşiv ve giriş denetimi
    InvalidArchive,
    UnsafePath,
    DuplicateEntry,
    TooManyFiles,
    ExtractedTooLarge,
    FileTooLarge,
    DisallowedFile,
    MissingManifest,
    MissingPayload,
    MultiplePayloads,

    // Manifest
    InvalidManifest,
    ManifestMismatch,
    CapabilitiesNotAllowed,

    // İçerik
    InvalidPayload,

    // Kayıt ve disk
    TooManyInstalled,
    KindConflict,
    IoError,
}

/// Kurulum sonucu. Başarıda `entry` kayda yazılan öğedir.
/// `previously_applied`: güncelleme ise ve eski sürüm uygulanmıştı: yeni dosyalar yerinde ama ayara/kullanıcı
/// dosyasına yansımadı, App "Uygula"yı yeniden çalıştırmalı (arka plan uzantısı değişmiş olabilir).
#[derive(Debug, Clone, PartialEq)]
pub struct InstallResult {
    pub error: ExtensionInstallError,
    pub entry: Option<InstalledEntry>,
    pub previously_applied: bool,
}

impl InstallResult {
    /// `new InstallResult(error, entry, previouslyApplied = false)`.
    pub fn new(error: ExtensionInstallError, entry: Option<InstalledEntry>, previously_applied: bool) -> Self {
        InstallResult { error, entry, previously_applied }
    }

    pub fn is_success(&self) -> bool {
        self.error == ExtensionInstallError::None && self.entry.is_some()
    }

    pub fn fail(error: ExtensionInstallError) -> Self {
        InstallResult { error, entry: None, previously_applied: false }
    }
}
