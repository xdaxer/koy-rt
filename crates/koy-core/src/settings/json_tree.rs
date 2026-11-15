//! System.Text.Json (`JsonNode.Parse`, `Utf8JsonWriter` girintili yazım) davranışının küçük karşılığı.
//!
//! Ayar dosyası C# ile BİREBİR uyumlu olmalı; `serde_json`un yazımı/okuması üç yerde ayrışır:
//! - Yazım: .NET varsayılan kodlayıcısı ASCII dışını ve `"`, `&`, `'`, `+`, `<`, `>`, `` ` `` karakterlerini `\uXXXX` (büyük
//!   hex) yazar; tam sayı değerli `double` `12` yazılır (`12.0` değil); satır sonu Windows'ta `\r\n`.
//! - Okuma: `1e400` hata vermeden `+Infinity` okunur (profil doğrulaması buna dayanır); yinelenen anahtar hatadır
//!   (C#'ta `ArgumentException` → profil `MalformedJson`); en çok 64 iç içe düzey.
//! - Hata yolu: `JsonException.Path` karşılığı olarak okuma hatası [`Seg`] dizisiyle bildirilir ve
//!   [`drop_at`] C# `SettingsStore.DropAt`'in yaptığı onarımı uygular.

use serde_json::{Map, Number, Value};

/// JSON ayrıştırma hatası (C# `JsonException`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError(pub String);

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for JsonError {}

/// `1e400` gibi aşırı büyük sayı: `Value::Number` sonsuzu taşıyamaz, bu yüzden bu işaretçi metinle temsil edilir.
/// Yalnız `double` çözücüsü bunu `±inf` olarak okur; başka her türde hata olur (C#'ta da bool/int/enum'a okunamaz).
pub const POSITIVE_INFINITY_TOKEN: &str = "\u{0}+Infinity";
pub const NEGATIVE_INFINITY_TOKEN: &str = "\u{0}-Infinity";

const MAX_DEPTH: usize = 64;

/// JSON metnini ayrıştırır (`JsonNode.Parse(string)`).
pub fn parse(text: &str) -> Result<Value, JsonError> {
    let mut p = Parser { s: text.as_bytes(), pos: 0 };
    p.skip_ws();
    let v = p.value(0)?;
    p.skip_ws();
    if p.pos != p.s.len() {
        return Err(p.err("beklenmeyen karakter"));
    }
    Ok(v)
}

/// Dosyadan okunan baytlar için (`JsonNode.Parse(Stream)`): UTF-8 BOM atlanır.
pub fn parse_bytes(bytes: &[u8]) -> Result<Value, JsonError> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|_| JsonError("geçersiz UTF-8".into()))?;
    parse(text)
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn err(&self, msg: &str) -> JsonError {
        JsonError(format!("{msg} (konum {})", self.pos))
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\r' | b'\n') = self.peek() {
            self.pos += 1;
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, JsonError> {
        match self.peek() {
            None => Err(self.err("beklenmeyen son")),
            Some(b'{') => self.object(depth + 1),
            Some(b'[') => self.array(depth + 1),
            Some(b'"') => Ok(Value::String(self.string()?)),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.err("beklenmeyen karakter")),
        }
    }

    fn literal(&mut self, word: &str, v: Value) -> Result<Value, JsonError> {
        if self.s[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(v)
        } else {
            Err(self.err("geçersiz sabit"))
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, JsonError> {
        if depth > MAX_DEPTH {
            return Err(self.err("en büyük derinlik aşıldı"));
        }
        self.pos += 1;
        let mut map = Map::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(map));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("özellik adı bekleniyordu"));
            }
            let key = self.string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(self.err("':' bekleniyordu"));
            }
            self.pos += 1;
            self.skip_ws();
            let v = self.value(depth)?;
            // C# ile aynı: yinelenen anahtar JsonNode'da ArgumentException'dır; burada ayrıştırma hatası sayılır.
            if map.contains_key(&key) {
                return Err(self.err("yinelenen özellik"));
            }
            map.insert(key, v);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(map));
                }
                _ => return Err(self.err("',' ya da '}' bekleniyordu")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, JsonError> {
        if depth > MAX_DEPTH {
            return Err(self.err("en büyük derinlik aşıldı"));
        }
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value(depth)?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.err("',' ya da ']' bekleniyordu")),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        if self.pos + 4 > self.s.len() {
            return Err(self.err("geçersiz kaçış"));
        }
        let mut n = 0u32;
        for i in 0..4 {
            let d = (self.s[self.pos + i] as char).to_digit(16).ok_or_else(|| self.err("geçersiz kaçış"))?;
            n = n * 16 + d;
        }
        self.pos += 4;
        Ok(n)
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.pos += 1; // açılış tırnağı
        let mut out: Vec<u8> = Vec::new();
        loop {
            let b = self.peek().ok_or_else(|| self.err("kapanmamış metin"))?;
            self.pos += 1;
            match b {
                b'"' => break,
                b'\\' => {
                    let e = self.peek().ok_or_else(|| self.err("geçersiz kaçış"))?;
                    self.pos += 1;
                    let ch: char = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let hi = self.hex4()?;
                            if (0xD800..0xDC00).contains(&hi) {
                                if self.s[self.pos..].starts_with(b"\\u") {
                                    self.pos += 2;
                                    let lo = self.hex4()?;
                                    if !(0xDC00..0xE000).contains(&lo) {
                                        return Err(self.err("geçersiz vekil çifti"));
                                    }
                                    char::from_u32(0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00))
                                        .ok_or_else(|| self.err("geçersiz vekil çifti"))?
                                } else {
                                    return Err(self.err("yalnız vekil"));
                                }
                            } else if (0xDC00..0xE000).contains(&hi) {
                                return Err(self.err("yalnız vekil"));
                            } else {
                                char::from_u32(hi).ok_or_else(|| self.err("geçersiz karakter"))?
                            }
                        }
                        _ => return Err(self.err("geçersiz kaçış")),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                0x00..=0x1F => return Err(self.err("metinde denetim karakteri")),
                _ => out.push(b),
            }
        }
        String::from_utf8(out).map_err(|_| self.err("geçersiz UTF-8"))
    }

    fn number(&mut self) -> Result<Value, JsonError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                while let Some(b'0'..=b'9') = self.peek() {
                    self.pos += 1;
                }
            }
            _ => return Err(self.err("geçersiz sayı")),
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') {
            is_float = true;
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.err("geçersiz sayı"));
            }
            while let Some(b'0'..=b'9') = self.peek() {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            is_float = true;
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.err("geçersiz sayı"));
            }
            while let Some(b'0'..=b'9') = self.peek() {
                self.pos += 1;
            }
        }
        let text = std::str::from_utf8(&self.s[start..self.pos]).unwrap_or("0");
        if !is_float {
            if let Ok(i) = text.parse::<i64>() {
                return Ok(Value::Number(Number::from(i)));
            }
            if let Ok(u) = text.parse::<u64>() {
                return Ok(Value::Number(Number::from(u)));
            }
        }
        let f: f64 = text.parse().map_err(|_| self.err("geçersiz sayı"))?;
        if f.is_infinite() {
            return Ok(Value::String(
                if f > 0.0 { POSITIVE_INFINITY_TOKEN } else { NEGATIVE_INFINITY_TOKEN }.to_string(),
            ));
        }
        Ok(Number::from_f64(f).map(Value::Number).unwrap_or(Value::Null))
    }
}

// ---------------------------------------------------------------- yazım

/// Satır sonu: `Environment.NewLine` (Windows'ta `\r\n`).
const NEW_LINE: &str = if cfg!(windows) { "\r\n" } else { "\n" };

/// `WriteIndented = true` ile System.Text.Json çıktısı (2 boşluk girinti, `"anahtar": değer`).
pub fn to_pretty_string(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value, 0);
    out
}

fn indent(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str("  ");
    }
}

fn write_value(out: &mut String, v: &Value, level: usize) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&format_number(n)),
        Value::String(s) => write_string(out, s),
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(NEW_LINE);
                indent(out, level + 1);
                write_value(out, item, level + 1);
            }
            out.push_str(NEW_LINE);
            indent(out, level);
            out.push(']');
        }
        Value::Object(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (k, item)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(NEW_LINE);
                indent(out, level + 1);
                write_string(out, k);
                out.push_str(": ");
                write_value(out, item, level + 1);
            }
            out.push_str(NEW_LINE);
            indent(out, level);
            out.push('}');
        }
    }
}

fn format_number(n: &Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    format_double(n.as_f64().unwrap_or(0.0))
}

/// .NET `double.ToString("R")`: en kısa gidiş-dönüş basamakları; üs ≥ 15 ya da < -5 ise `E+XX` / `E-XX` gösterimi.
pub fn format_double(x: f64) -> String {
    if !x.is_finite() {
        return "0".into();
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let sci = format!("{x:e}");
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    if !(-5..15).contains(&exp) {
        let sign = if exp < 0 { '-' } else { '+' };
        return format!("{mant}E{sign}{:02}", exp.abs());
    }
    format!("{x}")
}

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            '\\' => out.push_str("\\\\"),
            '"' | '&' | '\'' | '+' | '<' | '>' | '`' => push_u(out, c as u32),
            c if (c as u32) < 0x20 || (c as u32) == 0x7F => push_u(out, c as u32),
            c if (c as u32) < 0x7F => out.push(c),
            c => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    push_u(out, *unit as u32);
                }
            }
        }
    }
    out.push('"');
}

fn push_u(out: &mut String, unit: u32) {
    out.push_str(&format!("\\u{unit:04X}"));
}

// ---------------------------------------------------------------- hata yolu ve onarım

/// `JsonException.Path` parçası.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seg {
    /// Nesne özelliği (kayıt alanı).
    Prop(String),
    /// Dizi öğesi.
    Index(usize),
    /// Sözlük anahtarı.
    Key(String),
}

/// Okuma hatasının yeri. Çağıranlar yolu içten dışa doğru başa ekleyerek kurar.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Fail {
    pub path: Vec<Seg>,
}

impl Fail {
    /// Şu anki düğümün kendisi okunamadı.
    pub fn here() -> Fail {
        Fail { path: Vec::new() }
    }

    pub fn under(mut self, seg: Seg) -> Fail {
        self.path.insert(0, seg);
        self
    }
}

/// STJ yol yazımında özel karakterli ad `['ad']` olur: harf, rakam ve `_` dışında karakter varsa özeldir.
fn is_special_name(name: &str) -> bool {
    name.chars().any(|c| !(c.is_alphanumeric() || c == '_'))
}

/// `SettingsStore.DropAt`: hata yolundaki alanı (ya da yol bir listeden geçiyorsa en içteki liste öğesini) ağaçtan atar.
/// Yol bir sözlük anahtarı içeriyorsa ve anahtar özel karakter taşıyorsa (STJ `['..']` yazar) üst düzey bölümün tamamı düşer.
/// Dönüş: bir şey atıldı mı.
pub fn drop_at(root: &mut Value, path: &[Seg]) -> bool {
    let top = match path.first() {
        Some(Seg::Prop(name)) => name.clone(),
        _ => return false,
    };
    let remove_top = |root: &mut Value| root.as_object_mut().is_some_and(|o| o.shift_remove(&top).is_some());

    if path.iter().any(|s| matches!(s, Seg::Key(k) if is_special_name(k))) {
        return remove_top(root);
    }

    // Hedef: yoldaki son dizi öğesi varsa o (kesilmiş yol), yoksa yolun son anahtarı.
    let cut = path.iter().rposition(|s| matches!(s, Seg::Index(_)));
    let target = match cut {
        Some(i) => &path[..=i],
        None => path,
    };

    let (last, parents) = match target.split_last() {
        Some(x) => x,
        None => return false,
    };
    let mut node: &mut Value = root;
    for seg in parents {
        let next = match (seg, node) {
            (Seg::Prop(k) | Seg::Key(k), Value::Object(o)) => o.get_mut(k),
            (Seg::Index(i), Value::Array(a)) => a.get_mut(*i),
            _ => None,
        };
        match next {
            Some(n) => node = n,
            None => return remove_top(root),
        }
    }
    match (last, node) {
        (Seg::Prop(k) | Seg::Key(k), Value::Object(o)) => {
            if o.shift_remove(k).is_some() {
                true
            } else {
                remove_top(root)
            }
        }
        (Seg::Index(i), Value::Array(a)) if *i < a.len() => {
            a.remove(*i);
            true
        }
        _ => remove_top(root),
    }
}
