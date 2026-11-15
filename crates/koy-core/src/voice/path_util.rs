//! `System.IO.Path` işlevlerinin Windows davranışı (`\` ve `/` ayırıcı, `:` birim ayırıcı).
//!
//! Voice modülü Windows PATH çözümlemesi yaptığı için ana makinenin yol kurallarına değil, .NET'in Windows
//! davranışına bağlıdır; bu yüzden `std::path` yerine metin üzerinde çalışılır.

const DIRECTORY_SEPARATOR: char = '\\';

fn is_directory_separator(c: char) -> bool {
    c == '\\' || c == '/'
}

/// `Path.IsPathRooted`.
pub(crate) fn is_path_rooted(path: &str) -> bool {
    let mut it = path.chars();
    match (it.next(), it.next()) {
        (Some(c), _) if is_directory_separator(c) => true,
        (Some(c), Some(':')) => c.is_ascii_alphabetic(),
        _ => false,
    }
}

/// `Path.Combine(first, second)` (iki parça).
pub(crate) fn combine(first: &str, second: &str) -> String {
    if first.is_empty() {
        return second.to_string();
    }
    if second.is_empty() {
        return first.to_string();
    }
    if is_path_rooted(second) {
        return second.to_string();
    }
    let has_separator =
        first.chars().next_back().is_some_and(is_directory_separator) || second.chars().next().is_some_and(is_directory_separator);
    if has_separator {
        format!("{first}{second}")
    } else {
        format!("{first}{DIRECTORY_SEPARATOR}{second}")
    }
}

/// `Path.GetExtension`: noktayla başlar; yoksa ya da nokta son karakterse boş.
pub(crate) fn get_extension(path: &str) -> &str {
    let chars: Vec<(usize, char)> = path.char_indices().collect();
    for (n, &(byte_index, c)) in chars.iter().enumerate().rev() {
        if c == '.' {
            return if n != chars.len() - 1 { &path[byte_index..] } else { "" };
        }
        if is_directory_separator(c) || c == ':' {
            break;
        }
    }
    ""
}

/// Yolun kök uzunluğu (karakter): `C:`, `C:\`, `\`. UNC için `\\sunucu\paylaşım` de kök sayılır.
fn root_length(path: &str) -> usize {
    let chars: Vec<char> = path.chars().collect();
    if chars.len() >= 2 && chars[0].is_ascii_alphabetic() && chars[1] == ':' {
        return if chars.len() >= 3 && is_directory_separator(chars[2]) { 3 } else { 2 };
    }
    if chars.len() >= 2 && is_directory_separator(chars[0]) && is_directory_separator(chars[1]) {
        // \\sunucu\paylaşım[\]
        let mut i = 2;
        let mut parts = 0;
        while i < chars.len() {
            if is_directory_separator(chars[i]) {
                parts += 1;
                if parts == 2 {
                    return i + 1;
                }
            }
            i += 1;
        }
        return chars.len();
    }
    if !chars.is_empty() && is_directory_separator(chars[0]) {
        return 1;
    }
    0
}

/// `Path.GetFileName`: son ayırıcıdan sonrası; kök kısmı asla kesilmez.
pub(crate) fn get_file_name(path: &str) -> &str {
    let root = root_length(path);
    let chars: Vec<(usize, char)> = path.char_indices().collect();
    let last_sep = chars.iter().rposition(|&(_, c)| is_directory_separator(c));
    let start_char = match last_sep {
        Some(i) if i >= root => i + 1,
        _ => root,
    };
    match chars.get(start_char) {
        Some(&(byte_index, _)) => &path[byte_index..],
        None => "",
    }
}
