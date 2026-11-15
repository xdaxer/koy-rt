//! Karşılığı: `Diagnostics/CrashLog.cs`.

use std::fs;
use std::io::Write;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::NaiveDateTime;

const MAX_BYTES: u64 = 512 * 1024;

/// Dosya yazımı için kapı (C#'taki `lock (Gate)`).
static GATE: Mutex<()> = Mutex::new(());

/// `LogPath` özelliğinin değeri; `None` iken varsayılan yol geçerlidir.
static LOG_PATH: Mutex<Option<String>> = Mutex::new(None);

/// C# `Format(string, object? error, ...)` içindeki `error` değeri: istisna ya da başka bir nesne.
/// Rust'ta çalışma zamanı tür adı ve yığın izi olmadığı için çağıran bunları verir (bkz. [`CrashError::exception`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrashError {
    /// `error is Exception`: tür adı, `Message` ve `ex.ToString()` (iç istisnalar ve yığın dahil).
    Exception { type_name: String, message: String, details: String },
    /// İstisna olmayan nesne; `None` C#'taki `null` ("(boş)" yazılır).
    Object(Option<String>),
}

impl CrashError {
    /// Yığın izi verilen istisna. `details`, .NET'in `ex.ToString()` biçimidir: "Tip: mesaj" ve altında yığın.
    pub fn exception(type_name: &str, message: &str, stack_trace: &str) -> CrashError {
        let mut details = format!("{type_name}: {message}");
        if !stack_trace.is_empty() {
            details.push_str("\r\n");
            details.push_str(stack_trace);
        }
        CrashError::Exception { type_name: type_name.to_string(), message: message.to_string(), details }
    }

    /// `std::error::Error` değerinden: iç hatalar (`source()`) .NET'in `---> iç istisna` biçiminde eklenir.
    pub fn from_error(error: &(dyn std::error::Error + 'static), type_name: &str, stack_trace: &str) -> CrashError {
        let message = error.to_string();
        let mut details = format!("{type_name}: {message}");
        let mut inner = error.source();
        let has_inner = inner.is_some();
        while let Some(e) = inner {
            details.push_str(&format!(" ---> {e}"));
            inner = e.source();
        }
        if has_inner {
            details.push_str("\r\n   --- End of inner exception stack trace ---");
        }
        if !stack_trace.is_empty() {
            details.push_str("\r\n");
            details.push_str(stack_trace);
        }
        CrashError::Exception { type_name: type_name.to_string(), message, details }
    }

    /// İstisna olmayan nesne (`error.ToString()`).
    pub fn object(text: impl Into<String>) -> CrashError {
        CrashError::Object(Some(text.into()))
    }

    /// `null` nesne.
    pub fn null() -> CrashError {
        CrashError::Object(None)
    }
}

/// Yakalanmamış hataları `%LOCALAPPDATA%\Koy\crash.log` dosyasına yazar.
/// Hiçbir koşulda istisna fırlatmaz: log yazımı başarısız olursa sessizce vazgeçer ki çökme akışı bozulmasın.
pub struct CrashLog;

impl CrashLog {
    /// `DefaultPath`: `%LOCALAPPDATA%\Koy\crash.log` (ortam değişkeni yoksa C#'taki gibi boş klasör: `Koy\crash.log`).
    pub fn default_path() -> String {
        let base = std::env::var("LOCALAPPDATA").unwrap_or_default();
        path_combine(&path_combine(&base, "Koy"), "crash.log")
    }

    /// `LogPath` okuma: Write/Guard'ın yazdığı dosya. Testler geçici dosyaya yönlendirir.
    pub fn log_path() -> String {
        let mut guard = LOG_PATH.lock().unwrap_or_else(|e| e.into_inner());
        guard.get_or_insert_with(initial_log_path).clone()
    }

    /// `LogPath` atama.
    pub fn set_log_path(path: &str) {
        *LOG_PATH.lock().unwrap_or_else(|e| e.into_inner()) = Some(path.to_string());
    }

    /// Tek kayıt metni: zaman, kaynak, istisna tipi, mesaj ve yığın (iç istisnalar dahil).
    /// Satır sonları Windows'taki `Environment.NewLine` gibi `\r\n`.
    pub fn format(source: &str, error: &CrashError, now: NaiveDateTime) -> String {
        let mut sb = String::new();
        sb.push_str(&format!("[{}] {}\r\n", now.format("%Y-%m-%d %H:%M:%S"), source));

        match error {
            CrashError::Exception { type_name, message, details } => {
                sb.push_str(&format!("Tip: {type_name}\r\n"));
                sb.push_str(&format!("Mesaj: {message}\r\n"));
                sb.push_str("Yığın:\r\n");
                sb.push_str(details);
                sb.push_str("\r\n");
            }
            CrashError::Object(text) => {
                sb.push_str(&format!("Nesne: {}\r\n", text.as_deref().unwrap_or("(boş)")));
            }
        }

        sb.push_str("\r\n");
        sb
    }

    pub fn write(source: &str, error: &CrashError) {
        Self::write_to(&Self::log_path(), source, error);
    }

    pub fn write_to(path: &str, source: &str, error: &CrashError) {
        // Hata yutulur: log yazılamıyorsa yapılacak bir şey yok; çökme akışını bozma.
        let _ = try_write_to(path, source, error);
    }

    /// Win32 geri çağrıları (WndProc, WinEvent, zamanlayıcı) içinde çalıştır: hata (panik) loga yazılır, işletim
    /// sistemine sızmaz. Sızarsa süreç STATUS_FATAL_USER_CALLBACK_EXCEPTION ile düşer.
    pub fn guard(source: &str, action: impl FnOnce()) {
        if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(action)) {
            let message = if let Some(s) = payload.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "(panik iletisi yok)".to_string()
            };
            Self::write(source, &CrashError::exception("Panic", &message, ""));
        }
    }
}

fn try_write_to(path: &str, source: &str, error: &CrashError) -> std::io::Result<()> {
    let text = CrashLog::format(source, error, chrono::Local::now().naive_local());
    let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());

    if let Some(dir) = Path::new(path).parent() {
        if !dir.as_os_str().is_empty() {
            fs::create_dir_all(dir)?;
        }
    }

    // Dosya sonsuz büyümesin: eşik aşılınca eskisini .old yap.
    if let Ok(meta) = fs::metadata(path) {
        if meta.is_file() && meta.len() > MAX_BYTES {
            fs::rename(path, format!("{path}.old"))?;
        }
    }

    let mut file = fs::OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(text.as_bytes())
}

/// `Path.Combine(a, b)` (Windows): `a` boşsa `b`; `a` ayırıcıyla bitiyorsa doğrudan ekler.
fn path_combine(a: &str, b: &str) -> String {
    if a.is_empty() {
        return b.to_string();
    }
    let mut p = PathBuf::from(a);
    p.push(b);
    p.to_string_lossy().into_owned()
}

fn initial_log_path() -> String {
    // C# testlerinde `TestLogRedirect` (ModuleInitializer) tüm test derlemesini geçici dosyaya yönlendirir; kullanıcının
    // gerçek crash.log dosyasına test kayıtları düşmesin. Rust'ta modül başlatıcı olmadığı için `cfg(test)` ile aynı etki.
    #[cfg(test)]
    {
        std::env::temp_dir().join("koy-tests").join("crash.log").to_string_lossy().into_owned()
    }
    #[cfg(not(test))]
    {
        CrashLog::default_path()
    }
}
