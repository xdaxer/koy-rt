//! Karşılığı: `Uninstall/UninstallPlan.cs`.

use std::path::PathBuf;

/// Uygulama içinden kaldırmanın mümkün olup olmadığı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UninstallAvailability {
    /// Velopack ile kurulu ve kaldırıcı (Update.exe) yerinde.
    Available,

    /// Kurulu değil (dotnet run, elle çıkarılmış klasör).
    NotInstalled,

    /// Taşınabilir paket: kaldırılacak bir kurulum yok, klasörü silmek yeter.
    Portable,

    /// Kurulu görünüyor ama Update.exe bulunamadı (bozuk kurulum).
    UpdaterMissing,
}

/// Kaldırmanın saf kararları: kaldırıcı komutu, neyin silineceği ve ne zaman mümkün olduğu.
/// Dosya silme işi `UninstallCleaner`'da, süreç başlatma App katmanındadır.
pub struct UninstallPlan;

impl UninstallPlan {
    /// Velopack kaldırıcısının argümanı. Kaynak: Windows "Yüklü uygulamalar" kaydındaki UninstallString
    /// (`"...\Update.exe" --uninstall`), sessiz varyantı ek olarak `--silent` alır.
    /// Sessiz değil: kaldırıcı kendi ilerleme ve "kaldırıldı" penceresini gösterir, kullanıcı bitişi görür.
    pub const UPDATER_ARGUMENTS: &'static str = "--uninstall";

    /// Kullanıcı tercihleri ve içeriği: yalnız "ayarlarımı sakla" kapalıysa silinir.
    pub const SETTING_FILES: &'static [&'static str] = &[
        "settings.json",
        "settings.json.tmp",
        "shelf.json",
        "events.json",
        "events.json.tmp",
        // Harici takvim: hesap ayarları, DPAPI ile şifreli belirteçler ve çevrimdışı etkinlik önbelleği.
        "calendar-settings.json",
        "calendar-settings.json.tmp",
        "calendar-token-google.dat",
        "calendar-token-google.dat.tmp",
        "calendar-token-outlook.dat",
        "calendar-token-outlook.dat.tmp",
        "calendar-cache.json",
        "calendar-cache.json.tmp",
    ];

    /// Çalışma durumu: yeniden kurulumda işe yaramaz, her kaldırmada silinir.
    pub const STATE_FILES: &'static [&'static str] = &["taskbar-state.json"];

    /// Tanı kayıtları: Velopack kök klasörüyle zaten gider; ayrıca hiçbir şey bırakılmaz.
    pub const DIAGNOSTIC_FILES: &'static [&'static str] = &["crash.log", "crash.log.old"];

    /// Windows'un "Yüklü uygulamalar"ından kaldırmada ayarlar silinmez: kullanıcı Koy'u yeniden kurarsa
    /// tercihleri korunur. Bu yolda onay ekranı yoktur (Velopack kancası 30 sn sınırlı ve iptal edilemez).
    pub const KEEP_SETTINGS_ON_EXTERNAL_UNINSTALL: bool = true;

    pub fn evaluate(
        is_installed: bool,
        is_portable: bool,
        update_exe_path: Option<&str>,
        file_exists: &dyn Fn(&str) -> bool,
    ) -> UninstallAvailability {
        if is_portable {
            return UninstallAvailability::Portable;
        }
        if !is_installed {
            return UninstallAvailability::NotInstalled;
        }
        match update_exe_path {
            Some(p) if !crate::text::is_null_or_white_space(Some(p)) && file_exists(p) => UninstallAvailability::Available,
            _ => UninstallAvailability::UpdaterMissing,
        }
    }

    /// Silinecek dosyaların tam yolları. Ayarlar %AppData%\Koy (roaming), tanı kayıtları %LOCALAPPDATA%\Koy altındadır;
    /// kurulu sürümde ikincisi Velopack'in kök klasörüdür, bu yüzden klasöre dokunulmaz, yalnız bilinen dosyalar silinir.
    pub fn files_to_delete(roaming_dir: &str, local_dir: &str, keep_settings: bool) -> Vec<String> {
        let mut files: Vec<String> = Vec::new();
        if !keep_settings {
            files.extend(Self::SETTING_FILES.iter().map(|f| path_combine(roaming_dir, f)));
        }
        files.extend(Self::STATE_FILES.iter().map(|f| path_combine(roaming_dir, f)));
        files.extend(Self::DIAGNOSTIC_FILES.iter().map(|f| path_combine(local_dir, f)));
        files
    }
}

/// `Path.Combine(a, b)`.
fn path_combine(a: &str, b: &str) -> String {
    if a.is_empty() {
        return b.to_string();
    }
    let mut p = PathBuf::from(a);
    p.push(b);
    p.to_string_lossy().into_owned()
}
