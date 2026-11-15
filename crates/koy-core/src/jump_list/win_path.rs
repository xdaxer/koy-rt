//! .NET `System.IO.Path` işlevlerinin Windows davranışı (dize tabanlı, dosya sistemine dokunmaz).
//!
//! C# Core `Path.GetFileName / GetExtension / GetDirectoryName / GetFullPath` çağırır ve bu çağrıların sonuçları
//! Windows yol kurallarına bağlıdır (`\` ve `/` ayırıcıdır, kök uzunluğu, ayırıcı normalleştirme). Rust'ın
//! `std::path::Path`'i platforma göre davrandığı ve .NET ile ayrıntıda ayrıştığı için burada elle yazıldı.
//! Aygıt sözdizimi (`\\?\`, `\\.\`) desteklenmez; sade sürücü, UNC ve köke göreli yollar desteklenir.

fn is_sep(c: char) -> bool {
    c == '\\' || c == '/'
}

fn is_valid_drive_char(c: char) -> bool {
    c.is_ascii_alphabetic()
}

/// `Path.GetFileName`: son ayırıcıdan (ya da `:`'dan) sonrası.
pub fn get_file_name(path: &str) -> &str {
    match path.rfind(|c: char| is_sep(c) || c == ':') {
        Some(i) => &path[i + 1..],
        None => path,
    }
}

/// `Path.GetExtension`: son noktadan sonrası (nokta dahil); nokta son karakterse ya da yoksa boş.
pub fn get_extension(path: &str) -> &str {
    let bytes = path.as_bytes();
    let mut i = bytes.len();
    while i > 0 {
        i -= 1;
        let c = bytes[i];
        if c == b'.' {
            return if i != bytes.len() - 1 { &path[i..] } else { "" };
        }
        if c == b'\\' || c == b'/' || c == b':' {
            break;
        }
    }
    ""
}

/// `PathInternal.GetRootLength` (Windows, aygıt sözdizimi hariç), karakter sayısı olarak.
fn get_root_length(chars: &[char]) -> usize {
    let len = chars.len();
    let mut i = 0;
    if len > 0 && is_sep(chars[0]) {
        if len > 1 && is_sep(chars[1]) {
            // UNC: sunucu\paylaşım'ı geç.
            i = 2;
            let mut n = 2;
            while i < len {
                if is_sep(chars[i]) {
                    n -= 1;
                    if n <= 0 {
                        break;
                    }
                }
                i += 1;
            }
        } else {
            i = 1; // yalın köke göreli
        }
    } else if len >= 2 && chars[1] == ':' && is_valid_drive_char(chars[0]) {
        i = 2;
        if len > 2 && is_sep(chars[2]) {
            i += 1;
        }
    }
    i
}

/// `PathInternal.NormalizeDirectorySeparators`: `/` → `\`, art arda ayırıcılar teke iner (baştaki UNC çifti korunur).
fn normalize_directory_separators(path: &str) -> String {
    let chars: Vec<char> = path.chars().collect();
    let mut out = String::with_capacity(path.len());
    let mut start = 0;
    if !chars.is_empty() && is_sep(chars[0]) {
        start = 1;
        out.push('\\');
    }
    let mut i = start;
    while i < chars.len() {
        let mut current = chars[i];
        if is_sep(current) {
            if i + 1 < chars.len() && is_sep(chars[i + 1]) {
                i += 1;
                continue;
            }
            current = '\\';
        }
        out.push(current);
        i += 1;
    }
    out
}

/// `Path.GetDirectoryName`: üst klasör; kök ya da boş yolda `None`.
pub fn get_directory_name(path: &str) -> Option<String> {
    let chars: Vec<char> = path.chars().collect();
    let root_length = get_root_length(&chars);
    if chars.len() <= root_length {
        return None;
    }

    let mut end = chars.len();
    while end > root_length {
        end -= 1;
        if is_sep(chars[end]) {
            break;
        }
    }
    // Kalan ayırıcıları kırp (C:\foo\\bar).
    while end > root_length && is_sep(chars[end - 1]) {
        end -= 1;
    }

    let head: String = chars[..end].iter().collect();
    Some(normalize_directory_separators(&head))
}

/// `Path.GetFullPath`: .NET'in fırlattığı durumlarda (boş, NUL içeren) `None`. Ayırıcıları normalleştirir, `.` ve `..`
/// parçalarını çözer, göreli yolları geçerli klasöre bağlar.
pub fn get_full_path(path: &str) -> Option<String> {
    if path.is_empty() || path.contains('\0') {
        return None;
    }

    let mut full: Vec<char> = path.chars().map(|c| if c == '/' { '\\' } else { c }).collect();
    let rooted_len = get_root_length(&full);
    let is_drive_absolute = rooted_len >= 3 && full[1] == ':';
    let is_unc = rooted_len >= 2 && full[0] == '\\' && full[1] == '\\';
    if !is_drive_absolute && !is_unc {
        let cwd = std::env::current_dir().ok()?.to_string_lossy().replace('/', "\\");
        let cwd: Vec<char> = cwd.chars().collect();
        let cwd_root = get_root_length(&cwd);
        let mut joined: Vec<char>;
        if full.first() == Some(&'\\') {
            // \dosya: geçerli sürücünün köküne göre.
            joined = cwd[..cwd_root.min(cwd.len())].to_vec();
            while joined.last() == Some(&'\\') {
                joined.pop();
            }
            joined.extend(full.iter());
        } else if full.len() >= 2 && full[1] == ':' && is_valid_drive_char(full[0]) {
            // C:dosya: aynı sürücüdeyse geçerli klasöre, değilse sürücü köküne göre.
            let rest: Vec<char> = full[2..].to_vec();
            if cwd.len() >= 2 && cwd[0].eq_ignore_ascii_case(&full[0]) {
                joined = cwd.clone();
            } else {
                joined = vec![full[0], ':'];
            }
            joined.push('\\');
            joined.extend(rest);
        } else {
            joined = cwd.clone();
            joined.push('\\');
            joined.extend(full.iter());
        }
        full = joined;
    }

    let root_length = get_root_length(&full);
    let root: String = full[..root_length].iter().collect();
    let rest: String = full[root_length..].iter().collect();
    let ends_with_sep = rest.ends_with('\\');

    let mut parts: Vec<&str> = Vec::new();
    for part in rest.split('\\') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }

    let mut result = root;
    if !result.ends_with('\\') && !parts.is_empty() {
        result.push('\\');
    }
    result.push_str(&parts.join("\\"));
    if ends_with_sep && !parts.is_empty() {
        result.push('\\');
    }
    Some(result)
}
