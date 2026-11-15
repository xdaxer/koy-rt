use crate::text;

/// Gözcünün Koy sürecini beklerken aldığı sonuç.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatchdogWaitResult {
    /// Koy süreci sonlandı (Görev Yöneticisi, çökme ya da normal çıkış).
    ProcessExited,

    /// Koy "temiz çıkış" işaretini bıraktı (dock durdu, görev çubuğunu kendisi geri getirdi).
    StopRequested,

    /// Gözcü başladığında Koy süreci zaten yoktu.
    ProcessNotFound,

    /// Bekleme beklenmedik biçimde başarısız oldu.
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatchdogAction {
    /// Hiçbir şey yapma, çık.
    None,

    /// Windows görev çubuğunu geri getir.
    RestoreTaskbar,
}

/// Gözcü sürecin (Koy.Watchdog.exe) saf kararları: argüman çözümü, ortak adlar, bekleme sonucundan eylem.
/// Win32 yok; hem Koy hem gözcü aynı adları kullanır.
pub struct WatchdogPolicy;

impl WatchdogPolicy {
    pub const PID_ARGUMENT: &'static str = "--pid";

    /// Aynı anda tek gözcü: ikinci örnek bu muteksi alamayınca hemen çıkar.
    pub const SINGLETON_MUTEX_NAME: &'static str = r"Local\Koy.Watchdog.Singleton";

    /// Koy'un temiz çıkış işareti için adlandırılmış olay (Koy PID'ine özel).
    pub fn stop_event_name(koy_process_id: i32) -> String {
        format!(r"Local\Koy.Watchdog.Stop.{koy_process_id}")
    }

    /// Gözcüye verilecek komut satırı argümanları.
    pub fn build_arguments(koy_process_id: i32) -> String {
        format!("{} {}", Self::PID_ARGUMENT, koy_process_id)
    }

    /// `--pid N` argümanını çözer. Eksik, sayı olmayan ya da pozitif olmayan değerde `None` döner
    /// (C#'taki `false` + `processId = 0`).
    pub fn try_parse_process_id<S: AsRef<str>>(args: &[S]) -> Option<i32> {
        // `for (i = 0; i < args.Count - 1; i++)`
        for i in 0..args.len().saturating_sub(1) {
            if !text::eq_ignore_case(args[i].as_ref(), Self::PID_ARGUMENT) {
                continue;
            }

            // `NumberStyles.None`: yalnız rakamlar (işaret ve boşluk yok); taşma da başarısızlıktır.
            let value = args[i + 1].as_ref();
            let parsed = if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
                value.parse::<i32>().ok()
            } else {
                None
            };
            return parsed.filter(|&id| id > 0);
        }

        None
    }

    /// Koy öldüyse (ya da hiç bulunamadıysa) görev çubuğu geri getirilir; temiz çıkışta ve beklenmedik
    /// bekleme hatasında dokunulmaz (hatada yanlışlıkla kullanıcının ayarını bozmamak güvenli taraftır).
    pub fn decide(result: WatchdogWaitResult) -> WatchdogAction {
        match result {
            WatchdogWaitResult::ProcessExited => WatchdogAction::RestoreTaskbar,
            WatchdogWaitResult::ProcessNotFound => WatchdogAction::RestoreTaskbar,
            _ => WatchdogAction::None,
        }
    }
}
