//! Karşılığı: `JumpList/JumpListEntryFactory.cs`.

use super::jump_list_limits::JumpListLimits;
use super::jump_list_models::{JumpListEntry, JumpListLaunch};
use crate::text;

/// UTF-16 kod birimi sayısı: C# `string.Length` karşılığı (sınırlar karakter değil kod birimi sayar).
fn utf16_len(s: &str) -> i32 {
    s.encode_utf16().count() as i32
}

/// Platformun okuduğu ham alanlardan menü girdisi üretir. Kural: başlık uydurulmaz. Başlığı ve hedefi boş olan
/// (ya da kendini ayırıcı işaretleyen) öğe ayırıcıdır; başlığı boş ama hedefi olan öğe (ör. Spotify) atlanır.
pub struct JumpListEntryFactory;

impl JumpListEntryFactory {
    /// Görev / kategori kısayolu için girdi. Atlanacaksa `None` döner.
    pub fn from_link(
        title: Option<&str>,
        target: Option<&str>,
        arguments: Option<&str>,
        working_directory: Option<&str>,
        marked_separator: bool,
    ) -> Option<JumpListEntry> {
        let clean_title = JumpListText::sanitize(title, JumpListLimits::MAX_TITLE_CHARS);
        let clean_target = target.map(text::trim);
        let target_is_empty = clean_target.is_none_or(str::is_empty);

        if marked_separator || (clean_title.is_empty() && target_is_empty) {
            return Some(JumpListEntry::separator());
        }

        // Başlık yoksa uydurmak yerine atla; hedef yoksa çalıştırılacak bir şey yoktur.
        if clean_title.is_empty() || target_is_empty {
            return None;
        }
        let clean_target = clean_target?;

        if utf16_len(clean_target) > JumpListLimits::MAX_TARGET_CHARS
            || arguments.map_or(0, utf16_len) > JumpListLimits::MAX_ARGUMENTS_CHARS
        {
            return None;
        }

        let args = if text::is_null_or_white_space(arguments) { None } else { arguments.map(str::to_string) };
        let dir = if text::is_null_or_white_space(working_directory) {
            None
        } else {
            working_directory.map(|d| text::trim(d).to_string())
        };
        Some(JumpListEntry::new(
            clean_title,
            Some(JumpListLaunch {
                target: clean_target.to_string(),
                arguments: args,
                working_directory: dir,
                is_document: false,
            }),
        ))
    }

    /// Son / sık kullanılan belge girdisi (kabuk öğesi). Başlık ya da hedef yoksa `None`.
    pub fn from_document(title: Option<&str>, target: Option<&str>) -> Option<JumpListEntry> {
        let clean_title = JumpListText::sanitize(title, JumpListLimits::MAX_TITLE_CHARS);
        let clean_target = target.map(text::trim);
        let clean_target = match clean_target {
            Some(t) if !t.is_empty() => t,
            _ => return None,
        };
        if clean_title.is_empty() || utf16_len(clean_target) > JumpListLimits::MAX_TARGET_CHARS {
            return None;
        }

        Some(JumpListEntry::new(clean_title, Some(JumpListLaunch { target: clean_target.to_string(), is_document: true, ..Default::default() })))
    }
}

/// Güvenilmeyen metni menüye uygun hâle getirir.
pub struct JumpListText;

impl JumpListText {
    /// Denetim karakterlerini (satır sonu, sekme, NUL…) boşluğa çevirir, kenar boşluklarını ve art arda boşlukları
    /// atar, `max_chars` üstünü "…" ile keser. Boş ya da `None` girdi boş dize verir.
    ///
    /// Uzunluklar C#'taki gibi UTF-16 kod birimi sayar; bu yüzden tampon `u16` tutulur.
    pub fn sanitize(text: Option<&str>, max_chars: i32) -> String {
        let text = match text {
            Some(t) if !t.is_empty() => t,
            _ => return String::new(),
        };

        const SPACE: u16 = b' ' as u16;
        let mut buffer: Vec<u16> = Vec::with_capacity(utf16_len(text).min(max_chars + 1).max(0) as usize);
        let mut last_was_space = true; // baştaki boşlukları atar
        for ch in text.encode_utf16() {
            // char.IsControl / char.IsWhiteSpace: tek kod birimi üzerinde; vekil yarıları ikisi de değildir.
            let is_blank = char::from_u32(ch as u32).is_some_and(|c| c.is_control() || c.is_whitespace());
            let c = if is_blank { SPACE } else { ch };
            if c == SPACE {
                if last_was_space {
                    continue;
                }
                last_was_space = true;
            } else {
                last_was_space = false;
            }

            buffer.push(c);
            if buffer.len() as i32 > max_chars + 1 {
                break;
            }
        }

        while buffer.last() == Some(&SPACE) {
            buffer.pop();
        }

        if buffer.len() as i32 <= max_chars {
            return String::from_utf16_lossy(&buffer);
        }

        // Vekil çiftini ortadan bölme.
        let mut cut = max_chars - 1;
        if cut > 0 && (0xD800..0xDC00).contains(&buffer[(cut - 1) as usize]) {
            cut -= 1;
        }
        let head = String::from_utf16_lossy(&buffer[..cut.max(0) as usize]);
        format!("{}…", head.trim_end())
    }

    /// `@dll,-id` ya da `@{ms-resource…}` gibi dolaylı (kaynak) dize mi.
    pub fn is_indirect(text: Option<&str>) -> bool {
        text.is_some_and(|t| utf16_len(t) > 1 && t.starts_with('@'))
    }
}
