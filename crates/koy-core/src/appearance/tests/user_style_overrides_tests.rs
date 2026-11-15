//! Atlananlar:
//! - `Setting_is_off_by_default_and_survives_a_save_and_load_round_trip`: `AppSettings`/`SettingsStore` ister
//!   (Settings modülü taşınınca eklenecek).
//! - `Parsed_result_is_immutable_through_casts`: C# `FrozenDictionary` yazılamazlığını `IDictionary` dökümüyle sınıyordu;
//!   Rust'ta `&HashMap` zaten değiştirilemez, sınanacak çalışma zamanı davranışı yok.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use regex::Regex;

use super::resources_dir;
use crate::appearance::{
    AllowedTokens, UserStyleColor, UserStyleDiagnosticCode, UserStyleFile, UserStyleKind, UserStyleParseResult, UserStyleParser,
    UserStyleQuad, UserStyleToken, UserStyleValue,
};

fn parse(json: &str) -> UserStyleParseResult {
    UserStyleParser::parse(json)
}

/// `JsonSerializer.Serialize(string)`: tırnaklı, kaçışlı JSON metni.
fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn single<T>(items: &[T]) -> &T {
    assert_eq!(1, items.len(), "tek öğe bekleniyordu");
    &items[0]
}

fn pad_tokens() -> HashMap<String, UserStyleToken> {
    let mut tokens = HashMap::new();
    tokens.insert("Pad".to_string(), UserStyleToken::new("Pad", UserStyleKind::Thickness, 0.0, 32.0));
    tokens
}

// --- Geçerli değerler ---

#[test]
fn valid_brush_colors_in_both_forms_are_accepted() {
    let r = parse(r##"{ "KoyAccent": "#3B82F6", "KoyScrim": "#80112233" }"##);

    assert!(r.diagnostics.is_empty());
    assert_eq!(UserStyleValue::for_brush(UserStyleColor::new(255, 0x3B, 0x82, 0xF6)), r.valid_entries["KoyAccent"]);
    assert_eq!(UserStyleValue::for_brush(UserStyleColor::new(0x80, 0x11, 0x22, 0x33)), r.valid_entries["KoyScrim"]);
}

#[test]
fn hex_digits_are_case_insensitive() {
    let r = parse(r##"{ "KoyAccent": "#abcdef" }"##);
    assert_eq!(UserStyleColor::new(255, 0xAB, 0xCD, 0xEF), r.valid_entries["KoyAccent"].color);
}

#[test]
fn color_kind_token_yields_a_color_value() {
    let r = parse(r##"{ "KoyShadowColor": "#000000" }"##);
    assert_eq!(UserStyleKind::Color, r.valid_entries["KoyShadowColor"].kind);
}

#[test]
fn corner_radius_accepts_one_or_four_numbers() {
    let rows: [(&str, f64, f64, f64, f64); 4] = [
        (r#"{ "KoyRadiusRow": 12 }"#, 12.0, 12.0, 12.0, 12.0),
        (r#"{ "KoyRadiusRow": "12" }"#, 12.0, 12.0, 12.0, 12.0),
        (r#"{ "KoyRadiusRow": "8,8,0,0" }"#, 8.0, 8.0, 0.0, 0.0),
        (r#"{ "KoyRadiusRow": " 1.5 , 2 , 3 , 4 " }"#, 1.5, 2.0, 3.0, 4.0),
    ];
    for (json, a, b, c, d) in rows {
        let r = parse(json);

        assert!(r.diagnostics.is_empty(), "{json}");
        let v = &r.valid_entries["KoyRadiusRow"];
        assert_eq!(UserStyleKind::CornerRadius, v.kind, "{json}");
        assert_eq!(UserStyleQuad::new(a, b, c, d), v.quad, "{json}");
    }
}

#[test]
fn numbers_are_accepted_as_json_numbers_or_strings_within_bounds() {
    let r = parse(r#"{ "KoyFontBody": 14.5, "KoySpace2": "10", "KoyIconStroke": 0.5 }"#);

    assert!(r.diagnostics.is_empty());
    assert_eq!(14.5, r.valid_entries["KoyFontBody"].number);
    assert_eq!(10.0, r.valid_entries["KoySpace2"].number);
    assert_eq!(0.5, r.valid_entries["KoyIconStroke"].number);
}

#[test]
fn number_parsing_ignores_the_current_culture() {
    // Rust'ta kültüre bağlı sayı ayrıştırması yok; ondalık nokta her zaman değişmez kültürdeki gibi okunur.
    let r = parse(r#"{ "KoyFontBody": "13.5" }"#);
    assert_eq!(13.5, r.valid_entries["KoyFontBody"].number);
}

#[test]
fn font_family_accepts_plain_names_and_fallback_lists() {
    let r = parse(r#"{ "KoyFont": "Segoe UI Variable Text, Segoe UI, sans-serif", "KoyFontMono": "Cascadia Code" }"#);

    assert!(r.diagnostics.is_empty());
    assert_eq!("Segoe UI Variable Text, Segoe UI, sans-serif", r.valid_entries["KoyFont"].font_family);
    assert_eq!(UserStyleKind::FontFamily, r.valid_entries["KoyFontMono"].kind);
}

#[test]
fn non_latin_font_names_are_accepted() {
    let r = parse(r#"{ "KoyFont": "Yu Gothic UI" }"#);
    assert!(r.diagnostics.is_empty());
    let r = parse("{ \"KoyFont\": \"\u{00C7}izgi Yaz\u{0131}\" }");
    assert!(r.diagnostics.is_empty());
}

#[test]
fn comments_and_trailing_commas_are_tolerated() {
    let r = parse(
        "{\n  // vurgu\n  \"KoyAccent\": \"#112233\",\n}",
    );

    assert!(r.diagnostics.is_empty());
    assert_eq!(1, r.valid_entries.len());
}

#[test]
fn empty_object_is_valid_and_empty() {
    let r = parse("{}");
    assert!(r.valid_entries.is_empty());
    assert!(r.diagnostics.is_empty());
}

#[test]
fn valid_entries_survive_next_to_invalid_ones() {
    let r = parse(r##"{ "KoyAccent": "#112233", "Nope": "#000000", "KoyBgRow": "red" }"##);

    assert_eq!(1, r.valid_entries.len());
    assert_eq!(2, r.diagnostics.len());
    assert!(r.diagnostics.iter().any(|d| d.key == "Nope" && d.code == UserStyleDiagnosticCode::UnknownToken));
    assert!(r.diagnostics.iter().any(|d| d.key == "KoyBgRow" && d.code == UserStyleDiagnosticCode::BadColor));
}

// --- Thickness (şu an izinli listede Thickness anahtarı yok; dışarıdan verilen tabloyla denenir) ---

#[test]
fn thickness_accepts_one_or_four_non_negative_numbers() {
    let tokens = pad_tokens();

    let one = UserStyleParser::parse_with_tokens(Some(r#"{ "Pad": "6" }"#), &tokens);
    assert_eq!(UserStyleValue::for_thickness(UserStyleQuad::uniform(6.0)), one.valid_entries["Pad"]);

    let four = UserStyleParser::parse_with_tokens(Some(r#"{ "Pad": "1,2,3,4" }"#), &tokens);
    assert_eq!(UserStyleQuad::new(1.0, 2.0, 3.0, 4.0), four.valid_entries["Pad"].quad);
    assert_eq!(UserStyleKind::Thickness, four.valid_entries["Pad"].kind);
}

#[test]
fn thickness_rejects_bad_shapes_and_bounds() {
    let tokens = pad_tokens();
    for value in ["-1", "1,2", "1,2,3", "1,2,3,4,5", "1,,3,4", "1,2,3,x", "33", ""] {
        let json = format!(r#"{{ "Pad": "{value}" }}"#);

        let r = UserStyleParser::parse_with_tokens(Some(&json), &tokens);

        assert!(r.valid_entries.is_empty(), "{value}");
        assert_eq!(1, r.diagnostics.len(), "{value}");
    }
}

// --- Reddedilen girdiler ---

#[test]
fn unknown_tokens_are_rejected() {
    let keys = [
        "Window",
        "koyaccent", // büyük-küçük harf duyarlı
        "KoyAccent ",
        "System.Windows.Application",
        "{x:Static Application.Current}",
        "",
    ];
    for key in keys {
        let r = parse(&format!(r##"{{ {}: "#112233" }}"##, json_string(key)));

        assert!(r.valid_entries.is_empty(), "{key:?}");
        let d = single(&r.diagnostics);
        assert_eq!(UserStyleDiagnosticCode::UnknownToken, d.code, "{key:?}");
    }
}

#[test]
fn bad_colors_are_rejected() {
    let rows = [
        r#"{ "KoyAccent": "123456" }"#, // # yok
        r##"{ "KoyAccent": "#123" }"##, // kısa biçim yok
        r##"{ "KoyAccent": "#12345" }"##,
        r##"{ "KoyAccent": "#1234567" }"##,
        r##"{ "KoyAccent": "#GGGGGG" }"##,
        r#"{ "KoyAccent": "red" }"#,
        r##"{ "KoyAccent": "#1122 33" }"##,
    ];
    for json in rows {
        let r = parse(json);

        assert!(r.valid_entries.is_empty(), "{json}");
        assert_eq!(UserStyleDiagnosticCode::BadColor, single(&r.diagnostics).code, "{json}");
    }
}

#[test]
fn surrounding_whitespace_in_a_color_is_trimmed() {
    let r = parse(r##"{ "KoyAccent": "  #112233 " }"##);
    assert_eq!(1, r.valid_entries.len());
}

#[test]
fn bad_or_out_of_range_numbers_are_rejected() {
    let rows = [
        (r#"{ "KoyFontBody": "abc" }"#, UserStyleDiagnosticCode::BadNumber),
        (r#"{ "KoyFontBody": "NaN" }"#, UserStyleDiagnosticCode::BadNumber),
        (r#"{ "KoyFontBody": "Infinity" }"#, UserStyleDiagnosticCode::BadNumber),
        (r#"{ "KoyFontBody": "-Infinity" }"#, UserStyleDiagnosticCode::BadNumber),
        (r#"{ "KoyFontBody": "" }"#, UserStyleDiagnosticCode::BadNumber),
        (r#"{ "KoyFontBody": 7 }"#, UserStyleDiagnosticCode::OutOfRange),
        (r#"{ "KoyFontBody": 41 }"#, UserStyleDiagnosticCode::OutOfRange),
        (r#"{ "KoyFontBody": 1e308 }"#, UserStyleDiagnosticCode::OutOfRange),
        (r#"{ "KoySpace1": -1 }"#, UserStyleDiagnosticCode::OutOfRange),
        (r#"{ "KoyFontBody": "1e999" }"#, UserStyleDiagnosticCode::BadNumber),
    ];
    for (json, code) in rows {
        let r = parse(json);

        assert!(r.valid_entries.is_empty(), "{json}");
        assert_eq!(code, single(&r.diagnostics).code, "{json}");
    }
}

#[test]
fn bad_corner_radii_are_rejected() {
    for value in ["-1", "33", "1,2", "1,2,3", "1,2,3,4,5", "a", "1,2,3,-4", "NaN", "8;8;8;8"] {
        let r = parse(&format!(r#"{{ "KoyRadiusControl": "{value}" }}"#));

        assert!(r.valid_entries.is_empty(), "{value}");
        assert_eq!(1, r.diagnostics.len(), "{value}");
    }
}

#[test]
fn wrong_json_value_types_are_rejected() {
    let rows = [
        r#"{ "KoyAccent": 123 }"#, // renk sayı olamaz
        r#"{ "KoyAccent": true }"#,
        r#"{ "KoyAccent": null }"#,
        r##"{ "KoyAccent": ["#112233"] }"##,
        r##"{ "KoyAccent": { "Color": "#112233" } }"##,
        r#"{ "KoyFont": 12 }"#, // yazı tipi sayı olamaz
        r#"{ "KoyFontBody": true }"#,
        r#"{ "KoyFontBody": null }"#,
        r#"{ "KoyFontBody": [12] }"#,
        r#"{ "KoyRadiusRow": { "a": 1 } }"#,
    ];
    for json in rows {
        let r = parse(json);

        assert!(r.valid_entries.is_empty(), "{json}");
        assert_eq!(UserStyleDiagnosticCode::WrongValueType, single(&r.diagnostics).code, "{json}");
    }
}

#[test]
fn value_kind_is_fixed_by_the_token_not_by_the_file() {
    // Renk beklenen yere sayı, sayı beklenen yere renk, köşe beklenen yere yazı tipi adı.
    let r = parse(r##"{ "KoyAccent": "12", "KoyFontBody": "#112233", "KoyRadiusRow": "Arial" }"##);

    assert!(r.valid_entries.is_empty());
    assert_eq!(3, r.diagnostics.len());
}

#[test]
fn duplicate_keys_keep_the_first_value() {
    let r = parse(r##"{ "KoyAccent": "#111111", "KoyAccent": "#222222" }"##);

    assert_eq!(UserStyleColor::new(255, 0x11, 0x11, 0x11), r.valid_entries["KoyAccent"].color);
    assert_eq!(UserStyleDiagnosticCode::DuplicateToken, single(&r.diagnostics).code);
}

// --- Bozuk JSON ve dosya sınırları ---

#[test]
fn bad_json_yields_one_file_diagnostic_and_never_throws() {
    let rows = ["", "   ", "{", "{ \"KoyAccent\": ", "not json", "{ 'KoyAccent': '#112233' }", "\0\0\0"];
    for json in rows {
        let r = parse(json);

        assert!(r.valid_entries.is_empty(), "{json:?}");
        let d = single(&r.diagnostics);
        assert_eq!(UserStyleParser::FILE_KEY, d.key, "{json:?}");
        assert_eq!(UserStyleDiagnosticCode::InvalidJson, d.code, "{json:?}");
    }
}

#[test]
fn null_content_does_not_throw() {
    let r = UserStyleParser::parse_with_tokens(None, AllowedTokens::all());
    assert!(r.valid_entries.is_empty());
    assert_eq!(1, r.diagnostics.len());
}

#[test]
fn non_object_roots_are_rejected() {
    let rows = ["[]", "[{\"KoyAccent\":\"#112233\"}]", "\"KoyAccent\"", "42", "null", "true"];
    for json in rows {
        let r = parse(json);

        assert!(r.valid_entries.is_empty(), "{json}");
        assert_eq!(UserStyleDiagnosticCode::NotAnObject, single(&r.diagnostics).code, "{json}");
    }
}

#[test]
fn deeply_nested_json_is_rejected_without_throwing() {
    let json = format!("{}{}", "[".repeat(5000), "]".repeat(5000));
    let r = parse(&json);

    assert!(r.valid_entries.is_empty());
    assert_eq!(1, r.diagnostics.len());
}

#[test]
fn oversize_input_is_rejected_before_parsing() {
    let json = format!(
        "{{ \"KoyAccent\": \"#112233\", \"pad\": \"{}\" }}",
        "a".repeat(UserStyleParser::MAX_BYTES as usize)
    );
    let r = parse(&json);

    assert!(r.valid_entries.is_empty());
    assert_eq!(UserStyleDiagnosticCode::FileTooLarge, single(&r.diagnostics).code);
}

#[test]
fn multibyte_input_is_measured_in_bytes_not_characters() {
    // 30.000 karakter ama ~90.000 bayt.
    let json = format!("{{ \"x\": \"{}\" }}", "\u{20AC}".repeat(30_000));
    assert!(json.encode_utf16().count() < UserStyleParser::MAX_BYTES as usize);
    assert!(json.len() > UserStyleParser::MAX_BYTES as usize);

    assert_eq!(UserStyleDiagnosticCode::FileTooLarge, single(&parse(&json).diagnostics).code);
}

#[test]
fn entry_count_is_capped() {
    let mut sb = String::from("{");
    for i in 0..UserStyleParser::MAX_ENTRIES + 50 {
        sb.push_str(&format!("\"k{i}\":\"#000000\","));
    }
    sb.push_str("\"KoyAccent\":\"#112233\"}");

    let r = parse(&sb);

    // Üst sınır aşıldığı için sondaki geçerli girdi okunmaz.
    assert!(r.valid_entries.is_empty());
    assert!(r.diagnostics.iter().any(|d| d.code == UserStyleDiagnosticCode::TooManyEntries));
}

#[test]
fn diagnostics_list_is_capped_and_the_rest_is_counted() {
    let mut sb = String::from("{");
    for i in 0..300 {
        sb.push_str(&format!("\"bad{i}\":\"x\","));
    }
    sb.pop();
    sb.push('}');

    let r = parse(&sb);

    assert_eq!(UserStyleParser::MAX_DIAGNOSTICS as usize, r.diagnostics.len());
    assert_eq!(300 - UserStyleParser::MAX_DIAGNOSTICS, r.omitted_diagnostics);
}

#[test]
fn value_longer_than_the_limit_is_rejected() {
    let r = parse(&format!(r#"{{ "KoyFont": "{}" }}"#, "A".repeat(UserStyleParser::MAX_VALUE_LENGTH as usize + 1)));

    assert!(r.valid_entries.is_empty());
    assert_eq!(UserStyleDiagnosticCode::ValueTooLong, single(&r.diagnostics).code);
}

#[test]
fn echoed_key_is_truncated_and_control_characters_are_removed() {
    let long_key = "k".repeat(500);
    let r = parse(&format!(r#"{{ "{long_key}": "x", "a\nb\u001b[31mc": "x" }}"#));

    for d in &r.diagnostics {
        assert!(d.key.encode_utf16().count() <= 64, "{}", d.key);
        assert_eq!(0, d.key.chars().filter(|c| c.is_control()).count(), "{}", d.key);
    }
    assert_eq!(2, r.diagnostics.len());
}

// --- Enjeksiyon benzeri girdiler ---

#[test]
fn xaml_markup_paths_and_control_characters_are_never_accepted_as_font_names() {
    let values = [
        "{x:Type Window}",
        "{StaticResource KoyFont}",
        "<Button xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\"/>",
        "<ObjectDataProvider/>",
        "Arial\"/><Button",
        "pack://application:,,,/Koy;component/Resources/#Font",
        "C:\\Windows\\Fonts\\arial.ttf",
        "\\\\server\\share\\font.ttf",
        "./fonts/#My Font",
        "file:///etc/passwd",
        "Arial;calc.exe",
        "Arial\u{0000}Bold",
        "Arial\nBold",
        "Arial\tBold",
        "Arial,",
        ",Arial",
        "Arial,,Segoe UI",
        ",",
        "   ",
    ];
    for value in values {
        let json = format!(r#"{{ "KoyFont": {} }}"#, json_string(value));
        let r = parse(&json);

        assert!(r.valid_entries.is_empty(), "{value:?}");
        assert_eq!(1, r.diagnostics.len(), "{value:?}");
    }
}

#[test]
fn xaml_markup_is_never_accepted_in_non_font_values() {
    for key in ["KoyAccent", "KoyRadiusRow", "KoyFontBody"] {
        for value in [
            "{x:Static Application.Current}",
            "<SolidColorBrush Color=\"Red\"/>",
            "#112233<Window/>",
            "{Binding Path=Foo}",
        ] {
            let json = format!(r#"{{ "{key}": {} }}"#, json_string(value));
            let r = parse(&json);

            assert!(r.valid_entries.is_empty(), "{key} {value:?}");
            assert_eq!(1, r.diagnostics.len(), "{key} {value:?}");
        }
    }
}

#[test]
fn type_names_in_keys_do_not_instantiate_anything_and_are_reported_as_unknown() {
    let r = parse(r#"{ "System.Diagnostics.Process": "calc", "$type": "System.Object", "__proto__": "x" }"#);

    assert!(r.valid_entries.is_empty());
    assert_eq!(3, r.diagnostics.len());
    assert!(r.diagnostics.iter().all(|d| d.code == UserStyleDiagnosticCode::UnknownToken));
}

// --- Allow-list ile XAML eşitliği ---

#[test]
fn allow_list_matches_the_keys_defined_in_the_theme_and_token_dictionaries() {
    let mut expected: HashMap<String, UserStyleKind> = HashMap::new();
    let re = Regex::new(r#"<(?P<tag>SolidColorBrush|Color|sys:Double|CornerRadius|FontFamily|Thickness)\s+x:Key="(?P<key>\w+)""#).unwrap();
    for file in ["Theme.Dark.xaml", "Theme.Light.xaml", "Tokens.xaml"] {
        let text = fs::read_to_string(resources_dir().join(file)).expect("XAML dosyası okunamadı");
        for m in re.captures_iter(&text) {
            let kind = match &m["tag"] {
                "SolidColorBrush" => UserStyleKind::Brush,
                "Color" => UserStyleKind::Color,
                "sys:Double" => UserStyleKind::Number,
                "CornerRadius" => UserStyleKind::CornerRadius,
                "FontFamily" => UserStyleKind::FontFamily,
                _ => UserStyleKind::Thickness,
            };
            expected.insert(m["key"].to_string(), kind);
        }
    }

    assert!(!expected.is_empty());
    let actual: HashMap<String, UserStyleKind> = AllowedTokens::all().iter().map(|(k, v)| (k.clone(), v.kind)).collect();

    let mut expected_sorted: Vec<(&String, &UserStyleKind)> = expected.iter().collect();
    let mut actual_sorted: Vec<(&String, &UserStyleKind)> = actual.iter().collect();
    expected_sorted.sort_by(|a, b| a.0.cmp(b.0));
    actual_sorted.sort_by(|a, b| a.0.cmp(b.0));
    assert_eq!(expected_sorted, actual_sorted);
}

#[test]
fn every_allowed_token_has_a_sane_definition() {
    for (key, token) in AllowedTokens::all() {
        assert_eq!(key, &token.key);
        assert!(key.starts_with("Koy"), "{key}");
        if matches!(token.kind, UserStyleKind::Number | UserStyleKind::CornerRadius | UserStyleKind::Thickness) {
            assert!(token.min >= 0.0, "{key}");
            assert!(token.max > token.min, "{key}");
            assert!(token.max <= 100.0, "{key}");
        }
    }
}

#[test]
fn default_theme_values_fall_inside_their_own_bounds() {
    // Varsayılan değer izinli aralıkta olmalı; yoksa "varsayılanı yaz" bile reddedilirdi.
    let text = fs::read_to_string(resources_dir().join("Tokens.xaml")).expect("Tokens.xaml okunamadı");
    let re = Regex::new(r#"<sys:Double x:Key="(?P<key>\w+)">(?P<v>[\d.]+)</sys:Double>"#).unwrap();
    for m in re.captures_iter(&text) {
        let token = &AllowedTokens::all()[&m["key"]];
        let value: f64 = m["v"].parse().unwrap();
        assert!(value >= token.min && value <= token.max, "{}: {value}", &m["key"]);
    }
}

#[test]
fn sample_file_from_the_docs_parses_without_diagnostics() {
    let r = parse(
        r##"{
  "KoyAccent": "#8B5CF6",
  "KoyAccentHover": "#A78BFA",
  "KoyBgWindow": "#101014",
  "KoyScrim": "#B3000000",
  "KoyRadiusRow": "10",
  "KoyRadiusWindow": "16,16,16,16",
  "KoyFontBody": 14,
  "KoyFont": "Segoe UI Variable Text, Segoe UI, sans-serif"
}"##,
    );

    assert!(r.diagnostics.is_empty());
    assert_eq!(8, r.valid_entries.len());
}

// --- Dosya okuma ---

/// Test başına benzersiz geçici dosya yolu (GUID yerine süreç kimliği + sayaç).
fn temp_path(prefix: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("{prefix}{}-{n}", std::process::id()))
}

fn temp_file() -> PathBuf {
    let mut p = temp_path("koy-user-theme-");
    p.set_extension("json");
    p
}

#[test]
fn missing_file_is_an_empty_result_without_diagnostics() {
    let r = UserStyleFile::load(&temp_path("koy-yok-"));

    assert!(r.valid_entries.is_empty());
    assert!(r.diagnostics.is_empty());
    assert!(!UserStyleFile::get_stamp(&temp_path("koy-yok-")).exists);
}

#[test]
fn file_with_bom_is_read_and_stamp_changes_when_content_changes() {
    let path = temp_file();

    fs::write(&path, "\u{FEFF}{ \"KoyAccent\": \"#112233\" }").unwrap();
    let first = UserStyleFile::get_stamp(&path);

    let r = UserStyleFile::load(&path);
    assert!(r.diagnostics.is_empty());
    assert_eq!(1, r.valid_entries.len());
    assert!(first.exists);

    fs::write(&path, "{ \"KoyAccent\": \"#112233\", \"KoyBgRow\": \"#000000\" }").unwrap();
    let new_time = std::time::SystemTime::from(chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
        first.last_write_utc + chrono::TimeDelta::seconds(5),
        chrono::Utc,
    ));
    fs::File::options().write(true).open(&path).unwrap().set_modified(new_time).unwrap();
    assert_ne!(first, UserStyleFile::get_stamp(&path));

    fs::remove_file(&path).unwrap();
}

#[test]
fn oversize_file_is_rejected_by_length_without_parsing() {
    let path = temp_file();
    fs::write(&path, " ".repeat(UserStyleParser::MAX_BYTES as usize + 10)).unwrap();

    let r = UserStyleFile::load(&path);

    assert!(r.valid_entries.is_empty());
    assert_eq!(UserStyleDiagnosticCode::FileTooLarge, single(&r.diagnostics).code);
    fs::remove_file(&path).unwrap();
}

#[test]
fn directory_in_place_of_the_file_does_not_throw() {
    let dir = temp_path("koy-dizin-");
    fs::create_dir_all(&dir).unwrap();

    let r = UserStyleFile::load(&dir);
    assert!(r.valid_entries.is_empty());
    assert!(r.diagnostics.is_empty()); // FileInfo.Exists dizin için false

    fs::remove_dir(&dir).unwrap();
}

#[test]
fn invalid_utf8_bytes_do_not_throw() {
    let path = temp_file();
    fs::write(&path, [0x7B, 0xFF, 0xFE, 0xFD, 0x7D]).unwrap();

    let r = UserStyleFile::load(&path);

    assert!(r.valid_entries.is_empty());
    assert_eq!(1, r.diagnostics.len());
    fs::remove_file(&path).unwrap();
}

// --- Mağaza tema paketleri ---

#[test]
fn store_theme_packs_parse_cleanly_and_paint_window_notch_and_dock() {
    for id in ["koyhq.nord-theme", "koyhq.solarized-theme", "koyhq.high-contrast-theme"] {
        let path = resources_dir().join("../../../store/src").join(id).join("theme.json");
        let path = fs::canonicalize(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let result = UserStyleFile::load(&path);

        assert!(result.diagnostics.is_empty(), "{id}: {:?}", result.diagnostics);
        assert!(result.valid_entries.keys().any(|k| k.starts_with("KoyNotch")), "{id}");
        // Çentik zemini, kartları, yazısı ve dock zemini her pakette verilir; yoksa o yüzey temadan kopuk kalır.
        for key in [
            "KoyNotchSurface", "KoyNotchCard", "KoyNotchChip", "KoyNotchText", "KoyNotchTextSecondary", "KoyNotchTextMuted",
            "KoyNotchHover", "KoyNotchDivider", "KoyNotchMusicCard", "KoyNotchAccentControl", "KoyNotchAccentFocus",
            "KoyDockSurface", "KoyDockCapsuleSurface", "KoyDockBorder", "KoyDockIndicator", "KoyDockText", "KoyDockHover",
        ] {
            assert!(result.valid_entries.contains_key(key), "{id}: {key}");
        }
        assert!((result.valid_entries.len() as i32) < UserStyleParser::MAX_ENTRIES, "{id}");
    }
}
