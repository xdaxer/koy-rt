//! Geliştirici izi: `KOY_TRACE=1` ortam değişkeniyle açılır, `%TEMP%\koy-trace.log` dosyasına saniyede bir sayaç
//! özeti yazar. Kapalıyken (varsayılan) sayaç artırımı tek bir atomik işlemdir ve dosyaya hiç dokunulmaz.
//! C# karşılığı yok (yalnız Rust taşımasının canlı doğrulaması için).

use std::io::Write;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub static DOCK_REFRESH: AtomicU64 = AtomicU64::new(0);
pub static DOCK_RESIZE: AtomicU64 = AtomicU64::new(0);
pub static DOCK_FRAME: AtomicU64 = AtomicU64::new(0);
pub static DOCK_REGION: AtomicU64 = AtomicU64::new(0);
pub static DOCK_APPBAR: AtomicU64 = AtomicU64::new(0);
pub static DOCK_ITEMS: AtomicU64 = AtomicU64::new(0);
pub static DOCK_SHELL: AtomicU64 = AtomicU64::new(0);

pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("KOY_TRACE").is_ok_and(|v| v == "1"))
}

#[inline]
pub fn bump(c: &AtomicU64) {
    if enabled() {
        c.fetch_add(1, Ordering::Relaxed);
    }
}

/// Serbest satır (yalnız iz açıkken).
pub fn line(text: impl FnOnce() -> String) {
    if enabled() {
        write(&text());
    }
}

fn write(s: &str) {
    let path = std::env::temp_dir().join("koy-trace.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} {s}", chrono::Local::now().format("%H:%M:%S%.3f"));
    }
}

/// İz açıksa saniyede bir özet yazan zamanlayıcıyı kurar (arayüz iş parçacığında, olay döngüsü boyunca).
pub fn start() -> Option<slint::Timer> {
    if !enabled() {
        return None;
    }
    write("--- iz başladı ---");
    let t = slint::Timer::default();
    t.start(slint::TimerMode::Repeated, Duration::from_secs(1), || {
        let take = |c: &AtomicU64| c.swap(0, Ordering::Relaxed);
        write(&format!(
            "dock/sn: refresh={} resize={} frame={} region={} appbar={} items={} shell={}",
            take(&DOCK_REFRESH),
            take(&DOCK_RESIZE),
            take(&DOCK_FRAME),
            take(&DOCK_REGION),
            take(&DOCK_APPBAR),
            take(&DOCK_ITEMS),
            take(&DOCK_SHELL)
        ));
    });
    Some(t)
}
