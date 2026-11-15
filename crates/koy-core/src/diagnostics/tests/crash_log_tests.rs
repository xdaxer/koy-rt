//! Karşılığı: `tests/Core.Tests/CrashLogTests.cs`.

use std::backtrace::Backtrace;
use std::path::PathBuf;
use std::sync::Mutex;

use chrono::NaiveDate;

use crate::diagnostics::{CrashError, CrashLog};

// CrashLog.LogPath global: onu değiştiren testler sırayla koşar (C#'taki `[Collection("CrashLog.LogPath")]`;
// paralelde birbirinin yolunu eziyordu).
static LOG_PATH_COLLECTION: Mutex<()> = Mutex::new(());

fn temp_dir(prefix: &str) -> PathBuf {
    let unique = format!("{}-{:?}-{}", std::process::id(), std::thread::current().id(), line!());
    let unique: String = unique.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    std::env::temp_dir().join(format!("{prefix}{unique}"))
}

#[test]
fn format_zaman_tip_mesaj_ve_yigini_icerir() {
    // C#'ta yığın izi fırlatılan yerden gelir; burada hatanın oluştuğu yerde alınan geri izleme.
    let ex = CrashError::exception(
        "System.InvalidOperationException",
        "deneme mesaji",
        &Backtrace::force_capture().to_string(),
    );

    let now = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap().and_hms_opt(17, 11, 38).unwrap();
    let text = CrashLog::format("Dispatcher", &ex, now);

    assert!(text.contains("[2026-11-05 17:11:38] Dispatcher"));
    assert!(text.contains("System.InvalidOperationException"));
    assert!(text.contains("deneme mesaji"));
    assert!(text.contains("format_zaman_tip_mesaj_ve_yigini_icerir"), "yığın izi test adını içermeli: {text}");
}

#[test]
fn format_istisna_olmayan_nesneyi_yazar() {
    let text = CrashLog::format("AppDomain", &CrashError::object("garip nesne"), chrono::Local::now().naive_local());
    assert!(text.contains("garip nesne"));
}

#[test]
fn format_bos_nesne_icin_bos_yazar() {
    let text = CrashLog::format("X", &CrashError::null(), chrono::Local::now().naive_local());
    assert!(text.contains("Nesne: (boş)"));
}

#[test]
fn write_to_olmayan_klasoru_olusturup_ekler() {
    let dir = temp_dir("koy-crashlog-");
    let path = dir.join("alt").join("crash.log");
    let path_str = path.to_string_lossy().into_owned();

    CrashLog::write_to(&path_str, "A", &CrashError::exception("System.InvalidOperationException", "bir", ""));
    CrashLog::write_to(&path_str, "B", &CrashError::exception("System.InvalidOperationException", "iki", ""));

    let content = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(content.contains("bir"));
    assert!(content.contains("iki"));
}

#[test]
fn write_to_gecersiz_yol_istisna_firlatmaz() {
    CrashLog::write_to("Z:\\olmayan\\\0\\crash.log", "X", &CrashError::exception("System.InvalidOperationException", "x", ""));
}

#[test]
fn guard_istisnayi_yutar_ve_loga_yazar() {
    let _serial = LOG_PATH_COLLECTION.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp_dir("koy-crashlog-");
    let path = dir.join("crash.log");
    let old = CrashLog::log_path();

    CrashLog::set_log_path(&path.to_string_lossy());
    let mut ran = false;
    CrashLog::guard("test", || {
        ran = true;
        panic!("sizmamali");
    });
    let content = std::fs::read_to_string(&path);

    CrashLog::set_log_path(&old);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(ran);
    assert!(content.unwrap().contains("sizmamali"));
}
