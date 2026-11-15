//! `System.Text.Json.JsonDocument` davranışlı küçük JSON belge modeli.
//!
//! İndeks, paket manifesti ve kayıt dosyası şu davranışlara bağlıdır; `serde_json::Value` bunları taşımaz:
//! - yinelenen anahtarlar korunur (yinelenen dil kodu "bozuk öğe" sayılır; `TryGetProperty` sonuncuyu verir),
//! - ham sayı metni korunur (`TryGetInt32` / `TryGetInt64` tam sayı yazımı arar: "1.0" olmaz),
//! - `MaxDepth` sınırı (sınırı aşan iç içe yapı hata),
//! - yorum, sondaki virgül ve geçersiz UTF-8 yok; denetim karakteri içeren dize hata.
//! Eşsiz yedek (`\uD800`) C#'ta ayrıştırmada geçer ama `GetString`'de yakalanmamış istisna fırlatırdı; burada
//! ayrıştırma hatasıdır (çağıranlar bunu "bozuk JSON" sayar).

/// Ayrıştırılmış JSON değeri (`JsonElement`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Json {
    Object(Vec<(String, Json)>),
    Array(Vec<Json>),
    String(String),
    /// Ham sayı metni (`GetRawText`).
    Number(String),
    True,
    False,
    Null,
}

/// `JsonDocument.Parse(utf8, new JsonDocumentOptions { MaxDepth = max_depth })`. Hata ayrıntısı taşımaz
/// (çağıranlar yalnız `JsonException` yakalardı).
pub(crate) fn parse(utf8: &[u8], max_depth: usize) -> Result<Json, ()> {
    let text = std::str::from_utf8(utf8).map_err(|_| ())?;
    let mut reader = Reader { s: text, pos: 0, max_depth };
    reader.skip_whitespace();
    let value = reader.parse_value(0)?;
    reader.skip_whitespace();
    if reader.pos != reader.s.len() {
        return Err(());
    }
    Ok(value)
}

impl Json {
    /// `ValueKind == Object`.
    pub fn is_object(&self) -> bool {
        matches!(self, Json::Object(_))
    }

    /// `ValueKind == Number`.
    pub fn is_number(&self) -> bool {
        matches!(self, Json::Number(_))
    }

    /// `ValueKind == Null`.
    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }

    /// `GetString()` (yalnız dize değerlerde).
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    /// `EnumerateArray()`; dizi değilse `None`.
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    /// `EnumerateObject()` (yinelenenler dahil, belge sırasıyla); nesne değilse `None`.
    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Object(properties) => Some(properties),
            _ => None,
        }
    }

    /// `TryGetProperty(name, out value)`: nesne değilse `None` (C# `InvalidOperationException` atardı; çağıranlar
    /// önce tür denetler). Yinelenen anahtarda sonuncusu (.NET geriye doğru arar).
    pub fn try_get_property(&self, name: &str) -> Option<&Json> {
        match self {
            Json::Object(properties) => properties.iter().rev().find(|(k, _)| k == name).map(|(_, v)| v),
            _ => None,
        }
    }

    /// `TryGetInt32`: yalnız tam sayı yazımı ("1.0", "1e2" olmaz) ve aralık içi.
    pub fn try_get_i32(&self) -> Option<i32> {
        self.try_get_i64().and_then(|v| i32::try_from(v).ok())
    }

    /// `TryGetInt64`.
    pub fn try_get_i64(&self) -> Option<i64> {
        match self {
            Json::Number(raw) if raw.bytes().enumerate().all(|(i, b)| b.is_ascii_digit() || (i == 0 && b == b'-')) => raw.parse::<i64>().ok(),
            _ => None,
        }
    }
}

struct Reader<'a> {
    s: &'a str,
    pos: usize,
    max_depth: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.as_bytes().get(self.pos).copied()
    }

    // Yalnız ' ', '\t', '\r', '\n'.
    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.pos += 1;
        }
    }

    fn parse_value(&mut self, depth: usize) -> Result<Json, ()> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => self.parse_object(depth),
            Some(b'[') => self.parse_array(depth),
            Some(b'"') => Ok(Json::String(self.parse_string()?)),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(b't') => self.literal("true", Json::True),
            Some(b'f') => self.literal("false", Json::False),
            Some(b'n') => self.literal("null", Json::Null),
            _ => Err(()),
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, ()> {
        if self.s[self.pos..].starts_with(word) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(())
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<Json, ()> {
        // `Utf8JsonReader`: açılışta geçerli derinlik MaxDepth'e ulaştıysa hata.
        if depth >= self.max_depth {
            return Err(());
        }
        self.pos += 1;
        let mut properties = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Object(properties));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(());
            }
            let name = self.parse_string()?;
            self.skip_whitespace();
            if self.peek() != Some(b':') {
                return Err(());
            }
            self.pos += 1;
            let value = self.parse_value(depth + 1)?;
            properties.push((name, value));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Object(properties));
                }
                _ => return Err(()),
            }
        }
    }

    fn parse_array(&mut self, depth: usize) -> Result<Json, ()> {
        if depth >= self.max_depth {
            return Err(());
        }
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.parse_value(depth + 1)?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Array(items));
                }
                _ => return Err(()),
            }
        }
    }

    fn parse_number(&mut self) -> Result<Json, ()> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => self.digits(),
            _ => return Err(()),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(());
            }
            self.digits();
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(());
            }
            self.digits();
        }
        Ok(Json::Number(self.s[start..self.pos].to_string()))
    }

    fn digits(&mut self) {
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
    }

    fn parse_string(&mut self) -> Result<String, ()> {
        self.pos += 1; // açılış tırnağı
        let mut out = String::new();
        loop {
            let start = self.pos;
            // Kaçışsız parça: tırnak, ters bölü ya da denetim karakterine kadar.
            while let Some(c) = self.peek() {
                if c == b'"' || c == b'\\' || c < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            out.push_str(&self.s[start..self.pos]);
            match self.peek() {
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    match self.peek() {
                        Some(b'"') => out.push('"'),
                        Some(b'\\') => out.push('\\'),
                        Some(b'/') => out.push('/'),
                        Some(b'b') => out.push('\u{0008}'),
                        Some(b'f') => out.push('\u{000C}'),
                        Some(b'n') => out.push('\n'),
                        Some(b'r') => out.push('\r'),
                        Some(b't') => out.push('\t'),
                        Some(b'u') => {
                            let unit = self.hex4()?;
                            self.push_unit(&mut out, unit)?;
                            continue;
                        }
                        _ => return Err(()),
                    }
                    self.pos += 1;
                }
                _ => return Err(()),
            }
        }
    }

    // `\uXXXX`: pos 'u' üzerindeyken çağrılır, dört rakamdan sonrasına ilerler.
    fn hex4(&mut self) -> Result<u16, ()> {
        let digits = self.s.get(self.pos + 1..self.pos + 5).ok_or(())?;
        if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(());
        }
        self.pos += 5;
        u16::from_str_radix(digits, 16).map_err(|_| ())
    }

    // Yedek çifti birleştirilir; eşsiz yedek hata (bkz. modül notu).
    fn push_unit(&mut self, out: &mut String, unit: u16) -> Result<(), ()> {
        if (0xD800..0xDC00).contains(&unit) {
            if !self.s[self.pos..].starts_with("\\u") {
                return Err(());
            }
            self.pos += 1; // 'u' konumuna (hex4 beklentisi)
            let low = self.hex4()?;
            if !(0xDC00..0xE000).contains(&low) {
                return Err(());
            }
            let c = 0x10000 + (((unit as u32) - 0xD800) << 10) + ((low as u32) - 0xDC00);
            out.push(char::from_u32(c).ok_or(())?);
            return Ok(());
        }

        out.push(char::from_u32(unit as u32).ok_or(())?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_duplicate_keys_and_last_wins_on_lookup() {
        let doc = parse(br#"{ "a": 1, "a": 2 }"#, 8).unwrap();
        assert_eq!(2, doc.as_object().unwrap().len());
        assert_eq!(Some(2), doc.try_get_property("a").and_then(Json::try_get_i32));
    }

    #[test]
    fn integers_only_for_try_get() {
        let doc = parse(b"[1, 1.0, 1e2, -5, 99999999999]", 8).unwrap();
        let items = doc.as_array().unwrap();
        assert_eq!(Some(1), items[0].try_get_i32());
        assert_eq!(None, items[1].try_get_i32());
        assert_eq!(None, items[2].try_get_i32());
        assert_eq!(Some(-5), items[3].try_get_i32());
        assert_eq!(None, items[4].try_get_i32());
        assert_eq!(Some(99_999_999_999), items[4].try_get_i64());
    }

    #[test]
    fn rejects_what_json_document_rejects() {
        for bad in ["", "   ", "{", "[1,]", "{\"a\":1,}", "{'a':1}", "01", "1 2", "// c\n{}", "{\"a\":\"\u{1}\"}", "\"\\x\"", "\"\\ud800\"", "nul"] {
            assert!(parse(bad.as_bytes(), 8).is_err(), "{bad:?}");
        }
        assert!(parse(&[0xFF, 0xFE], 8).is_err());
    }

    #[test]
    fn depth_limit_counts_nesting_levels() {
        assert!(parse(b"[[[1]]]", 3).is_ok());
        assert!(parse(b"[[[1]]]", 2).is_err());
        let deep = format!("{}{}", "[".repeat(200), "]".repeat(200));
        assert!(parse(deep.as_bytes(), 16).is_err());
    }

    #[test]
    fn decodes_escapes_and_surrogate_pairs() {
        let doc = parse(br#""a\n\u00e7\ud83d\ude00""#, 8).unwrap();
        assert_eq!(Some("a\n\u{e7}\u{1F600}"), doc.as_str());
    }
}
