//! Karşılığı: `JumpList/ShortcutAppIdMatcher.cs`.

use super::win_path;
use crate::text;

/// Süreç düzeyinde AppUserModelID koyan paketsiz uygulamaların (Discord gibi) AUMID'i Başlat menüsü kısayolunda durur.
/// Bu tip bir kısayolun belirli bir exe'ye ait olup olmadığına karar verir; kısayol okuma Platform'da yapılır.
///
/// - Tam eşleşme: kısayol hedefi exe yoluyla (büyük/küçük harf duyarsız) aynı.
/// - Squirrel düzeni: exe `<kök>\app-x.y.z\X.exe` (yolu sürümle değişir), kısayol ise kökteki bir başlatıcıyı
///   (`Update.exe` ya da saplama `X.exe`) `--processStart X.exe` ile çalıştırır.
pub struct ShortcutAppIdMatcher;

impl ShortcutAppIdMatcher {
    pub const SQUIRREL_FOLDER_PREFIX: &'static str = "app-";

    /// Kısayol hedefi exe'ye tam yolla işaret ediyorsa true.
    pub fn matches_exact(exe_path: &str, link_target: Option<&str>) -> bool {
        match (normalize(Some(exe_path)), normalize(link_target)) {
            (Some(exe), Some(target)) => text::eq_ignore_case(&exe, &target),
            _ => false,
        }
    }

    /// Kısayol hedefi `<kök>` klasöründeki bir exe ise (`Update.exe` ya da kökteki saplama `X.exe`) ve bağımsız
    /// değişkenlerinde `--processStart` ile exe dosya adı geçiyorsa, exe de `<kök>\app-*\<ad>.exe` ise true.
    pub fn matches_squirrel(exe_path: &str, link_target: Option<&str>, link_arguments: Option<&str>) -> bool {
        let exe = normalize(Some(exe_path));
        let target = normalize(link_target);
        let (Some(exe), Some(target)) = (exe, target) else { return false };
        let arguments = match link_arguments {
            Some(a) if !text::is_null_or_white_space(Some(a)) => a,
            _ => return false,
        };

        let Some(app_folder) = win_path::get_directory_name(&exe) else { return false };
        if !text::starts_with_ignore_case(win_path::get_file_name(&app_folder), Self::SQUIRREL_FOLDER_PREFIX) {
            return false;
        }

        let root = win_path::get_directory_name(&app_folder);
        let updater_root = win_path::get_directory_name(&target);
        let (Some(root), Some(updater_root)) = (root, updater_root) else { return false };
        if !text::eq_ignore_case(&root, &updater_root) {
            return false;
        }

        Self::has_process_start(arguments, win_path::get_file_name(&exe))
    }

    /// Bağımsız değişkenlerde `--processStart` (ya da `--processStartAndWait`) var ve exe adı bir sözcük olarak geçiyor.
    pub fn has_process_start(arguments: &str, exe_name: &str) -> bool {
        let mut has_flag = false;
        let mut has_name = false;
        for token in arguments.split([' ', '\t', '"', '=']).filter(|t| !t.is_empty()) {
            if text::starts_with_ignore_case(token, "--processStart") {
                has_flag = true;
            } else if text::eq_ignore_case(token, exe_name) {
                has_name = true;
            }
        }

        has_flag && has_name
    }
}

fn normalize(path: Option<&str>) -> Option<String> {
    if text::is_null_or_white_space(path) {
        return None;
    }

    // Path.GetFullPath(path.Trim().Trim('"')); fırlatırsa null.
    win_path::get_full_path(text::trim(path?).trim_matches('"'))
}
