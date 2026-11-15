use super::i_pinned_apps_service::PinnedApp;
use super::known_app_user_model_ids::KnownAppUserModelIds;
use super::taskband_packaged_pins::TaskbandPackagedPins;
use crate::shell::OpenWindowInfo;
use crate::text;
use crate::win_path;

/// Açık bir pencerenin hangi sabitli uygulamaya ait olduğu. Windows görev çubuğu düğmeleri AppUserModelID (AUMID) ile
/// gruplar; Koy da önce onu kullanır. AUMID yoksa tam yol, en sonda dosya adı ve başlık gelir.
/// File Explorer kısayolunun hedef yolu boştur (kabuk öğesi) ve Gezgin pencereleri pencere başına AUMID taşımaz; bu yüzden
/// pencerede AUMID yoksa bilinen sistem uygulamaları tablosuna (explorer.exe → "Microsoft.Windows.Explorer") bakılır.
/// Tablo olmasaydı eşleme başlığa düşüp Gezgin penceresi için ikinci bir simge çıkarırdı.
pub struct PinnedAppMatcher;

fn is_empty_opt(s: &Option<String>) -> bool {
    s.as_deref().is_none_or(str::is_empty)
}

impl PinnedAppMatcher {
    pub fn is_match(pin: &PinnedApp, win: &OpenWindowInfo) -> bool {
        // 1. Kimlik: ikisinde de AUMID varsa karar onundur. Aynı exe'nin farklı AUMID'li pencereleri (Chrome profilleri,
        //    PWA'lar) Windows'ta ayrı düğmedir, burada da ayrı kalır.
        let win_aumid: Option<String> = if is_empty_opt(&win.app_user_model_id) {
            KnownAppUserModelIds::for_window(win.process_path.as_deref(), win.class_name.as_deref())
        } else {
            win.app_user_model_id.clone()
        };
        if !is_empty_opt(&pin.app_user_model_id) && win_aumid.as_deref().is_some_and(|s| !s.is_empty()) {
            return text::eq_ignore_case(pin.app_user_model_id.as_deref().unwrap_or(""), win_aumid.as_deref().unwrap_or(""));
        }

        // 2. Hedef yürütülebilir dosya: normalleştirilmiş tam yol, sonra dosya adı.
        if let (Some(target), Some(proc_path)) = (pin.target_path.as_deref(), win.process_path.as_deref())
            && !target.is_empty()
            && !proc_path.is_empty()
        {
            if Self::same_path(target, proc_path) {
                return true;
            }

            let pin_exe = win_path::get_file_name(target);
            let win_exe = win_path::get_file_name(proc_path);
            if !pin_exe.is_empty() && text::eq_ignore_case(pin_exe, win_exe) {
                return true;
            }
        }

        // 3. Başlık / kısayol adı (son çare; shell:AppsFolder\<AUMID> yolundaki noktalar paket adını kesmesin diye atlanır)
        if !TaskbandPackagedPins::is_apps_folder_path(Some(&pin.shortcut_path)) {
            let shortcut_name = win_path::get_file_name_without_extension(&pin.shortcut_path);
            if !shortcut_name.is_empty() && !win.title.is_empty() && text::contains_ignore_case(&win.title, shortcut_name) {
                return true;
            }
        }

        !pin.title.is_empty() && !win.title.is_empty() && text::contains_ignore_case(&win.title, &pin.title)
    }

    /// Yolları karşılaştırır: ortam değişkenleri açılır, büyük/küçük harf ve System32/SysWOW64/Sysnative farkı yok sayılır.
    pub fn same_path(a: &str, b: &str) -> bool {
        text::eq_ignore_case(&Self::normalize(a), &Self::normalize(b))
    }

    pub fn normalize(path: &str) -> String {
        let trimmed = text::trim(path).trim_matches('"');
        let mut p = Self::expand_environment_variables(trimmed);
        // Geçersiz yol olduğu gibi karşılaştırılır (GetFullPath atarsa `None`).
        if let Some(full) = win_path::get_full_path(&p) {
            p = full;
        }

        let p = Self::replace_ignore_case(&p, "\\SysWOW64\\", "\\System32\\");
        Self::replace_ignore_case(&p, "\\Sysnative\\", "\\System32\\")
    }

    /// `Environment.ExpandEnvironmentVariables`: `%AD%` tanımlıysa değeriyle değişir, tanımsız olan olduğu gibi kalır.
    fn expand_environment_variables(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut pos = 0;
        while pos < text.len() {
            let Some(rel) = text[pos..].find('%') else {
                out.push_str(&text[pos..]);
                break;
            };
            let first = pos + rel;
            out.push_str(&text[pos..first]);
            let Some(rel2) = text[first + 1..].find('%') else {
                out.push_str(&text[first..]);
                break;
            };
            let second = first + 1 + rel2;
            let name = &text[first + 1..second];
            match if name.is_empty() { None } else { std::env::var(name).ok() } {
                Some(value) => {
                    out.push_str(&value);
                    pos = second + 1;
                }
                None => {
                    out.push_str(&text[first..second]);
                    pos = second;
                }
            }
        }
        out
    }

    // string.Replace(from, to, StringComparison.OrdinalIgnoreCase); desenler ASCII olduğundan ASCII küçük harfe çevirme
    // bayt uzunluklarını korur.
    fn replace_ignore_case(s: &str, from: &str, to: &str) -> String {
        let lower = s.to_ascii_lowercase();
        let from_lower = from.to_ascii_lowercase();
        let mut out = String::with_capacity(s.len());
        let mut pos = 0;
        while let Some(rel) = lower[pos..].find(&from_lower) {
            let at = pos + rel;
            out.push_str(&s[pos..at]);
            out.push_str(to);
            pos = at + from.len();
        }
        out.push_str(&s[pos..]);
        out
    }
}
