use std::collections::HashMap;

use super::utf16_len;
use crate::text;

/// `IDictionary<string, string>` / `IReadOnlyDictionary<string, string>` karşılığı. Büyük/küçük harf duyarsızlığı
/// C#'ta sözlüğün karşılaştırıcısından gelir (`ValueDictionary`), `AppIconOverrides`'tan değil: bu yüzden anahtar
/// eşlemesi uygulayana bırakılır. Ayarlar modülü kendi sözlüğü için bu trait'i uygular.
pub trait IconOverrideMap {
    /// Anahtara ait değer (anahtar karşılaştırması uygulayanın kuralıyla).
    fn get_value(&self, key: &str) -> Option<&str>;
    /// `dict[key] = value`.
    fn set_value(&mut self, key: &str, value: String);
    /// `dict.Remove(key)`.
    fn remove_value(&mut self, key: &str);
}

/// Sıradan (büyük/küçük harf duyarlı) `HashMap` için varsayılan uygulama.
impl IconOverrideMap for HashMap<String, String> {
    fn get_value(&self, key: &str) -> Option<&str> {
        self.get(key).map(String::as_str)
    }

    fn set_value(&mut self, key: &str, value: String) {
        self.insert(key.to_string(), value);
    }

    fn remove_value(&mut self, key: &str) {
        self.remove(key);
    }
}

/// Uygulama başına simge değişimi: anahtar kısayol adı (büyük/küçük harf duyarsız), değer simge dosyasının yolu ya da adı.
/// Değer yalnız okunur ve doğrulanır; hiçbir zaman çalıştırılmaz. Geçersiz değer haritada kalsa bile çözümlemede yok sayılır.
pub struct AppIconOverrides;

const ALLOWED_EXTENSIONS: [&str; 3] = [".png", ".ico", ".svg"];

// Windows'ta yol ve dosya adında yasak karakterler. Ayırıcılar ayrıca denetlenir.
const FORBIDDEN_CHARS: [char; 6] = ['<', '>', '"', '|', '?', '*'];

// Win32'nin ayrılmış aygıt adları. Uzantı yok sayılır: "NUL.png" da NUL aygıtıdır, "CON.png" okumak sonsuza dek bekler.
// COM¹/COM²/COM³ ve LPT¹/LPT²/LPT³ (üst simge rakam) Windows'ta aynı şekilde ayrılmıştır.
const RESERVED_NAMES: [&str; 30] = [
    "CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "COM\u{00B9}", "COM\u{00B2}", "COM\u{00B3}", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    "LPT\u{00B9}", "LPT\u{00B2}", "LPT\u{00B3}",
];

// Ağ ve aygıt yolu önekleri. "\\" ve "//" zaten "\\?\" ile "\\.\" önekini de kapsar; yine de açık listelenir.
const UNC_PREFIXES: [&str; 4] = ["\\\\", "//", "\\\\?\\", "\\\\.\\"];

impl AppIconOverrides {
    pub const MAX_LENGTH: i32 = 260;

    /// Anahtar: boş değil, uzunluk sınırında, denetim ve yol karakteri içermez.
    pub fn is_valid_key(app_key: Option<&str>) -> bool {
        if text::is_null_or_white_space(app_key) {
            return false;
        }
        let key = app_key.unwrap_or_default().trim();
        if utf16_len(key) > Self::MAX_LENGTH as usize {
            return false;
        }
        !key.chars().any(|c| c.is_control() || matches!(c, '\\' | '/' | ':') || FORBIDDEN_CHARS.contains(&c))
    }

    /// Değer: mutlak yol ya da yalnız dosya adı; uzantı png, ico ya da svg olmalı. Yollarda nokta bileşeni, boş ya da
    /// nokta/boşlukla biten bileşen ve aygıt adı (CON, NUL, COM1…) yasak. Kırpılmış ve normalleştirilmiş değeri verir.
    pub fn try_validate_source(value: Option<&str>) -> Option<String> {
        if text::is_null_or_white_space(value) {
            return None;
        }

        let candidate = value.unwrap_or_default().trim();
        if utf16_len(candidate) > Self::MAX_LENGTH as usize {
            return None;
        }
        if candidate.chars().any(|c| c.is_control() || FORBIDDEN_CHARS.contains(&c)) {
            return None;
        }

        let extension = path_get_extension(candidate);
        if !ALLOWED_EXTENSIONS.iter().any(|e| text::eq_ignore_case(e, extension)) {
            return None;
        }

        if path_is_rooted(candidate) {
            // Mutlak yol yalnız <harf>:\ biçiminde. UNC (ağ, SMB/NTLM tetikler), ADS (iki nokta: gizli akış)
            // ve nokta bileşenleri (".", "..") kaçış yolu olduğu için reddedilir.
            if is_unc_path(candidate) {
                return None;
            }
            if !is_drive_rooted_path(candidate) {
                return None;
            }
            if candidate.chars().skip(2).any(|c| c == ':') {
                return None;
            }
            // `candidate[3..]`: ilk üç karakter (harf, ':' ve '\') ASCII olduğundan bayt konumu 3 doğrudur.
            if !candidate[3..].split(['\\', '/']).all(is_safe_component) {
                return None;
            }
        } else if candidate.contains('\\') || candidate.contains('/') || candidate.contains(':') {
            // Göreli yol kabul edilmez: yalnız dosya adı (klasör çözümü Core dışında).
            return None;
        } else if !is_safe_component(candidate) {
            return None;
        }

        Some(candidate.to_string())
    }

    /// Simge paketi girdisi: yalnız dosya adı (klasör, sürücü ya da UNC yok). Uzantı ve yasak karakter kuralları
    /// [`AppIconOverrides::try_validate_source`] ile aynıdır.
    pub fn try_validate_pack_file(value: Option<&str>) -> Option<String> {
        let candidate = Self::try_validate_source(value)?;
        if candidate.contains(['\\', '/', ':']) {
            return None;
        }

        Some(candidate)
    }

    /// Geçerli bir simge kaydeder. Geçersiz anahtar ya da değerde harita değişmez ve false döner.
    pub fn try_set<M: IconOverrideMap + ?Sized>(overrides: &mut M, app_key: &str, value: &str) -> bool {
        if !Self::is_valid_key(Some(app_key)) {
            return false;
        }
        let Some(normalized) = Self::try_validate_source(Some(value)) else {
            return false;
        };
        overrides.set_value(app_key.trim(), normalized);
        true
    }

    /// Uygulamanın simge değişimini siler ("Simgeyi sıfırla").
    pub fn reset<M: IconOverrideMap + ?Sized>(overrides: &mut M, app_key: &str) {
        if Self::is_valid_key(Some(app_key)) {
            overrides.remove_value(app_key.trim());
        }
    }

    /// Anahtara ait geçerli simge; yoksa ya da değer geçersizse `None`.
    pub fn lookup<M: IconOverrideMap + ?Sized>(overrides: Option<&M>, app_key: Option<&str>) -> Option<String> {
        let overrides = overrides?;
        if !Self::is_valid_key(app_key) {
            return None;
        }
        let value = overrides.get_value(app_key.unwrap_or_default().trim())?;
        Self::try_validate_source(Some(value))
    }
}

/// Yol bileşeni: boş değil, sonu nokta ya da boşluk değil (Win32 bunları sessizce kırpar, "..." ve "x." gibi biçimler
/// bu yüzden başka bir şeye çözülür) ve ayrılmış aygıt adı değil.
fn is_safe_component(part: &str) -> bool {
    if part.is_empty() || part.ends_with(['.', ' ']) {
        return false;
    }

    let stem = match part.find('.') {
        Some(dot) => &part[..dot],
        None => part,
    }
    .trim_end_matches(' ');
    !RESERVED_NAMES.iter().any(|r| text::eq_ignore_case(r, stem))
}

fn is_unc_path(candidate: &str) -> bool {
    UNC_PREFIXES.iter().any(|prefix| text::starts_with_ignore_case(candidate, prefix))
}

fn is_drive_rooted_path(candidate: &str) -> bool {
    let mut it = candidate.chars();
    matches!((it.next(), it.next(), it.next()), (Some(d), Some(':'), Some('\\')) if d.is_ascii_alphabetic())
}

/// `Path.IsPathRooted` (Windows): ayırıcıyla başlar ya da "<harf>:" ile başlar.
fn path_is_rooted(path: &str) -> bool {
    let mut it = path.chars();
    match (it.next(), it.next()) {
        (Some('\\' | '/'), _) => true,
        (Some(d), Some(':')) => d.is_ascii_alphabetic(),
        _ => false,
    }
}

/// `Path.GetExtension` (Windows): son ayırıcıdan sonraki son noktadan başlar; nokta sonda ise boş.
pub(crate) fn path_get_extension(path: &str) -> &str {
    for (i, c) in path.char_indices().rev() {
        if c == '.' {
            return if i + 1 != path.len() { &path[i..] } else { "" };
        }
        if c == '\\' || c == '/' {
            break;
        }
    }
    ""
}

/// `Path.GetFileNameWithoutExtension`. Köklü yol ("C:x") ayrıntısı yok: buraya yalnız ayırıcı/iki nokta içermeyen,
/// doğrulanmış anahtarlar ve dosya adları gelir.
pub(crate) fn path_get_file_name_without_extension(path: &str) -> &str {
    let name = match path.rfind(['\\', '/']) {
        Some(i) => &path[i + 1..],
        None => path,
    };
    match name.rfind('.') {
        Some(dot) => &name[..dot],
        None => name,
    }
}
