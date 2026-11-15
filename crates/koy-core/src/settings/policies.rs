//! Karşılığı: `koy/src/Core/Settings/FirstRunPolicy.cs`, `FeatureStateSyncPolicy.cs`, `ApplyRevertPolicy.cs`.

use super::{AppSettings, DockSettings, GeneralSettings, NotchSettings};
use crate::time::{DateTimeOffset, TimeSpan, span_between};

/// İlk kurulum sihirbazı tamamlanmadan (ör. X ile kapatılınca) uygulamanın kullanıcının görev çubuğuna dokunmaması için
/// güvenli varsayılanlar. Yalnız bellekteki ayara uygulanır, diske yazılmaz: sihirbaz bir sonraki açılışta yeniden çıkar.
pub struct FirstRunPolicy;

impl FirstRunPolicy {
    /// Sihirbaz tamamlandıysa ayar değişmeden (kopya) döner. C# `Assert.Same` yerine eşitlik.
    pub fn apply_safe_defaults_if_incomplete(settings: &AppSettings) -> AppSettings {
        if settings.general.first_run_completed {
            return settings.clone();
        }

        // Dock görev çubuğunu gizler (AGENTS.md #12); kullanıcı henüz seçim yapmadığı için kapalı kalır.
        AppSettings {
            dock: DockSettings { enabled: false, hide_windows_taskbar: false, ..settings.dock.clone() },
            ..settings.clone()
        }
    }

    /// "Ayarları sıfırla" için varsayılan ayarlar. Kurulum sihirbazı zaten tamamlandığından `FirstRunCompleted`
    /// korunur; yoksa sıfırlama sihirbazı yeniden açardı.
    pub fn create_reset(current: &AppSettings) -> AppSettings {
        AppSettings {
            general: GeneralSettings { first_run_completed: current.general.first_run_completed, ..GeneralSettings::default() },
            ..AppSettings::default()
        }
    }
}

/// Harici kaynaklardan (sağ alt menü, kısayollar) gelen çentik/dock durumlarını mevcut ayar durumuyla birleştirir;
/// bayat değerlerin diske yazılmasını önler.
pub struct FeatureStateSyncPolicy;

impl FeatureStateSyncPolicy {
    /// Verilen hedef ayar nesnesini, harici olarak sağlanan özellik durumlarıyla uzlaştırır.
    /// Belirtilmeyen (`None`) durumlar için mevcut hedef değerler korunur.
    pub fn reconcile(current: &AppSettings, notch_enabled: Option<bool>, dock_enabled: Option<bool>) -> AppSettings {
        AppSettings {
            notch: NotchSettings { enabled: notch_enabled.unwrap_or(current.notch.enabled), ..current.notch.clone() },
            dock: DockSettings { enabled: dock_enabled.unwrap_or(current.dock.enabled), ..current.dock.clone() },
            ..current.clone()
        }
    }
}

/// Kullanıcının riskli ayar için verdiği yanıt; yoksa `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ApplyRevertChoice {
    #[default]
    None,
    Confirm,
    Revert,
}

/// Riskli ayarın ne olacağı: beklemeye devam, değeri tut ya da önceki değere dön.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ApplyRevertDecision {
    #[default]
    Wait,
    Keep,
    Revert,
}

/// Riskli bir ayar (dock konumu, görev çubuğu gizleme, monitör ataması) uygulandıktan sonra, kullanıcı onaylamazsa
/// önceki değere dönme kararı. Saf: zaman ve kullanıcı yanıtı dışarıdan verilir, zamanlayıcı App'te kurulur.
/// Açık onay her zaman zaman aşımından önce gelir; onay yoksa süre dolunca geri alınır.
pub struct ApplyRevertPolicy;

impl ApplyRevertPolicy {
    /// Onay için varsayılan süre (~15 sn). (C# `static readonly` → sabit.)
    pub const DEFAULT_TIMEOUT: TimeSpan = TimeSpan::from_secs(15);

    pub fn decide(applied_at: DateTimeOffset, now: DateTimeOffset, choice: ApplyRevertChoice, timeout: TimeSpan) -> ApplyRevertDecision {
        Self::validate_timeout(timeout);

        if choice == ApplyRevertChoice::Confirm {
            return ApplyRevertDecision::Keep;
        }

        if choice == ApplyRevertChoice::Revert {
            return ApplyRevertDecision::Revert;
        }

        if span_between(now, applied_at) >= timeout { ApplyRevertDecision::Revert } else { ApplyRevertDecision::Wait }
    }

    /// Onay penceresinde kalan süre; dolmuşsa sıfır.
    pub fn remaining(applied_at: DateTimeOffset, now: DateTimeOffset, timeout: TimeSpan) -> TimeSpan {
        Self::validate_timeout(timeout);

        // Saat geriye giderse (now < appliedAt) kalan süre zaman aşımını aşmasın. Çıkarmadan önce bakılır:
        // çok büyük zaman aşımında timeout - (negatif) TimeSpan'i taşırırdı.
        let elapsed = span_between(now, applied_at);
        if elapsed <= TimeSpan::ZERO {
            return timeout;
        }

        let left = timeout - elapsed;
        if left > TimeSpan::ZERO { left } else { TimeSpan::ZERO }
    }

    /// C# `ArgumentOutOfRangeException` → panik (programcı hatası).
    fn validate_timeout(timeout: TimeSpan) {
        if timeout <= TimeSpan::ZERO {
            panic!("Zaman aşımı pozitif olmalı. (timeout)");
        }
    }
}
