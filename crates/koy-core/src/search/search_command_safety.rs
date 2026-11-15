/// Yazılan metnin komut ya da program olarak çalıştırılmadan önce onay isteyip istemediği.
/// Pano metni ve satır sonu / denetim karakteri içeren metin tek Enter ile çalışmaz.
pub struct SearchCommandSafety;

impl SearchCommandSafety {
    pub const NEWLINE_MARK: &'static str = "⏎";

    /// Sekme dışında denetim karakteri (satır sonu dahil) var mı.
    pub fn has_control_characters(text: &str) -> bool {
        text.chars().any(|c| c.is_control() && c != '\t')
    }

    /// Onayda gösterilecek metin: satır sonları ⏎, diğer denetim karakterleri boşluk olur.
    pub fn for_display(text: &str) -> String {
        let chars: Vec<char> = text.chars().collect();
        let mut result = String::with_capacity(text.len());
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '\r' || c == '\n' {
                if c == '\r' && i + 1 < chars.len() && chars[i + 1] == '\n' {
                    i += 1;
                }
                result.push('⏎');
            } else {
                result.push(if c.is_control() && c != '\t' { ' ' } else { c });
            }
            i += 1;
        }

        result
    }

    /// Kabuğa verilecek metin: satır sonu ve denetim karakterleri boşluğa çevrilir, kabuğa hiç satır sonu gitmez.
    pub fn to_single_line(text: &str) -> String {
        let result: String = text.chars().map(|c| if c.is_control() && c != '\t' { ' ' } else { c }).collect();
        result.trim().to_string()
    }
}
