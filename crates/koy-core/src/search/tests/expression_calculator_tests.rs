use crate::clock::Culture;
use crate::search::{Decimal, ExpressionCalculator};

fn tr() -> Culture {
    Culture::tr_tr()
}

fn en() -> Culture {
    Culture::en_us()
}

fn eval(text: &str, culture: Option<Culture>) -> Decimal {
    ExpressionCalculator::try_evaluate(text, &culture.unwrap_or_else(tr)).unwrap_or_else(|| panic!("{text}"))
}

/// `(decimal)double`.
fn dec(value: f64) -> Decimal {
    Decimal::from_f64(value).unwrap()
}

#[test]
fn evaluates() {
    let cases = [
        ("12*8", 96.0),
        ("2+3*4", 14.0),
        ("(2+3)*4", 20.0),
        ("10/4", 2.5),
        ("10-2-3", 5.0),
        ("2^10", 1024.0),
        ("2^3^2", 512.0),
        ("-2^2", -4.0),
        ("2^-1", 0.5),
        ("2*-3", -6.0),
        ("=1+1", 2.0),
        (" 1 + 1 ", 2.0),
        ("12x8", 96.0),
        ("12×8", 96.0),
        ("12÷8", 1.5),
    ];
    for (text, expected) in cases {
        assert_eq!(dec(expected), eval(text, None), "{text}");
    }
}

#[test]
fn decimal_arithmetic_is_exact() {
    assert_eq!(Decimal::parse("0.3").unwrap(), eval("0,1+0,2", None));
}

#[test]
fn turkish_number_formats() {
    let cases = [
        ("3,5*2", 7.0),
        ("3.5*2", 7.0), // alışkanlık: nokta da ondalık sayılır
        ("1.000*2", 2000.0), // Türkçede üç rakamlı grup binliktir
        ("1.000.000+1", 1000001.0),
    ];
    for (text, expected) in cases {
        assert_eq!(dec(expected), eval(text, Some(tr())), "{text}");
    }
}

#[test]
fn english_number_formats() {
    let cases = [("1,000*2", 2000.0), ("3.5*2", 7.0), ("3,5*2", 7.0)];
    for (text, expected) in cases {
        assert_eq!(dec(expected), eval(text, Some(en())), "{text}");
    }
}

#[test]
fn rejects() {
    let cases = [
        "123",   // işlem yok
        "-5",    // yalnız işaret
        "merhaba",
        "chrome-7",
        "1+",
        "(1+2",
        "1+2)",
        "1/0",
        "1,2,3+1",
        "",
    ];
    for text in cases {
        assert!(ExpressionCalculator::try_evaluate(text, &tr()).is_none(), "{text:?}");
    }
}

#[test]
fn too_long_input_is_rejected() {
    let text = vec!["1"; ExpressionCalculator::MAX_LENGTH].join("+");

    assert!(ExpressionCalculator::try_evaluate(&text, &tr()).is_none());
}

#[test]
fn deep_nesting_is_rejected_without_crashing() {
    let text = format!("1+{}1{}", "(".repeat(60), ")".repeat(60));

    assert!(ExpressionCalculator::try_evaluate(&text, &tr()).is_none());
}

#[test]
fn repeated_signs_are_limited_without_crashing() {
    assert!(ExpressionCalculator::try_evaluate(&format!("1+{}1", "-".repeat(100)), &tr()).is_none());
}

#[test]
fn overflow_gives_no_result() {
    assert!(ExpressionCalculator::try_evaluate("9^1000", &tr()).is_none());
}

#[test]
fn huge_exponent_is_rejected_quickly() {
    assert!(ExpressionCalculator::try_evaluate("1^99999999", &tr()).is_none());
}

#[test]
fn functions_hex_and_trailing_equals() {
    let cases = [
        ("sqrt(16)+1", 5.0),
        ("abs(-3)", 3.0),
        ("floor(2,9)", 2.0),
        ("round(2,5)+1", 3.0),
        ("log(1000)", 3.0),
        ("0x10+1", 17.0),
        ("1+1=", 2.0),
    ];
    for (text, expected) in cases {
        assert_eq!(dec(expected), eval(text, None), "{text}");
    }
}

#[test]
fn constants_work() {
    assert_eq!(Decimal::parse("6.28").unwrap(), eval("2*pi", None).round(2, false));
}

#[test]
fn bad_functions_give_no_result() {
    for text in ["foo(2)", "sqrt(-1)", "sqrt("] {
        assert!(ExpressionCalculator::try_evaluate(text, &tr()).is_none(), "{text}");
    }
}

#[test]
fn base_prefix_formats_result() {
    let cases = [("hex=255", "0xFF"), ("bin=5", "0b101"), ("oct=8", "010"), ("hex=250+5", "0xFF")];
    for (text, expected) in cases {
        let (display, copy) = ExpressionCalculator::try_calculate(text, &tr()).unwrap_or_else(|| panic!("{text}"));
        assert_eq!((expected, expected), (display.as_str(), copy.as_str()), "{text}");
    }
}

#[test]
fn base_prefix_needs_a_whole_number() {
    assert!(ExpressionCalculator::try_calculate("hex=1/2", &tr()).is_none());
}

#[test]
fn display_uses_grouping_and_copy_does_not() {
    let value = Decimal::parse("1234.5").unwrap();
    assert_eq!("1.234,5", ExpressionCalculator::format_for_display(&value, &tr()));
    assert_eq!("1234,5", ExpressionCalculator::format_for_copy(&value, &tr()));
}
