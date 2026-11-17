//! Windows başlangıç (HKCU\...\Run ve StartupApproved\Run) kayıtlarının hedef yol, biçimlendirme ve göç kararları.
//! Saf mantık: Win32 ve kayıt defteri çağrıları Platform katmanındadır. Karşılığı: `Core/Shell/StartupPathPolicy.cs`.

use crate::win_path;

pub struct StartupPathPolicy;

impl StartupPathPolicy {
    pub const RUN_KEY_PATH: &'static str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    pub const STARTUP_APPROVED_RUN_KEY_PATH: &'static str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
    pub const APP_VALUE_NAME: &'static str = "Koy";

    /// Otomatik başlatmaya yazılacak çalıştırılabilir dosya yolu. Velopack kurulumunda `current\Koy.exe` yerine kök
    /// klasördeki kalıcı saplamayı (`Koy.exe`) seçer; saplama yoksa ya da taşınabilir/geliştirme derlemesindeyse
    /// `process_path` döner.
    pub fn resolve_target_path(
        is_installed: bool,
        is_portable: bool,
        root_app_dir: Option<&str>,
        process_path: Option<&str>,
        file_exists: impl Fn(&str) -> bool,
    ) -> Option<String> {
        let process_path = process_path.filter(|p| !p.trim().is_empty())?;
        if is_installed && !is_portable && let Some(root) = root_app_dir.filter(|r| !r.trim().is_empty()) {
            let exe_name = win_path::get_file_name(process_path);
            if !exe_name.is_empty() {
                let stub = win_path::combine(root, exe_name);
                if file_exists(&stub) {
                    return Some(stub);
                }
            }
        }
        Some(process_path.to_string())
    }

    /// Boşluk içeren yollar Run anahtarında bozulmasın diye yolu tırnaklar. Boş yol `None` (C# ArgumentException).
    pub fn format_command_line(exe_path: &str, arguments: Option<&str>) -> Option<String> {
        if exe_path.trim().is_empty() {
            return None;
        }
        let trimmed = exe_path.trim().trim_matches('"');
        if trimmed.trim().is_empty() {
            return None;
        }
        Some(match arguments.filter(|a| !a.trim().is_empty()) {
            None => format!("\"{trimmed}\""),
            Some(a) => format!("\"{trimmed}\" {}", a.trim()),
        })
    }

    /// Kayıt defterindeki komut satırından çalıştırılabilir dosya yolunu ayıklar: tırnaklı yollar, tırnaksız boşluklu
    /// `.exe` yolları ve ek argümanlı satırlar.
    pub fn extract_executable_path(command_line: Option<&str>) -> Option<String> {
        let trimmed = command_line?.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(rest) = trimmed.strip_prefix('"') {
            let close = rest.find('"')?;
            if close == 0 {
                return None;
            }
            let quoted = rest[..close].trim();
            return if quoted.is_empty() { None } else { Some(quoted.to_string()) };
        }
        if let Some(end) = find_exe_boundary(trimmed) {
            return Some(trimmed[..end].trim().to_string());
        }
        match trimmed.find([' ', '\t']) {
            Some(i) if i > 0 => Some(trimmed[..i].to_string()),
            _ => Some(trimmed.to_string()),
        }
    }

    /// Görev Yöneticisi (`StartupApproved\Run`) bu girdiyi devre dışı bırakmış mı? Kayıt yoksa ya da boşsa etkindir;
    /// ilk bayt tek ise (ör. 0x03, 0x07, 0x01) kullanıcı tarafından kapatılmıştır.
    pub fn is_startup_approved_enabled(approved_bytes: Option<&[u8]>) -> bool {
        match approved_bytes {
            None => true,
            Some(b) => b.is_empty() || b[0] & 0x01 == 0,
        }
    }

    /// Kayıtlı başlangıç girdisi gerçekten çalışır ve beklenen hedefe eşit mi? Silinmiş yol, eski geliştirme /
    /// taşınabilir yolu, `current\Koy.exe` ya da Görev Yöneticisi engeli false döner.
    pub fn is_entry_valid(
        registry_command_line: Option<&str>,
        approved_bytes: Option<&[u8]>,
        expected_exe_path: Option<&str>,
        file_exists: impl Fn(&str) -> bool,
    ) -> bool {
        if !Self::is_startup_approved_enabled(approved_bytes) {
            return false;
        }
        let Some(registered) = Self::extract_executable_path(registry_command_line).filter(|p| !p.trim().is_empty()) else {
            return false;
        };
        if !file_exists(&registered) {
            return false;
        }
        if let Some(expected) = expected_exe_path.filter(|e| !e.trim().is_empty())
            && !Self::paths_equal(Some(&registered), Some(expected))
        {
            return false;
        }
        true
    }

    /// Kurulu sürüm açılırken mevcut Run kaydı eski, geliştirme/taşınabilir ya da `current\Koy.exe` yoluna bakıyorsa
    /// kalıcı saplamaya geçirilmeli mi? Görev Yöneticisi'nde açıkça kapatılmışsa kullanıcının kararı korunur.
    pub fn should_migrate_installed_entry(
        registry_command_line: Option<&str>,
        approved_bytes: Option<&[u8]>,
        expected_installed_exe_path: Option<&str>,
        file_exists: impl Fn(&str) -> bool,
    ) -> bool {
        let Some(expected) = expected_installed_exe_path.filter(|e| !e.trim().is_empty()) else { return false };
        if !Self::is_startup_approved_enabled(approved_bytes) {
            return false;
        }
        let Some(registered) = Self::extract_executable_path(registry_command_line).filter(|p| !p.trim().is_empty()) else {
            return false;
        };
        if Self::paths_equal(Some(&registered), Some(expected)) {
            return false;
        }
        let registered_name = win_path::get_file_name(&registered);
        let expected_name = win_path::get_file_name(expected);
        crate::text::eq_ignore_case(registered_name, expected_name) || !file_exists(&registered)
    }

    /// Dosya yollarını büyük/küçük harf duyarsız ve normalleştirilmiş biçimde karşılaştırır.
    pub fn paths_equal(left: Option<&str>, right: Option<&str>) -> bool {
        let (Some(l), Some(r)) = (left.filter(|s| !s.trim().is_empty()), right.filter(|s| !s.trim().is_empty())) else {
            return false;
        };
        let norm = |p: &str| {
            win_path::get_full_path(p.trim().trim_matches('"')).map(|f| f.trim_end_matches(['\\', '/']).to_string())
        };
        match (norm(l), norm(r)) {
            (Some(a), Some(b)) => crate::text::eq_ignore_case(&a, &b),
            // C#: yol geçersizse (ArgumentException vb.) kırpılmış metinler karşılaştırılır.
            _ => crate::text::eq_ignore_case(l.trim(), r.trim()),
        }
    }
}

/// `.exe` ile biten ilk sınır (sonrası boşluk ya da metin sonu); yoksa `None`. C# `IndexOf(".exe", OrdinalIgnoreCase)`.
fn find_exe_boundary(value: &str) -> Option<usize> {
    // ASCII küçültme bayt konumlarını korur.
    let lower = value.to_ascii_lowercase();
    let mut idx = 0;
    while idx < value.len() {
        let m = lower[idx..].find(".exe")? + idx;
        let end = m + 4;
        if end == value.len() || value[end..].chars().next().is_some_and(char::is_whitespace) {
            return Some(end);
        }
        idx = end;
    }
    None
}
