use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use super::{is_control_unit, utf16_len};
use crate::text;

/// Bir kullanıcı stil girdisinin değer türü. Her izinli token'ın türü sabittir; dosya türü seçemez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserStyleKind {
    /// SolidColorBrush kaynağı: "#RRGGBB" ya da "#AARRGGBB".
    Brush,
    /// Color kaynağı: "#RRGGBB" ya da "#AARRGGBB".
    Color,
    /// double kaynağı: sonlu, token'ın aralığında sayı.
    Number,
    /// CornerRadius kaynağı: "a" ya da "sol-üst,sağ-üst,sağ-alt,sol-alt".
    CornerRadius,
    /// Thickness kaynağı: "a" ya da "sol,üst,sağ,alt".
    Thickness,
    /// FontFamily kaynağı: düz yazı tipi adı (ya da virgülle ayrılmış yedek listesi).
    FontFamily,
}

/// İzinli bir kaynak anahtarı. `min` ve `max` yalnız sayı, köşe ve kalınlık türlerinde anlamlıdır.
#[derive(Debug, Clone, PartialEq)]
pub struct UserStyleToken {
    pub key: String,
    pub kind: UserStyleKind,
    pub min: f64,
    pub max: f64,
}

impl UserStyleToken {
    pub fn new(key: &str, kind: UserStyleKind, min: f64, max: f64) -> Self {
        UserStyleToken { key: key.to_string(), kind, min, max }
    }

    /// `Min`/`Max` varsayılanlarıyla (0, 0): renk ve yazı tipi türleri için.
    pub fn unbounded(key: &str, kind: UserStyleKind) -> Self {
        Self::new(key, kind, 0.0, 0.0)
    }
}

/// ARGB renk (WPF'siz). "#RRGGBB" için A = 255.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct UserStyleColor {
    pub a: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl UserStyleColor {
    pub const fn new(a: u8, r: u8, g: u8, b: u8) -> Self {
        UserStyleColor { a, r, g, b }
    }
}

/// Dört kenarlı değer. Köşe yarıçapında sıra sol-üst, sağ-üst, sağ-alt, sol-alt; kalınlıkta sol, üst, sağ, alt
/// (WPF'in kendi sıraları). Tek sayı verilince dördü de aynıdır.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UserStyleQuad {
    pub first: f64,
    pub second: f64,
    pub third: f64,
    pub fourth: f64,
}

impl UserStyleQuad {
    pub const fn new(first: f64, second: f64, third: f64, fourth: f64) -> Self {
        UserStyleQuad { first, second, third, fourth }
    }

    pub const fn uniform(value: f64) -> Self {
        Self::new(value, value, value, value)
    }
}

/// Doğrulanmış kullanıcı stil değeri. Yalnız `UserStyleParser` üretir; türe göre ilgili alan doludur,
/// diğerleri varsayılandır. App bunu donmuş WPF nesnelerine çevirir (Core WPF bilmez).
#[derive(Debug, Clone, PartialEq)]
pub struct UserStyleValue {
    pub kind: UserStyleKind,
    pub color: UserStyleColor,
    pub number: f64,
    pub quad: UserStyleQuad,
    pub font_family: String,
}

impl UserStyleValue {
    fn of_kind(kind: UserStyleKind) -> Self {
        UserStyleValue {
            kind,
            color: UserStyleColor::default(),
            number: 0.0,
            quad: UserStyleQuad::default(),
            font_family: String::new(),
        }
    }

    pub fn for_brush(color: UserStyleColor) -> Self {
        UserStyleValue { color, ..Self::of_kind(UserStyleKind::Brush) }
    }

    pub fn for_color(color: UserStyleColor) -> Self {
        UserStyleValue { color, ..Self::of_kind(UserStyleKind::Color) }
    }

    pub fn for_number(number: f64) -> Self {
        UserStyleValue { number, ..Self::of_kind(UserStyleKind::Number) }
    }

    pub fn for_corner_radius(quad: UserStyleQuad) -> Self {
        UserStyleValue { quad, ..Self::of_kind(UserStyleKind::CornerRadius) }
    }

    pub fn for_thickness(quad: UserStyleQuad) -> Self {
        UserStyleValue { quad, ..Self::of_kind(UserStyleKind::Thickness) }
    }

    pub fn for_font_family(name: &str) -> Self {
        UserStyleValue { font_family: name.to_string(), ..Self::of_kind(UserStyleKind::FontFamily) }
    }
}

/// Bir girdinin neden yok sayıldığının makine okunur karşılığı (App bunu yerelleştirir).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserStyleDiagnosticCode {
    InvalidJson,
    NotAnObject,
    FileTooLarge,
    FileUnreadable,
    TooManyEntries,
    UnknownToken,
    DuplicateToken,
    WrongValueType,
    ValueTooLong,
    BadColor,
    BadNumber,
    BadQuad,
    BadFontFamily,
    OutOfRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserStyleDiagnostic {
    /// Anahtar adı (kırpılmış, denetim karakterleri temizlenmiş); dosya düzeyi sorunlarda "(file)".
    pub key: String,
    /// Kısa İngilizce açıklama (günlük ve tanı için). Arayüz `code`'dan yerelleştirir.
    pub reason: String,
    pub code: UserStyleDiagnosticCode,
}

impl UserStyleDiagnostic {
    pub fn new(key: &str, reason: &str, code: UserStyleDiagnosticCode) -> Self {
        UserStyleDiagnostic { key: key.to_string(), reason: reason.to_string(), code }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserStyleParseResult {
    pub valid_entries: HashMap<String, UserStyleValue>,
    pub diagnostics: Vec<UserStyleDiagnostic>,
    /// Rapor sınırı yüzünden listeye alınmayan tanı sayısı.
    pub omitted_diagnostics: i32,
}

impl UserStyleParseResult {
    pub fn new(valid_entries: HashMap<String, UserStyleValue>, diagnostics: Vec<UserStyleDiagnostic>, omitted_diagnostics: i32) -> Self {
        UserStyleParseResult { valid_entries, diagnostics, omitted_diagnostics }
    }

    /// `UserStyleParseResult.Empty`.
    pub fn empty() -> Self {
        Self::new(HashMap::new(), Vec::new(), 0)
    }
}

/// Güvenlik sınırı: kullanıcı stil dosyasında yalnız bu listedeki anahtarlar geçersiz kılınabilir. Liste
/// Theme.Dark.xaml, Theme.Light.xaml ve Tokens.xaml'deki fırça, renk, sayı, köşe ve yazı tipi anahtarlarından
/// türer (appearance/tests/user_style_overrides_tests.rs XAML dosyalarıyla eşitliği denetler).
/// Hiçbiri pencere ya da tuval boyutu değildir (AGENTS #13). XAML, yol, tip adı ya da kod burada yoktur.
pub struct AllowedTokens;

// Tema fırçaları (Koyu ve Açık aynı anahtarları taşır).
const THEME_BRUSHES: &[&str] = &[
    "KoyBgWindow", "KoyBgShell", "KoyBgRow", "KoyBgControl", "KoyBgSegment", "KoyBgHover", "KoyBgActive",
    "KoyBgNavHover", "KoyBgNavActive", "KoyBorderHairline", "KoyBorderShell", "KoyBorderStrong", "KoyBorderControl",
    "KoyTextPrimary", "KoyTextSecondary", "KoyTextMuted", "KoyTextDisabled", "KoyAccent", "KoyAccentHover",
    "KoyAccentPressed", "KoyOnAccent", "KoyAccentText", "KoyFocusRing", "KoyAccentSubtle", "KoyBgInfo", "KoyFgInfo",
    "KoyDangerText", "KoyDangerBorder", "KoyDangerBg", "KoyScrim", "KoyStatusOk", "KoyStatusWarn",
    "KoyToggleTrackOff", "KoyToggleKnobOff", "KoyToggleKnobOn",
    // Dock (Koyu ve Açık aynı değerleri taşır; dock temadan bağımsız koyu yüzeydir, tema paketi ezer).
    "KoyDockSurface", "KoyDockCapsuleSurface", "KoyDockBorder", "KoyDockCapsuleBorder", "KoyDockSeparator",
    "KoyDockDivider", "KoyDockHover", "KoyDockActive", "KoyDockPill", "KoyDockCover", "KoyDockIndicator",
    "KoyDockText", "KoyDockTextSecondary", "KoyDockGlyph", "KoyDockClockText", "KoyDockClockSubtext",
    "KoyDockPreviewSurface", "KoyDockPreviewText",
];

// Tokens.xaml fırçaları: bağlam menüsü, kaydırma çubuğu ve çentik (temadan bağımsız koyu yüzey).
const TOKEN_BRUSHES: &[&str] = &[
    "KoyMenuBg", "KoyMenuBorder", "KoyMenuText", "KoyMenuTextMuted", "KoyMenuTextDisabled", "KoyMenuHover", "KoyMenuSeparator",
    "KoyScrollThumb", "KoyScrollThumbHover", "KoyScrollThumbDrag",
    "KoyNotchText", "KoyNotchTextSecondary", "KoyNotchTextOnLight", "KoyNotchHover", "KoyNotchPressed", "KoyNotchActive",
    "KoyNotchTextMuted", "KoyNotchDivider", "KoyNotchSurface", "KoyNotchCard", "KoyNotchCardBorder", "KoyNotchChip", "KoyNotchMuted",
    "KoyNotchDim", "KoyNotchOk", "KoyNotchAccentHome", "KoyNotchAccentFocus", "KoyNotchAccentShelf",
    "KoyNotchAccentClipboard", "KoyNotchAccentSystem", "KoyNotchAccentControl", "KoyNotchMusicCard",
    "KoyNotchMusicCover", "KoyNotchMusicSub", "KoyNotchMusicSpectrum", "KoyNotchWeatherCard", "KoyNotchWeatherSoft",
    "KoyNotchWeatherIconBg", "KoyNotchFocusCard", "KoyNotchFocusTrack", "KoyNotchFocusButton", "KoyNotchFocusSoft",
    "KoyNotchShelfCard", "KoyNotchShelfChip", "KoyNotchShelfChipText", "KoyNotchShelfSoft", "KoyNotchShelfDash",
    "KoyNotchControlCard", "KoyNotchControlTrack", "KoyNotchControlSoft", "KoyNotchControlTile", "KoyNotchBrightness",
    "KoyNotchControlBattery", "KoyNotchControlIconBg", "KoyNotchRowHover", "KoyNotchRowError",
    "KoyNotchAccentCalendar", "KoyNotchCalendarCard", "KoyNotchCalendarChip", "KoyNotchCalendarSoft",
    "KoyNotchClipCard", "KoyNotchClipChip", "KoyNotchClipChipText", "KoyNotchClipSoft",
    "KoyNotchSystemCard", "KoyNotchSystemTrack", "KoyNotchSystemSoft", "KoyNotchNetCard", "KoyNotchNetSoft",
    "KoyNotchDash", "KoyNotchCheckBorder", "KoyNotchDoneText", "KoyNotchTrack", "KoyNotchIconBg",
];

impl AllowedTokens {
    /// Bütün izinli tokenlar (anahtar → tanım).
    pub fn all() -> &'static HashMap<String, UserStyleToken> {
        static BY_KEY: OnceLock<HashMap<String, UserStyleToken>> = OnceLock::new();
        BY_KEY.get_or_init(Self::build)
    }

    pub fn try_get(key: &str) -> Option<&'static UserStyleToken> {
        Self::all().get(key)
    }

    fn build() -> HashMap<String, UserStyleToken> {
        let mut list: Vec<UserStyleToken> = Vec::new();

        for key in THEME_BRUSHES {
            list.push(UserStyleToken::unbounded(key, UserStyleKind::Brush));
        }
        for key in TOKEN_BRUSHES {
            list.push(UserStyleToken::unbounded(key, UserStyleKind::Brush));
        }

        list.push(UserStyleToken::unbounded("KoyShadowColor", UserStyleKind::Color));

        list.push(UserStyleToken::unbounded("KoyFont", UserStyleKind::FontFamily));
        list.push(UserStyleToken::unbounded("KoyFontMono", UserStyleKind::FontFamily));
        list.push(UserStyleToken::unbounded("KoyNotchFont", UserStyleKind::FontFamily));
        list.push(UserStyleToken::unbounded("KoyNotchDisplayFont", UserStyleKind::FontFamily));
        list.push(UserStyleToken::unbounded("KoyClockFont", UserStyleKind::FontFamily));

        // Yazı boyutları (DIP): okunur kalsın diye dar aralık.
        for key in [
            "KoyFontPageTitle", "KoyFontSubtitle", "KoyFontSection", "KoyFontRowTitle", "KoyFontBody",
            "KoyFontControl", "KoyFontCaption", "KoyFontKey",
        ] {
            list.push(UserStyleToken::new(key, UserStyleKind::Number, 8.0, 40.0));
        }

        // Boşluk ölçeği (DIP).
        for key in ["KoySpace1", "KoySpace2", "KoySpace3", "KoySpace4", "KoySpace5", "KoySpace6"] {
            list.push(UserStyleToken::new(key, UserStyleKind::Number, 0.0, 64.0));
        }

        // Köşe yarıçapları.
        for key in [
            "KoyRadiusWindow", "KoyRadiusWindowTop", "KoyRadiusWindowBottomLeft", "KoyRadiusRow",
            "KoyRadiusControl", "KoyRadiusSmall",
        ] {
            list.push(UserStyleToken::new(key, UserStyleKind::CornerRadius, 0.0, 32.0));
        }

        // Ölçüler (DIP).
        list.push(UserStyleToken::new("KoyControlHeight", UserStyleKind::Number, 20.0, 56.0));
        list.push(UserStyleToken::new("KoyRowMinHeight", UserStyleKind::Number, 32.0, 96.0));
        list.push(UserStyleToken::new("KoyNavItemHeight", UserStyleKind::Number, 24.0, 56.0));
        list.push(UserStyleToken::new("KoyIconSize", UserStyleKind::Number, 10.0, 32.0));
        list.push(UserStyleToken::new("KoyIconSizeLarge", UserStyleKind::Number, 12.0, 40.0));
        list.push(UserStyleToken::new("KoyIconStroke", UserStyleKind::Number, 0.5, 4.0));

        list.into_iter().map(|t| (t.key.clone(), t)).collect()
    }
}

/// Kullanıcı stil dosyasının (user-theme.json) kısıtlı ayrıştırıcısı. Saf fonksiyon, asla fırlatmaz.
/// Yalnız kendi küçük JSON okuyucusunu kullanır: XAML okunmaz, tip adı çözülmez, yansıma yoktur. Bilinmeyen anahtar,
/// bozuk değer ve bozuk JSON tanı olarak raporlanır; geçerli girdiler yine uygulanır.
pub struct UserStyleParser;

type Failure = (UserStyleDiagnosticCode, String);

const MAX_ECHOED_KEY_LENGTH: usize = 64;
const MAX_JSON_DEPTH: usize = 4;

impl UserStyleParser {
    /// Dosya boyutu üst sınırı (UTF-8 bayt).
    pub const MAX_BYTES: i32 = 64 * 1024;

    /// Okunan en çok girdi sayısı (bilinmeyenler dahil); fazlası yok sayılır.
    pub const MAX_ENTRIES: i32 = 512;

    /// Raporlanan en çok tanı; fazlası yalnız sayılır.
    pub const MAX_DIAGNOSTICS: i32 = 64;

    /// Tek bir değerin en çok uzunluğu (karakter).
    pub const MAX_VALUE_LENGTH: i32 = 128;

    /// Yazı tipi adı üst sınırı (karakter).
    pub const MAX_FONT_FAMILY_LENGTH: i32 = 96;

    /// Dosya düzeyi tanılarda anahtar yerine yazılır.
    pub const FILE_KEY: &'static str = "(file)";

    pub fn parse(json_content: &str) -> UserStyleParseResult {
        Self::parse_with_tokens(Some(json_content), AllowedTokens::all())
    }

    /// İzinli token tablosunu dışarıdan alan aşırı yükleme (testler ve ileride başka kümeler için).
    pub fn parse_with_tokens(json_content: Option<&str>, tokens: &HashMap<String, UserStyleToken>) -> UserStyleParseResult {
        let mut diagnostics: Vec<UserStyleDiagnostic> = Vec::new();
        let mut omitted: i32 = 0;

        let mut report = |key: &str, code: UserStyleDiagnosticCode, reason: &str| {
            // Dosya düzeyi tanı (en çok bir tane) sınırdan bağımsız yazılır: "çok girdi" uyarısı kaybolmasın.
            if (diagnostics.len() as i32) < Self::MAX_DIAGNOSTICS || key == Self::FILE_KEY {
                diagnostics.push(UserStyleDiagnostic::new(&echo(key), reason, code));
            } else {
                omitted += 1;
            }
        };

        let mut entries: HashMap<String, UserStyleValue> = HashMap::new();

        let json_content = match json_content {
            Some(c) if !text::is_null_or_white_space(Some(c)) => c,
            _ => {
                report(Self::FILE_KEY, UserStyleDiagnosticCode::InvalidJson, "File is empty or not valid JSON.");
                return UserStyleParseResult::new(entries, diagnostics, omitted);
            }
        };

        // Boyut sınırı ayrıştırmadan önce: dev girdi için belge bile kurulmaz. Kaba üst sınır uzunluktan:
        // bir karakter en çok 3 UTF-8 bayttır (yedek çiftleri 2 karakter / 4 bayt).
        if utf16_len(json_content) > Self::MAX_BYTES as usize || json_content.len() > Self::MAX_BYTES as usize {
            report(
                Self::FILE_KEY,
                UserStyleDiagnosticCode::FileTooLarge,
                &format!("File is larger than {} KB.", Self::MAX_BYTES / 1024),
            );
            return UserStyleParseResult::new(entries, diagnostics, omitted);
        }

        let Ok(root) = strict_json::parse_document(json_content, MAX_JSON_DEPTH) else {
            report(Self::FILE_KEY, UserStyleDiagnosticCode::InvalidJson, "File is empty or not valid JSON.");
            return UserStyleParseResult::new(entries, diagnostics, omitted);
        };

        let strict_json::Json::Object(properties) = root else {
            report(
                Self::FILE_KEY,
                UserStyleDiagnosticCode::NotAnObject,
                "The top level must be a JSON object of key-value pairs.",
            );
            return UserStyleParseResult::new(entries, diagnostics, omitted);
        };

        // Yalnız izinli (ve bu yüzden sınırlı sayıdaki) anahtarlar izlenir; bilinmeyenler için küme büyümez.
        let mut seen: HashSet<&str> = HashSet::new();
        let mut count = 0;

        for (key, element) in &properties {
            count += 1;
            if count > Self::MAX_ENTRIES {
                report(
                    Self::FILE_KEY,
                    UserStyleDiagnosticCode::TooManyEntries,
                    &format!("More than {} entries; the rest were ignored.", Self::MAX_ENTRIES),
                );
                break;
            }

            let Some(token) = tokens.get(key.as_str()) else {
                report(key, UserStyleDiagnosticCode::UnknownToken, "Unknown key; only theme resource keys are allowed.");
                continue;
            };

            if !seen.insert(key.as_str()) {
                report(key, UserStyleDiagnosticCode::DuplicateToken, "Duplicate key; the first value is used.");
                continue;
            }

            let outcome = try_read_text(element, token).and_then(|text| Self::try_parse_value(token, &text));
            match outcome {
                Err((code, reason)) => report(key, code, &reason),
                Ok(value) => {
                    entries.insert(key.clone(), value);
                }
            }
        }

        UserStyleParseResult::new(entries, diagnostics, omitted)
    }

    /// Tek bir değer metnini token'ın türüne göre doğrular. Hata varsa kodu ve nedeni, yoksa değeri verir.
    /// Dışarıdan çağrılabilir (testler her türü doğrudan dener).
    pub fn try_parse_value(token: &UserStyleToken, text: &str) -> Result<UserStyleValue, Failure> {
        let text = text.trim();

        match token.kind {
            UserStyleKind::Brush | UserStyleKind::Color => {
                let Some(color) = try_parse_color(text) else {
                    return Err((UserStyleDiagnosticCode::BadColor, "Expected a color like #RRGGBB or #AARRGGBB.".to_string()));
                };
                Ok(if token.kind == UserStyleKind::Brush { UserStyleValue::for_brush(color) } else { UserStyleValue::for_color(color) })
            }

            UserStyleKind::Number => {
                let Some(number) = try_parse_number(text) else {
                    return Err((UserStyleDiagnosticCode::BadNumber, "Expected a finite number.".to_string()));
                };
                if number < token.min || number > token.max {
                    return Err((
                        UserStyleDiagnosticCode::OutOfRange,
                        format!("Value must be between {} and {}.", format_number(token.min), format_number(token.max)),
                    ));
                }
                Ok(UserStyleValue::for_number(normalize(number)))
            }

            UserStyleKind::CornerRadius | UserStyleKind::Thickness => {
                let quad = try_parse_quad(token, text)?;
                Ok(if token.kind == UserStyleKind::CornerRadius {
                    UserStyleValue::for_corner_radius(quad)
                } else {
                    UserStyleValue::for_thickness(quad)
                })
            }

            UserStyleKind::FontFamily => {
                if !is_plain_font_family(text) {
                    return Err((
                        UserStyleDiagnosticCode::BadFontFamily,
                        "Expected a plain font name (letters, digits, spaces, - _ . , ( )).".to_string(),
                    ));
                }
                Ok(UserStyleValue::for_font_family(text))
            }
        }
    }
}

// JSON değerini metne indirger. Sayı yalnız sayısal türlerde (Number, CornerRadius, Thickness) kabul edilir;
// nesne, dizi, true/false/null hiçbir türde geçmez.
fn try_read_text(element: &strict_json::Json, token: &UserStyleToken) -> Result<String, Failure> {
    let text = match element {
        strict_json::Json::String(s) => s.clone(),
        strict_json::Json::Number(raw)
            if matches!(token.kind, UserStyleKind::Number | UserStyleKind::CornerRadius | UserStyleKind::Thickness) =>
        {
            raw.clone()
        }
        _ => return Err((UserStyleDiagnosticCode::WrongValueType, "Wrong value type for this key.".to_string())),
    };

    if utf16_len(&text) > UserStyleParser::MAX_VALUE_LENGTH as usize {
        return Err((
            UserStyleDiagnosticCode::ValueTooLong,
            format!("Value is longer than {} characters.", UserStyleParser::MAX_VALUE_LENGTH),
        ));
    }

    Ok(text)
}

fn try_parse_color(text: &str) -> Option<UserStyleColor> {
    let bytes = text.as_bytes();
    // Ortak olmayan (ASCII dışı) karakter altıgen rakam olmayacağından bayt uzunluğu yeterlidir; yine de uzunluk
    // C#'ta UTF-16 olduğundan önce kod birimi sayısına bakılır.
    let len = utf16_len(text);
    if !(len == 7 || len == 9) || bytes.first() != Some(&b'#') {
        return None;
    }

    if !text.chars().skip(1).all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let read_byte = |index: usize| u8::from_str_radix(&text[index..index + 2], 16).unwrap_or(0);

    let mut offset = 1;
    let mut alpha = 255u8;
    if len == 9 {
        alpha = read_byte(1);
        offset = 3;
    }

    Some(UserStyleColor::new(alpha, read_byte(offset), read_byte(offset + 2), read_byte(offset + 4)))
}

// Kültürden bağımsız: "1,5" Türkçe ortamda da aynı sonucu verir (virgül burada ayraç, ondalık değil).
fn try_parse_number(text: &str) -> Option<f64> {
    text.parse::<f64>().ok().filter(|n| n.is_finite())
}

fn try_parse_quad(token: &UserStyleToken, text: &str) -> Result<UserStyleQuad, Failure> {
    let bad_shape = || (UserStyleDiagnosticCode::BadQuad, "Expected one number (\"a\") or four (\"a,b,c,d\").".to_string());

    let parts: Vec<&str> = text.split(',').collect();
    if !(parts.len() == 1 || parts.len() == 4) {
        return Err(bad_shape());
    }

    let mut numbers = [0.0f64; 4];
    for (i, part) in parts.iter().enumerate() {
        let Some(n) = try_parse_number(part.trim()) else {
            return Err(bad_shape());
        };
        if n < token.min || n > token.max {
            return Err((
                UserStyleDiagnosticCode::OutOfRange,
                format!("Each value must be between {} and {}.", format_number(token.min), format_number(token.max)),
            ));
        }
        numbers[i] = normalize(n);
    }

    Ok(if parts.len() == 1 {
        UserStyleQuad::uniform(numbers[0])
    } else {
        UserStyleQuad::new(numbers[0], numbers[1], numbers[2], numbers[3])
    })
}

// `char.IsLetterOrDigit` (L* ve Nd) karşılığı. Rust'ta genel kategori tablosu yok: ASCII dışında "alfabetik ama
// sayısal değil" (harf) ya da "sayısal ama alfabetik değil" (rakam) ile yaklaşılır.
fn is_letter_or_digit(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphanumeric();
    }
    c.is_alphabetic() != c.is_numeric()
}

// Düz ad: yol, URI, "{...}" işaretlemesi, "<...>" etiketi ve denetim karakterleri geçmez. Virgülle ayrılmış
// yedek listesine izin verilir (varsayılan KoyFont da böyle), her parça boş olmamalı.
fn is_plain_font_family(text: &str) -> bool {
    let len = utf16_len(text);
    if len == 0 || len > UserStyleParser::MAX_FONT_FAMILY_LENGTH as usize {
        return false;
    }

    let mut segment_length = 0;
    // C# `foreach (var c in text)` UTF-16 kod birimi gezer: yedek çifti (BMP dışı) birimleri harf sayılmaz.
    for unit in text.encode_utf16() {
        if unit == u16::from(b',') {
            if segment_length == 0 {
                return false;
            }
            segment_length = 0;
            continue;
        }

        if is_control_unit(unit) {
            return false;
        }

        let allowed = match char::from_u32(unit as u32) {
            Some(c) => is_letter_or_digit(c) || matches!(c, ' ' | '-' | '_' | '.' | '(' | ')'),
            None => false,
        };
        if !allowed {
            return false;
        }

        if unit != u16::from(b' ') {
            segment_length += 1;
        }
    }

    segment_length > 0
}

// -0 aralığı geçer ama kanonik değerde "-0" kalmasın.
fn normalize(n: f64) -> f64 {
    if n == 0.0 { 0.0 } else { n }
}

/// `n.ToString("0.##", InvariantCulture)`.
fn format_number(n: f64) -> String {
    let rounded = text::round_digits_away(n, 2);
    let mut s = format!("{rounded:.2}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" { "0".to_string() } else { s }
}

// Rapora giden anahtar kırpılır ve denetim karakterleri temizlenir: tanı listesi dosyadaki içerikle şişirilemez
// ve satır sonu / kaçış dizisi taşıyamaz.
fn echo(key: &str) -> String {
    if key.is_empty() {
        return String::new();
    }

    let units: Vec<u16> = key.encode_utf16().collect();
    let length = units.len().min(MAX_ECHOED_KEY_LENGTH);
    let cleaned: Vec<u16> = units[..length].iter().map(|&u| if is_control_unit(u) { u16::from(b'?') } else { u }).collect();
    String::from_utf16_lossy(&cleaned)
}

/// `System.Text.Json.JsonDocument` davranışlı katı JSON okuyucusu (`MaxDepth` sınırlı, sondaki virgül serbest,
/// yorumlar atlanır). `serde_json` kullanılmaz: yinelenen anahtarlar ve ham sayı metni korunmalı.
mod strict_json {
    #[derive(Debug)]
    pub enum Json {
        Object(Vec<(String, Json)>),
        /// Dizinin içeriği hiçbir türde kullanılmaz; yalnız geçerliliği (ve derinliği) denetlenir.
        Array,
        String(String),
        /// Ham sayı metni (`GetRawText`).
        Number(String),
        Literal,
    }

    struct Reader<'a> {
        s: &'a str,
        pos: usize,
        max_depth: usize,
    }

    pub fn parse_document(s: &str, max_depth: usize) -> Result<Json, ()> {
        let mut r = Reader { s, pos: 0, max_depth };
        let value = r.parse_value(0)?;
        r.skip_trivia()?;
        if r.pos != r.s.len() {
            return Err(());
        }
        Ok(value)
    }

    impl Reader<'_> {
        fn peek(&self) -> Option<u8> {
            self.s.as_bytes().get(self.pos).copied()
        }

        // Boşluk (yalnız ' ', '\t', '\r', '\n') ve yorumlar (// ve /* */).
        fn skip_trivia(&mut self) -> Result<(), ()> {
            loop {
                match self.peek() {
                    Some(b' ' | b'\t' | b'\r' | b'\n') => self.pos += 1,
                    Some(b'/') => match self.s.as_bytes().get(self.pos + 1) {
                        Some(b'/') => {
                            self.pos += 2;
                            while let Some(c) = self.peek() {
                                self.pos += 1;
                                if c == b'\n' {
                                    break;
                                }
                            }
                        }
                        Some(b'*') => {
                            let rest = &self.s[self.pos + 2..];
                            match rest.find("*/") {
                                Some(end) => self.pos += 2 + end + 2,
                                None => return Err(()),
                            }
                        }
                        _ => return Err(()),
                    },
                    _ => return Ok(()),
                }
            }
        }

        fn parse_value(&mut self, depth: usize) -> Result<Json, ()> {
            self.skip_trivia()?;
            match self.peek() {
                Some(b'{') => self.parse_object(depth),
                Some(b'[') => self.parse_array(depth),
                Some(b'"') => Ok(Json::String(self.parse_string()?)),
                Some(b'-' | b'0'..=b'9') => self.parse_number(),
                Some(b't') => self.literal("true"),
                Some(b'f') => self.literal("false"),
                Some(b'n') => self.literal("null"),
                _ => Err(()),
            }
        }

        fn literal(&mut self, word: &str) -> Result<Json, ()> {
            if self.s[self.pos..].starts_with(word) {
                self.pos += word.len();
                Ok(Json::Literal)
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
            let mut props = Vec::new();
            self.skip_trivia()?;
            if self.peek() == Some(b'}') {
                self.pos += 1;
                return Ok(Json::Object(props));
            }
            loop {
                self.skip_trivia()?;
                if self.peek() != Some(b'"') {
                    return Err(());
                }
                let name = self.parse_string()?;
                self.skip_trivia()?;
                if self.peek() != Some(b':') {
                    return Err(());
                }
                self.pos += 1;
                let value = self.parse_value(depth + 1)?;
                props.push((name, value));
                self.skip_trivia()?;
                match self.peek() {
                    Some(b',') => {
                        self.pos += 1;
                        self.skip_trivia()?;
                        // Sondaki virgül serbest.
                        if self.peek() == Some(b'}') {
                            self.pos += 1;
                            return Ok(Json::Object(props));
                        }
                    }
                    Some(b'}') => {
                        self.pos += 1;
                        return Ok(Json::Object(props));
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
            self.skip_trivia()?;
            if self.peek() == Some(b']') {
                self.pos += 1;
                return Ok(Json::Array);
            }
            loop {
                self.parse_value(depth + 1)?;
                self.skip_trivia()?;
                match self.peek() {
                    Some(b',') => {
                        self.pos += 1;
                        self.skip_trivia()?;
                        if self.peek() == Some(b']') {
                            self.pos += 1;
                            return Ok(Json::Array);
                        }
                    }
                    Some(b']') => {
                        self.pos += 1;
                        return Ok(Json::Array);
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

        // Yedek çifti birleştirilir. Eşsiz yedek C#'ta GetString'de fırlatırdı; Rust metni U+FFFD ile sürer.
        fn push_unit(&mut self, out: &mut String, unit: u16) -> Result<(), ()> {
            if (0xD800..0xDC00).contains(&unit) && self.s[self.pos..].starts_with("\\u") {
                let save = self.pos;
                self.pos += 1; // 'u' konumuna (hex4 beklentisi)
                if let Ok(low) = self.hex4() {
                    if (0xDC00..0xE000).contains(&low) {
                        let c = 0x10000 + (((unit as u32) - 0xD800) << 10) + ((low as u32) - 0xDC00);
                        out.push(char::from_u32(c).unwrap_or('\u{FFFD}'));
                        return Ok(());
                    }
                }
                self.pos = save;
            }
            out.push(char::from_u32(unit as u32).unwrap_or('\u{FFFD}'));
            Ok(())
        }
    }
}
