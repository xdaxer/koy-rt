use super::path_util;
use crate::text;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HarnessExecutableKind {
    /// Doğrudan başlatılabilir (.exe / .com) dosya bulundu.
    Found,

    /// Yalnız .cmd / .bat betiği bulundu. Windows bunu kabuk olmadan başlatamaz.
    ScriptNotSupported,

    /// Hiçbir aday bulunamadı.
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessExecutableResolution {
    pub kind: HarnessExecutableKind,
    pub path: String,
}

const NATIVE_EXTENSIONS: [&str; 2] = [".exe", ".com"];
const SCRIPT_EXTENSIONS: [&str; 2] = [".cmd", ".bat"];

/// Harness komutunu Windows PATH'inde çözer. npm kurulumları `codex.cmd` gibi betik shim'leri bırakır;
/// `Process.Start` (UseShellExecute=false) bunları çalıştıramaz ve hata "dosya bulunamadı" olarak döner.
/// Bu sınıf, bir `.exe` adayı varsa onu seçer; yalnız betik varsa bunu açıkça bildirir.
/// Dosya sistemi erişimi `file_exists` ile dışarıdan verilir (test edilebilir).
pub struct HarnessExecutableResolver;

impl HarnessExecutableResolver {
    pub fn resolve<I>(name: &str, path_directories: I, file_exists: impl Fn(&str) -> bool) -> HarnessExecutableResolution
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        // ArgumentException.ThrowIfNullOrWhiteSpace(name)
        assert!(!text::is_null_or_white_space(Some(name)), "name");

        // Yol bilgisi içeren ad (ör. C:\tools\x) doğrudan denenir; PATH'e bakılmaz.
        if path_util::is_path_rooted(name) || name.contains('\\') || name.contains('/') {
            return Self::resolve_file(name, &file_exists);
        }

        for dir in path_directories {
            let dir = dir.as_ref();
            if text::is_null_or_white_space(Some(dir)) {
                continue;
            }

            let result = Self::resolve_file(&path_util::combine(dir, name), &file_exists);
            if result.kind != HarnessExecutableKind::NotFound {
                return result;
            }
        }

        HarnessExecutableResolution { kind: HarnessExecutableKind::NotFound, path: name.to_string() }
    }

    fn resolve_file(path: &str, file_exists: &impl Fn(&str) -> bool) -> HarnessExecutableResolution {
        let ext = path_util::get_extension(path);

        if !ext.is_empty() {
            if NATIVE_EXTENSIONS.iter().any(|e| text::eq_ignore_case(e, ext)) {
                return if file_exists(path) { Self::found(path) } else { Self::not_found(path) };
            }

            if SCRIPT_EXTENSIONS.iter().any(|e| text::eq_ignore_case(e, ext)) {
                return if file_exists(path) {
                    HarnessExecutableResolution { kind: HarnessExecutableKind::ScriptNotSupported, path: path.to_string() }
                } else {
                    Self::not_found(path)
                };
            }

            // Bilinmeyen uzantı: dosya varsa olduğu gibi denenir.
            return if file_exists(path) { Self::found(path) } else { Self::not_found(path) };
        }

        // Uzantısız ad: önce yerel uzantılar, sonra betikler (Windows'taki sıraya yakın).
        for native in NATIVE_EXTENSIONS {
            let candidate = format!("{path}{native}");
            if file_exists(&candidate) {
                return Self::found(&candidate);
            }
        }

        for script in SCRIPT_EXTENSIONS {
            let candidate = format!("{path}{script}");
            if file_exists(&candidate) {
                return HarnessExecutableResolution { kind: HarnessExecutableKind::ScriptNotSupported, path: candidate };
            }
        }

        Self::not_found(path)
    }

    fn found(path: &str) -> HarnessExecutableResolution {
        HarnessExecutableResolution { kind: HarnessExecutableKind::Found, path: path.to_string() }
    }

    fn not_found(path: &str) -> HarnessExecutableResolution {
        HarnessExecutableResolution { kind: HarnessExecutableKind::NotFound, path: path.to_string() }
    }
}
