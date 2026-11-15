use super::support::{identity_t, q, FakeShell};
use crate::appearance::Rgb;
use crate::clock::Culture;
use crate::search::*;
use crate::text;
use std::sync::Arc;

fn tr() -> Culture {
    Culture::tr_tr()
}

fn en() -> Culture {
    Culture::en_us()
}

fn de() -> Culture {
    Culture::de_de()
}

fn convert(text: &str, culture: Option<Culture>) -> f64 {
    UnitConverter::try_convert(Some(text), &culture.unwrap_or_else(en)).unwrap_or_else(|| panic!("'{text}' çevrilemedi")).value
}

/// xUnit `Assert.Equal(expected, actual, precision)`: ikisi de `precision` ondalığa yuvarlanıp karşılaştırılır.
fn assert_equal(expected: f64, actual: f64, precision: i32, what: &str) {
    assert_eq!(text::round_digits(expected, precision), text::round_digits(actual, precision), "{what}: beklenen {expected}, gelen {actual}");
}

fn check_all(cases: &[(&str, f64)], precision: i32) {
    for (text, expected) in cases {
        assert_equal(*expected, convert(text, None), precision, text);
    }
}

// ---- Uzunluk, kütle, hız ----

#[test]
fn length_conversions() {
    check_all(
        &[
            ("10 km to mi", 6.21371192),
            ("1 mi to km", 1.609344),
            ("1 m to cm", 100.0),
            ("250 cm to m", 2.5),
            ("1 ft to in", 12.0),
            ("1 in to cm", 2.54),
            ("1 mi to ft", 5280.0),
            ("3 ft to m", 0.9144),
            ("1 km to m", 1000.0),
        ],
        6,
    );
}

#[test]
fn mass_conversions() {
    check_all(
        &[
            ("1 kg to g", 1000.0),
            ("1 kg to lb", 2.20462262),
            ("1 lb to oz", 16.0),
            ("16 oz to lb", 1.0),
            ("500 g to kg", 0.5),
            ("5 kg lb", 11.0231131),
        ],
        6,
    );
}

#[test]
fn speed_conversions() {
    check_all(
        &[
            ("60 mph to km/h", 96.56064),
            ("100 km/h to mph", 62.1371192),
            ("10 m/s to km/h", 36.0),
            ("36 km/h to m/s", 10.0),
            ("1 mph to m/s", 0.44704),
        ],
        5,
    );
}

// ---- Sıcaklık: ofsetli ----

#[test]
fn temperature_uses_offsets_not_only_factors() {
    check_all(
        &[
            ("100 c in f", 212.0),
            ("0 c in f", 32.0),
            ("-40 c to f", -40.0),
            ("32 f to c", 0.0),
            ("212 f to c", 100.0),
            ("-40 f to c", -40.0),
            ("0 c to k", 273.15),
            ("273.15 k to c", 0.0),
            ("0 k to c", -273.15),
            ("300 k to f", 80.33),
            ("98.6 f to k", 310.15),
        ],
        6,
    );
}

#[test]
fn temperature_below_absolute_zero_is_rejected() {
    for text in ["-300 c to f", "-1 k to c", "-500 f to c"] {
        assert!(UnitConverter::try_convert(Some(text), &en()).is_none(), "{text}");
    }
}

#[test]
fn temperature_degree_sign_and_long_names_work() {
    assert_equal(212.0, convert("100 °C to °F", None), 6, "1");
    assert_equal(212.0, convert("100 celsius to fahrenheit", None), 6, "2");
    assert_equal(212.0, convert("100°c in °f", None), 6, "3");
    assert_equal(373.15, convert("100 Celsius in Kelvin", None), 6, "4");
}

// ---- Veri boyutu ----

#[test]
fn data_is_si_by_default_and_binary_with_ib_units() {
    check_all(
        &[
            ("1 gb in mb", 1000.0),
            ("1 kb to b", 1000.0),
            ("1 tb to gb", 1000.0),
            ("2500 mb to gb", 2.5),
            ("1 mb to kb", 1000.0),
            ("1 gib to mib", 1024.0),
            ("1 kib to b", 1024.0),
            ("1 tib to gib", 1024.0),
        ],
        9,
    );
}

#[test]
fn data_si_and_binary_differ() {
    assert_equal(1000.0, convert("1 gb to mb", None), 9, "1");
    assert_equal(1024.0, convert("1 gib to mib", None), 9, "2");
    assert_equal(1073.741824, convert("1 gib to mb", None), 6, "3");
}

// ---- Söz dizimi, takma adlar, büyük/küçük harf ----

#[test]
fn syntax_variants_all_give_the_same_answer() {
    for text in [
        "10 km to mi",
        "10 km in mi",
        "10 km into mi",
        "10 km as mi",
        "10 km mi",
        "10km mi",
        "10km->mi",
        "10 km -> mi",
        "10 km => mi",
        "10 km = mi",
        "10 km > mi",
        "10 km → mi",
        "  10   km    to   mi  ",
        "10 KM TO MI",
        "10 Km To Mi",
        "10 kilometers to miles",
        "10 kilometres in mile",
    ] {
        assert_equal(6.21371192, convert(text, None), 6, text);
    }
}

#[test]
fn the_word_in_is_both_a_unit_and_a_separator() {
    for text in ["1 in in cm", "1 inch to cm", "1 inches in cm", "1 in cm", "1IN TO CM"] {
        assert_equal(2.54, convert(text, None), 9, text);
    }
}

#[test]
fn aliases() {
    check_all(
        &[
            ("6 feet to cm", 182.88),
            ("1 foot in inches", 12.0),
            ("2 pounds to kg", 0.90718474),
            ("2 lbs to oz", 32.0),
            ("1 ounce to g", 28.349523125),
            ("3 meters to ft", 9.84251969),
            ("1 kmh to m/s", 0.27777778),
            ("1 kph to mph", 0.62137119),
            ("1 byte to b", 1.0),
            ("1 megabyte to kilobytes", 1000.0),
        ],
        6,
    );
}

#[test]
fn turkish_dotless_i_folds_in_unit_names() {
    // "İNÇ" / "inç" -> "inc"; "KİLO" -> "kilo"
    assert_equal(2.54, convert("1 İNÇ to cm", Some(tr())), 9, "inç");
    assert_equal(1000.0, convert("1 KİLO to g", Some(tr())), 9, "kilo");
}

#[test]
fn same_unit_returns_the_input() {
    assert_equal(7.0, convert("7 km to km", None), 12, "km");
    assert_equal(25.0, convert("25 c to c", None), 12, "c");
}

// ---- Sayı ayrıştırma ve kültür ----

#[test]
fn decimal_comma_and_point_both_work_in_any_culture() {
    for (text, expected) in [("10,5 km to m", 10500.0), ("10.5 km to m", 10500.0)] {
        assert_equal(expected, convert(text, Some(tr())), 6, text);
        assert_equal(expected, convert(text, Some(en())), 6, text);
        assert_equal(expected, convert(text, Some(de())), 6, text);
        assert_equal(expected, convert(text, Some(Culture::invariant())), 6, text);
    }
}

#[test]
fn a_single_separator_followed_by_three_digits_is_thousands_only_in_its_own_culture() {
    let cases = [
        ("1.500 kg to g", "tr-TR", 1_500_000.0),
        ("1,500 kg to g", "en-US", 1_500_000.0),
        ("1,500 kg to g", "tr-TR", 1_500.0),
        ("1.500 kg to g", "en-US", 1_500.0),
    ];
    for (text, culture, expected) in cases {
        assert_equal(expected, convert(text, Culture::from_name(culture)), 6, text);
    }
}

#[test]
fn grouped_numbers() {
    let cases = [
        ("1.000.000 m to km", 1000.0),
        ("1,000,000 m to km", 1000.0),
        ("1.234,5 m to cm", 123450.0),
        ("1,234.5 m to cm", 123450.0),
    ];
    for (text, expected) in cases {
        assert_equal(expected, convert(text, Some(tr())), 6, text);
        assert_equal(expected, convert(text, Some(en())), 6, text);
    }
}

#[test]
fn malformed_numbers_are_rejected() {
    for text in ["1.2.3 km to m", "1,23,456 km to m", "1.234,5,6 km to m", "1,5.2,3 km to m", "1..5 km to m"] {
        assert!(UnitConverter::try_convert(Some(text), &en()).is_none(), "{text}");
    }
}

#[test]
fn number_edge_forms() {
    let cases = [
        (".5 km to m", 500.0),
        (",5 km to m", 500.0),
        ("0,5 km to m", 500.0),
        ("0.500 km to m", 500.0),
        ("+5 km to m", 5000.0),
        ("-5 km to m", -5000.0),
        ("−5 km to m", -5000.0),
        ("5. km to m", 5000.0),
    ];
    for (text, expected) in cases {
        assert_equal(expected, convert(text, Some(tr())), 6, text);
        assert_equal(expected, convert(text, Some(en())), 6, text);
    }
}

#[test]
fn try_parse_number_basics() {
    let cases = [("10", "en-US", 10.0), ("1234567", "en-US", 1234567.0), ("-0.25", "tr-TR", -0.25), ("-0,25", "en-US", -0.25)];
    for (text, culture, expected) in cases {
        let v = UnitConverter::try_parse_number(Some(text), &Culture::from_name(culture).unwrap()).unwrap_or_else(|| panic!("{text}"));
        assert_equal(expected, v, 12, text);
    }
}

#[test]
fn try_parse_number_rejects_garbage() {
    for text in [None, Some(""), Some("   "), Some("abc"), Some("-"), Some("."), Some(","), Some("1e5"), Some("1 000")] {
        assert!(UnitConverter::try_parse_number(text, &en()).is_none(), "{text:?}");
    }
}

// ---- Reddedilenler ----

#[test]
fn cross_category_conversions_are_rejected() {
    for text in ["10 km to kg", "10 kg to km", "10 c to km", "10 gb to mph", "10 mph to f", "10 m to b", "10 oz to mb"] {
        assert!(UnitConverter::try_convert(Some(text), &en()).is_none(), "{text}");
    }
}

#[test]
fn garbage_is_rejected_without_throwing() {
    let cases = [
        None,
        Some(""),
        Some("   "),
        Some("chrome"),
        Some("km to mi"),
        Some("10"),
        Some("10 km"),
        Some("10 km to"),
        Some("10 to mi"),
        Some("10 km to mi extra"),
        Some("10 km foo mi"),
        Some("10 km to foo"),
        Some("10 foo to mi"),
        Some("10 xx yy"),
        Some("12*8"),
        Some("10 km to mi to ft"),
        Some("to to to"),
        Some("->"),
        Some("1e999 km to m"),
        Some("#fff"),
        Some("rgb(1,2,3)"),
        Some("10 km/ to mi"),
        Some("10 k m to mi"),
    ];
    for text in cases {
        assert!(UnitConverter::try_convert(text, &en()).is_none(), "{text:?}");
    }
}

#[test]
fn very_long_input_is_rejected() {
    let text = format!("{} km to m", "9".repeat(200));
    assert!(UnitConverter::try_convert(Some(&text), &en()).is_none());
}

#[test]
fn huge_but_parseable_numbers_never_produce_non_finite_results() {
    let big = "9".repeat(30);
    if let Some(c) = UnitConverter::try_convert(Some(&format!("{big} tb to b")), &en()) {
        assert!(c.value.is_finite());
    }
}

#[test]
fn never_throws_on_random_noise() {
    // Sabit tohumlu basit üreteç (C# testi `new Random(1234)` kullanır).
    let mut state: u64 = 1234;
    let mut next = |bound: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % bound as u64) as usize
    };
    let alphabet: Vec<char> = "0123456789.,+-−eE kmgbcfinhsto/°→=> ()#%\t".chars().collect();
    for _ in 0..5000 {
        let len = next(24);
        let text: String = (0..len).map(|_| alphabet[next(alphabet.len())]).collect();
        let _ = UnitConverter::try_convert(Some(&text), &en());
        let _ = UnitConverter::try_convert(Some(&text), &tr());
        let _ = ColorParser::try_parse(Some(&text));
    }
}

// ---- Gidiş-dönüş ----

#[test]
fn round_trip_returns_the_original() {
    let pairs = [
        ("km", "mi"),
        ("m", "ft"),
        ("cm", "in"),
        ("kg", "lb"),
        ("g", "oz"),
        ("c", "f"),
        ("c", "k"),
        ("f", "k"),
        ("kb", "mb"),
        ("gib", "gb"),
        ("km/h", "mph"),
        ("m/s", "mph"),
    ];
    for (a, b) in pairs {
        let ua = UnitConverter::try_find_unit(Some(a)).unwrap_or_else(|| panic!("{a}"));
        let ub = UnitConverter::try_find_unit(Some(b)).unwrap_or_else(|| panic!("{b}"));
        for value in [0.0, 1.0, 37.25, 1234.5678, 100.0] {
            let there = UnitConverter::try_convert_value(value, &ua, &ub).unwrap();
            let back = UnitConverter::try_convert_value(there, &ub, &ua).unwrap();
            assert_equal(value, back, 6, &format!("{a}/{b}"));
        }
    }
}

#[test]
fn try_find_unit_is_case_insensitive_and_rejects_unknowns() {
    assert_eq!("km/h", UnitConverter::try_find_unit(Some("KM/H")).unwrap().symbol);
    assert_eq!("mi", UnitConverter::try_find_unit(Some(" Miles ")).unwrap().symbol);
    assert!(UnitConverter::try_find_unit(Some("parsec")).is_none());
    assert!(UnitConverter::try_find_unit(None).is_none());
    assert!(UnitConverter::try_find_unit(Some("")).is_none());
}

#[test]
fn categories_are_assigned_correctly() {
    let category = |name: &str| UnitConverter::try_find_unit(Some(name)).unwrap().category;
    assert_eq!(UnitCategory::Length, category("km"));
    assert_eq!(UnitCategory::Mass, category("lb"));
    assert_eq!(UnitCategory::Temperature, category("k"));
    assert_eq!(UnitCategory::Data, category("gb"));
    assert_eq!(UnitCategory::Speed, category("mph"));
}

// ---- Biçimleme ----

#[test]
fn format_number_rounds_sensibly() {
    let cases = [
        (6.21371192, "en-US", false, "6.21371"),
        (6.21371192, "tr-TR", false, "6,21371"),
        (1234.5678, "en-US", false, "1234.57"),
        (1234.5678, "en-US", true, "1,234.57"),
        (1234.5678, "tr-TR", true, "1.234,57"),
        (1234567.891, "en-US", false, "1234568"),
        (1500000.0, "en-US", true, "1,500,000"),
        (100.0, "en-US", false, "100"),
        (2.5, "en-US", false, "2.5"),
        (0.000123456789, "en-US", false, "0.000123457"),
        (-40.0, "en-US", false, "-40"),
        (-12.34567, "tr-TR", false, "-12,3457"),
        (0.0, "en-US", false, "0"),
        (0.1 + 0.2, "en-US", false, "0.3"),
    ];
    for (value, culture, grouping, expected) in cases {
        assert_eq!(expected, UnitConverter::format_number(value, &Culture::from_name(culture).unwrap(), grouping), "{value} {culture}");
    }
}

#[test]
fn format_number_never_uses_exponents_in_normal_range_and_survives_extremes() {
    assert!(!UnitConverter::format_number(1e-12, &en(), false).contains('E'));
    assert_eq!("", UnitConverter::format_number(f64::NAN, &en(), false));
    assert_eq!("", UnitConverter::format_number(f64::INFINITY, &en(), false));
    assert_ne!("", UnitConverter::format_number(1e300, &en(), false));
    assert_ne!("", UnitConverter::format_number(1e-300, &en(), false));
    assert_ne!("0", UnitConverter::format_number(1e-300, &en(), false));
}

// ---- UnitConverterSource ----

#[test]
fn unit_source_returns_one_copyable_row() {
    let shell = FakeShell::new();
    let source = UnitConverterSource::new(shell.clone(), identity_t(), en());
    let mut results = source.search(&q("10 km to mi"));
    assert_eq!(1, results.len());
    let result = results.remove(0);

    assert_eq!("6.21371 mi", result.title);
    assert!(result.subtitle.as_deref().unwrap().starts_with("10 km"));
    assert!(result.restores_focus);
    assert_eq!(SearchRelevance::HIGHEST, result.category_relevance);
    assert_eq!("units", source.id());

    (result.execute)();
    assert_eq!(vec!["copy 6.21371"], shell.log());
}

#[test]
fn unit_source_copies_the_bare_number_in_the_users_culture() {
    let shell = FakeShell::new();
    let result = UnitConverterSource::new(shell.clone(), identity_t(), tr()).search(&q("10,5 km to mi")).remove(0);

    assert_eq!("6,5244 mi", result.title);
    (result.execute)();
    assert_eq!(vec!["copy 6,5244"], shell.log());
}

#[test]
fn unit_source_shows_grouping_but_copies_without_it() {
    let shell = FakeShell::new();
    let result = UnitConverterSource::new(shell.clone(), identity_t(), en()).search(&q("1 tb in mb")).remove(0);

    assert_eq!("1,000,000 MB", result.title);
    (result.execute)();
    assert_eq!(vec!["copy 1000000"], shell.log());
}

#[test]
fn unit_source_temperature_title() {
    let result = UnitConverterSource::new(FakeShell::new(), identity_t(), en()).search(&q("100 c in f")).remove(0);
    assert_eq!("212 °F", result.title);
}

#[test]
fn unit_source_ignores_non_conversions() {
    for text in ["", "chrome", "10 km to kg", "12*8", "#ff8800"] {
        assert!(UnitConverterSource::new(FakeShell::new(), identity_t(), en()).search(&q(text)).is_empty(), "{text}");
    }
}

// ---- Renk ----

fn parse(text: &str) -> ParsedColor {
    ColorParser::try_parse(Some(text)).unwrap_or_else(|| panic!("{text}"))
}

#[test]
fn color_input_forms() {
    let cases = [
        ("#FF8800", 255, 136, 0),
        ("#ff8800", 255, 136, 0),
        ("#f80", 255, 136, 0),
        ("#F80", 255, 136, 0),
        ("#000", 0, 0, 0),
        ("#FFFFFF", 255, 255, 255),
        ("  #3B82F6  ", 59, 130, 246),
        ("rgb(255, 136, 0)", 255, 136, 0),
        ("rgb(255,136,0)", 255, 136, 0),
        ("RGB( 255 , 136 , 0 )", 255, 136, 0),
        ("rgb(255 136 0)", 255, 136, 0),
        ("rgb(0,0,0)", 0, 0, 0),
        ("hsl(0, 100%, 50%)", 255, 0, 0),
        ("hsl(120,100%,50%)", 0, 255, 0),
        ("HSL(240, 100%, 50%)", 0, 0, 255),
        ("hsl(0, 0%, 100%)", 255, 255, 255),
        ("hsl(0, 0%, 0%)", 0, 0, 0),
        ("hsl(360, 100%, 50%)", 255, 0, 0),
        ("hsl(-120, 100%, 50%)", 0, 0, 255),
        ("hsl(480, 100%, 50%)", 0, 255, 0),
        ("hsl(0 100 50)", 255, 0, 0),
        ("hsl(0deg, 100%, 50%)", 255, 0, 0),
        ("hsl(0°, 100%, 50%)", 255, 0, 0),
    ];
    for (text, r, g, b) in cases {
        let color = parse(text);
        assert_eq!(Rgb::new(r, g, b), color.color, "{text}");
        assert!(!color.has_alpha(), "{text}");
    }
}

#[test]
fn color_garbage_is_rejected() {
    let cases = [
        None,
        Some(""),
        Some("   "),
        Some("#"),
        Some("#ff"),
        Some("#ffff"),
        Some("#fffff"),
        Some("#fffffff"),
        Some("#fffffffff"),
        Some("#gggggg"),
        Some("#ff88zz"),
        Some("ff8800"),
        Some("red"),
        Some("rgb(256,0,0)"),
        Some("rgb(0,300,0)"),
        Some("rgb(-1,0,0)"),
        Some("rgb(1,2)"),
        Some("rgb(1,2,3,4)"),
        Some("rgb(1,2,3"),
        Some("rgb(a,b,c)"),
        Some("rgb(1.5,2,3)"),
        Some("hsl(0, 101%, 50%)"),
        Some("hsl(0, 100%, 150%)"),
        Some("hsl(0, -5%, 50%)"),
        Some("hsl(a, b, c)"),
        Some("hsl(0, 100%)"),
        Some("hsl(0, 100%, 50%"),
        Some("12*8"),
        Some("10 km to mi"),
    ];
    for text in cases {
        assert!(ColorParser::try_parse(text).is_none(), "{text:?}");
    }
}

#[test]
fn color_long_input_is_rejected() {
    assert!(ColorParser::try_parse(Some(&format!("#{}", "f".repeat(100)))).is_none());
}

#[test]
fn eight_digit_hex_is_alpha_first_like_wpf() {
    let color = parse("#80FF8800");
    assert_eq!(Rgb::new(255, 136, 0), color.color);
    assert_eq!(0x80, color.alpha);
    assert!(color.has_alpha());
    assert_eq!("#80FF8800", color.to_hex());
    assert_eq!("rgba(255, 136, 0, 0.5)", color.to_rgb_text());
    assert_eq!("hsla(32, 100%, 50%, 0.5)", color.to_hsl_text());
}

#[test]
fn opaque_eight_digit_hex_drops_the_alpha_in_output() {
    let color = parse("#FFFF8800");
    assert!(!color.has_alpha());
    assert_eq!("#FF8800", color.to_hex());
    assert_eq!("rgb(255, 136, 0)", color.to_rgb_text());
}

#[test]
fn fully_transparent_alpha() {
    assert_eq!("rgba(17, 34, 51, 0)", parse("#00112233").to_rgb_text());
}

#[test]
fn color_outputs() {
    let cases = [
        ("#FF8800", "#FF8800", "rgb(255, 136, 0)", "hsl(32, 100%, 50%)"),
        ("#f80", "#FF8800", "rgb(255, 136, 0)", "hsl(32, 100%, 50%)"),
        ("#000", "#000000", "rgb(0, 0, 0)", "hsl(0, 0%, 0%)"),
        ("#FFF", "#FFFFFF", "rgb(255, 255, 255)", "hsl(0, 0%, 100%)"),
        ("rgb(255, 0, 0)", "#FF0000", "rgb(255, 0, 0)", "hsl(0, 100%, 50%)"),
        ("rgb(59, 130, 246)", "#3B82F6", "rgb(59, 130, 246)", "hsl(217, 91%, 60%)"),
        ("hsl(120, 100%, 50%)", "#00FF00", "rgb(0, 255, 0)", "hsl(120, 100%, 50%)"),
        ("hsl(240,100%,25%)", "#000080", "rgb(0, 0, 128)", "hsl(240, 100%, 25%)"),
        ("hsl(360, 100%, 50%)", "#FF0000", "rgb(255, 0, 0)", "hsl(0, 100%, 50%)"),
        ("hsl(-120, 100%, 50%)", "#0000FF", "rgb(0, 0, 255)", "hsl(240, 100%, 50%)"),
        ("#808080", "#808080", "rgb(128, 128, 128)", "hsl(0, 0%, 50%)"),
    ];
    for (input, hex, rgb, hsl) in cases {
        let color = parse(input);
        assert_eq!(hex, color.to_hex(), "{input}");
        assert_eq!(rgb, color.to_rgb_text(), "{input}");
        assert_eq!(hsl, color.to_hsl_text(), "{input}");
    }
}

#[test]
fn hsl_input_is_echoed_not_recomputed_from_quantized_rgb() {
    // hsl(120, 10%, 5%) 8 bitlik kanala inince S çok kayar; kullanıcının yazdığı değer görünmeli.
    assert_eq!("hsl(120, 10%, 5%)", parse("hsl(120, 10%, 5%)").to_hsl_text());
}

#[test]
fn hsl_fractions_round_for_display() {
    assert_eq!("hsl(32, 100%, 50%)", parse("hsl(32.4, 99.6%, 50.4%)").to_hsl_text());
}

#[test]
fn hsl_hue_that_rounds_to_360_shows_zero() {
    assert!(parse("hsl(359.6, 100%, 50%)").to_hsl_text().starts_with("hsl(0,"));
}

#[test]
fn hex_rgb_hsl_round_trips() {
    for hex in ["#FF8800", "#3B82F6", "#000000", "#FFFFFF", "#123456", "#ABCDEF", "#7F7F7F"] {
        let original = parse(hex);

        let via_rgb = parse(&original.to_rgb_text());
        assert_eq!(original.color, via_rgb.color, "{hex}");

        // HSL tam sayıya yuvarlanır; bir kanalda en çok birkaç birim sapma olabilir.
        let via_hsl = parse(&original.to_hsl_text());
        assert!((via_hsl.color.r as i32 - original.color.r as i32).abs() <= 3, "{hex}");
        assert!((via_hsl.color.g as i32 - original.color.g as i32).abs() <= 3, "{hex}");
        assert!((via_hsl.color.b as i32 - original.color.b as i32).abs() <= 3, "{hex}");

        let via_hex = parse(&original.to_hex());
        assert_eq!(original.color, via_hex.color, "{hex}");
    }
}

#[test]
fn round_trip_over_the_whole_primary_grid() {
    for r in (0..256).step_by(51) {
        for g in (0..256).step_by(51) {
            for b in (0..256).step_by(51) {
                let rgb = Rgb::new(r as u8, g as u8, b as u8);
                let parsed = parse(&rgb.to_hex());
                assert_eq!(rgb, parsed.color);
                let again = parse(&parsed.to_rgb_text());
                assert_eq!(rgb, again.color);
            }
        }
    }
}

#[test]
fn color_source_returns_hex_rgb_hsl_rows_each_copyable() {
    let shell = FakeShell::new();
    let source = ColorConverterSource::new(shell.clone(), identity_t());
    let results = source.search(&q("#FF8800"));

    let expected = ["#FF8800", "rgb(255, 136, 0)", "hsl(32, 100%, 50%)"];
    assert_eq!(expected.to_vec(), results.iter().map(|r| r.title.as_str()).collect::<Vec<_>>());
    assert_eq!("color", source.id());
    for r in &results {
        assert!(r.restores_focus);
        assert_eq!(SearchRelevance::HIGHEST, r.category_relevance);
        assert!(r.subtitle.as_deref().unwrap().ends_with("SearchHintCopy"));
    }
    let mut unique: Vec<&str> = results.iter().map(|r| r.id.as_str()).collect();
    unique.sort();
    unique.dedup();
    assert_eq!(3, unique.len());
    assert!(results[0].relevance > results[1].relevance && results[1].relevance > results[2].relevance);

    for row in &results {
        (row.execute)();
    }

    assert_eq!(expected.iter().map(|e| format!("copy {e}")).collect::<Vec<_>>(), shell.log());
}

#[test]
fn color_source_gives_the_same_three_rows_for_any_input_form() {
    for input in ["rgb(255,136,0)", "hsl(32, 100%, 50%)", "#f80"] {
        let results = ColorConverterSource::new(FakeShell::new(), identity_t()).search(&q(input));
        assert_eq!(
            vec!["#FF8800", "rgb(255, 136, 0)", "hsl(32, 100%, 50%)"],
            results.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(),
            "{input}"
        );
    }
}

#[test]
fn color_source_ignores_non_colors() {
    for text in ["", "chrome", "#zzz", "rgb(999,0,0)", "10 km to mi"] {
        assert!(ColorConverterSource::new(FakeShell::new(), identity_t()).search(&q(text)).is_empty(), "{text}");
    }
}

// ---- Kaynaklar birbirine ve hesap makinesine karışmaz ----

#[test]
fn the_calculator_does_not_claim_converter_queries() {
    for text in [
        "10 km to mi",
        "100 c in f",
        "5 kg lb",
        "1 gb in mb",
        "60 mph to km/h",
        "#FF8800",
        "rgb(255, 136, 0)",
        "hsl(32, 100%, 50%)",
    ] {
        assert!(CalculatorSearchSource::new(FakeShell::new(), identity_t(), en()).search(&q(text)).is_empty(), "{text}");
    }
}

#[test]
fn colors_are_not_units() {
    for text in ["#FF8800", "rgb(255, 136, 0)", "hsl(32, 100%, 50%)"] {
        assert!(UnitConverterSource::new(FakeShell::new(), identity_t(), en()).search(&q(text)).is_empty(), "{text}");
    }
}

#[test]
fn units_are_not_colors() {
    for text in ["10 km to mi", "100 c in f", "1 gb in mb"] {
        assert!(ColorConverterSource::new(FakeShell::new(), identity_t()).search(&q(text)).is_empty(), "{text}");
    }
}

#[test]
fn help_lists_documented_converter_sources() {
    let mut units = UnitConverterSource::new(FakeShell::new(), identity_t(), en());
    units.base.syntaxes = vec![SearchSyntax::new("10 km to mi", "x")];
    let mut color = ColorConverterSource::new(FakeShell::new(), identity_t());
    color.base.syntaxes = vec![SearchSyntax::new("#FF8800", "y")];
    let all: Vec<Arc<dyn ISearchSource>> = vec![Arc::new(units), Arc::new(color)];
    let help = HelpSearchSource::new(Box::new(move || all.clone()), identity_t());

    let rows = help.search(&q("?"));
    assert_eq!(2, rows.len());
    assert!(rows.iter().any(|r| r.title == "10 km to mi"));
    assert!(rows.iter().any(|r| r.title == "#FF8800"));
}
