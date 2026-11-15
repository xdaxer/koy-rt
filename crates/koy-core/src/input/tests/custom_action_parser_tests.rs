//! Karşılığı: `CustomActionParserTests.cs`.

use crate::input::*;

fn data(id: &str, kind: &str, target: &str, args: Option<&str>, dir: Option<&str>) -> Option<CustomActionData> {
    Some(CustomActionData {
        id: Some(id.to_string()),
        kind: Some(kind.to_string()),
        target: Some(target.to_string()),
        arguments: args.map(str::to_string),
        working_directory: dir.map(str::to_string),
    })
}

fn d(id: &str, kind: &str, target: &str) -> Option<CustomActionData> {
    data(id, kind, target, None, None)
}

#[test]
fn parse_gecerli_kayit_kabul_edilir() {
    let result = CustomActionParser::parse([data("build", "RunCommand", "dotnet", Some("build -c Release"), Some(r"C:\src"))]);

    assert_eq!(1, result.accepted.len());
    let action = &result.accepted[0];
    assert_eq!("build", action.id);
    assert_eq!(CustomActionKind::RunCommand, action.kind);
    assert_eq!("dotnet", action.target);
    assert_eq!(Some("build -c Release"), action.arguments.as_deref());
    assert_eq!(Some(r"C:\src"), action.working_directory.as_deref());
    assert_eq!(0, result.dropped);
}

#[test]
fn parse_bos_kimlik_ve_hedef_dusurulur() {
    let result = CustomActionParser::parse([d(" ", "OpenPath", r"C:\x"), d("a", "OpenPath", "  ")]);

    assert!(result.accepted.is_empty());
    assert_eq!(2, result.dropped);
}

#[test]
fn parse_bilinmeyen_tur_dusurulur() {
    let result = CustomActionParser::parse([d("a", "RunPlugin", "x")]);

    assert!(result.accepted.is_empty());
    assert_eq!(1, result.dropped);
}

#[test]
fn parse_kimlik_tekildir_ilk_kalir() {
    let result = CustomActionParser::parse([d("Build", "OpenPath", r"C:\a"), d("build", "OpenPath", r"C:\b")]);

    assert_eq!(1, result.accepted.len());
    assert_eq!(r"C:\a", result.accepted[0].target);
    assert_eq!(1, result.dropped);
}

#[test]
fn parse_url_yalniz_http_ve_https() {
    let result = CustomActionParser::parse([
        d("ok", "OpenUrl", "https://example.com/yol"),
        d("ftp", "OpenUrl", "ftp://example.com"),
        d("goreli", "OpenUrl", "/yol"),
        d("dosya", "OpenUrl", r"C:\dosya.txt"),
    ]);

    let ids: Vec<&str> = result.accepted.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(vec!["ok"], ids);
    assert_eq!(3, result.dropped);
}

#[test]
fn parse_bos_arguman_ve_dizin_null_olur() {
    let result = CustomActionParser::parse([data("a", "OpenPath", r"C:\x", Some("  "), Some(""))]);

    assert_eq!(1, result.accepted.len());
    assert!(result.accepted[0].arguments.is_none());
    assert!(result.accepted[0].working_directory.is_none());
}

#[test]
fn parse_bos_liste() {
    let result = CustomActionParser::parse([]);

    assert!(result.accepted.is_empty());
    assert_eq!(0, result.dropped);
}

#[test]
fn gecersiz_kayit_kimligi_rezerve_etmez_sonraki_gecerli_kayit_kabul_edilir() {
    let result = CustomActionParser::parse([
        d("link", "OpenUrl", "ftp://ornek.example/x"),
        d("link", "OpenUrl", "https://ornek.example/x"),
    ]);

    assert_eq!(1, result.accepted.len());
    assert_eq!("https://ornek.example/x", result.accepted[0].target);
    assert_eq!(1, result.dropped);
}

#[test]
fn parse_null_oge_cokertmez_dusurulur() {
    let result = CustomActionParser::parse([None, d("a", "OpenPath", r"C:\x")]);

    assert_eq!(1, result.accepted.len());
    assert_eq!("a", result.accepted[0].id);
    assert_eq!(1, result.dropped);
}

#[test]
fn parse_en_cok_max_actions_kabul_edilir() {
    let items: Vec<Option<CustomActionData>> =
        (0..CustomActionParser::MAX_ACTIONS + 5).map(|i| d(&format!("a{i}"), "OpenPath", r"C:\x")).collect();

    let result = CustomActionParser::parse(items);

    assert_eq!(CustomActionParser::MAX_ACTIONS, result.accepted.len());
    assert_eq!(5, result.dropped);
}
