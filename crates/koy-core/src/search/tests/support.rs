//! Testlerin ortak sahteleri: C# test dosyalarındaki `FakeShell` / `CountingShell` varyantlarının birleşimi.

use crate::search::{ISearchShell, SearchQuery, Translate};
use crate::text;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

/// `ISearchShell` sahtesi: dosya/klasör/ortam değişkenleri büyük/küçük harf duyarsız (C#'ta `OrdinalIgnoreCase`),
/// yapılan işler `log`'a düşer.
#[derive(Default)]
pub struct FakeShell {
    files: Mutex<HashSet<String>>,
    directories: Mutex<HashSet<String>>,
    env: Mutex<HashMap<String, String>>,
    log: Mutex<Vec<String>>,
    pub file_checks: AtomicI32,
    pub checked: Mutex<Vec<String>>,
}

impl FakeShell {
    pub fn new() -> Arc<FakeShell> {
        Arc::new(FakeShell::default())
    }

    pub fn add_file(&self, path: &str) {
        self.files.lock().unwrap().insert(text::to_upper_invariant(path));
    }

    pub fn add_directory(&self, path: &str) {
        self.directories.lock().unwrap().insert(text::to_upper_invariant(path));
    }

    pub fn set_env(&self, name: &str, value: &str) {
        self.env.lock().unwrap().insert(text::to_upper_invariant(name), value.to_string());
    }

    pub fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }

    pub fn file_checks(&self) -> i32 {
        self.file_checks.load(Ordering::SeqCst)
    }

    fn push(&self, entry: String) {
        self.log.lock().unwrap().push(entry);
    }
}

impl ISearchShell for FakeShell {
    fn file_exists(&self, path: &str) -> bool {
        self.file_checks.fetch_add(1, Ordering::SeqCst);
        self.checked.lock().unwrap().push(path.to_string());
        self.files.lock().unwrap().contains(&text::to_upper_invariant(path))
    }

    fn directory_exists(&self, path: &str) -> bool {
        self.directories.lock().unwrap().contains(&text::to_upper_invariant(path))
    }

    fn get_environment_variable(&self, name: &str) -> Option<String> {
        self.env.lock().unwrap().get(&text::to_upper_invariant(name)).cloned()
    }

    fn open(&self, target: &str) {
        self.push(format!("open {target}"));
    }

    fn run(&self, executable: &str, arguments: Option<&str>) {
        self.push(format!("run {} {}", executable, arguments.unwrap_or("")).trim_end().to_string());
    }

    fn run_with_arguments(&self, executable: &str, arguments: &[String]) {
        self.push(format!("run {} [{}]", executable, arguments.join("|")));
    }

    fn run_in_terminal(&self, command_line: &str) {
        self.push(format!("term {command_line}"));
    }

    fn open_containing_folder(&self, path: &str) {
        self.push(format!("folder {path}"));
    }

    fn copy_text(&self, text: &str) {
        self.push(format!("copy {text}"));
    }
}

/// `new SearchQuery(text)`.
pub fn q(text: &str) -> SearchQuery {
    SearchQuery::from_text(text)
}

/// Çeviri işlevi sahtesi.
pub fn translate(f: impl Fn(&str) -> String + Send + Sync + 'static) -> Translate {
    Arc::new(f)
}

/// Anahtarı olduğu gibi döndüren `T`.
pub fn identity_t() -> Translate {
    translate(|k| k.to_string())
}

/// Geçici klasör (C# testlerinin `IDisposable` temizliği).
pub struct TempDir(pub std::path::PathBuf);

impl TempDir {
    pub fn new(prefix: &str) -> Self {
        TempDir(std::env::temp_dir().join(format!("{}{}", prefix, crate::clipboard::Guid::new_guid())))
    }

    pub fn path(&self, name: &str) -> String {
        self.0.join(name).to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

/// `Task.WaitAsync(timeout)` ile beklenen `TaskCompletionSource` karşılığı: `Changed` olayında işaretlenir.
pub struct Signal {
    inner: Arc<(Mutex<bool>, std::sync::Condvar)>,
}

impl Signal {
    pub fn new() -> Signal {
        Signal { inner: Arc::new((Mutex::new(false), std::sync::Condvar::new())) }
    }

    /// `TrySetResult()`.
    pub fn set(&self) {
        let (m, c) = &*self.inner;
        *m.lock().unwrap() = true;
        c.notify_all();
    }

    pub fn handle(&self) -> Signal {
        Signal { inner: Arc::clone(&self.inner) }
    }

    /// `WaitAsync(timeout)`: süre dolarsa panik.
    pub fn wait(&self, seconds: u64) {
        let (m, c) = &*self.inner;
        let guard = m.lock().unwrap();
        let (guard, result) = c.wait_timeout_while(guard, std::time::Duration::from_secs(seconds), |set| !*set).unwrap();
        assert!(*guard && !result.timed_out(), "zaman aşımı: olay gelmedi");
    }
}
