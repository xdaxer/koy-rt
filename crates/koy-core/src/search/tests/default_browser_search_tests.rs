use crate::search::*;

#[test]
fn recognizes_browser_from_open_command() {
    let cases = [
        (r#""C:\Program Files\Google\Chrome\Application\chrome.exe" --single-argument %1"#, "Chrome", BrowserFamily::Chromium),
        (r#""C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --single-argument %1"#, "Edge", BrowserFamily::Chromium),
        (r#""C:\Program Files\Mozilla Firefox\firefox.exe" -osint -url "%1""#, "Firefox", BrowserFamily::Firefox),
        (r"C:\Brave\brave.exe --single-argument %1", "Brave", BrowserFamily::Chromium),
    ];
    for (command, name, family) in cases {
        let browser = DefaultBrowserSearch::from_open_command(Some(command)).unwrap_or_else(|| panic!("{command}"));
        assert_eq!(name, browser.name, "{command}");
        assert_eq!(family, browser.family, "{command}");
        assert!(browser.executable.ends_with(".exe"), "{command}");
    }
}

#[test]
fn unknown_or_missing_browser_gives_null() {
    let cases = [
        None,
        Some(""),
        Some(r#""C:\Tools\unknown.exe" %1"#),
        Some(r#""C:\Opera\launcher.exe" %1"#),
        Some("rundll32.exe url.dll,FileProtocolHandler %1"),
    ];
    for command in cases {
        assert!(DefaultBrowserSearch::from_open_command(command).is_none(), "{command:?}");
    }
}

#[test]
fn chromium_search_is_a_single_question_mark_argument() {
    let command = DefaultBrowserSearch::search_command(
        &DefaultBrowser::new("chrome.exe", "Chrome", BrowserFamily::Chromium),
        "  --incognito kedi ",
    )
    .unwrap();

    // Terim seçenek gibi başlasa da "?" ile tek adres olarak gider.
    assert_eq!(vec!["--single-argument", "? --incognito kedi"], command.arguments);
}

#[test]
fn firefox_uses_search_option_but_not_for_dash_terms() {
    let firefox = DefaultBrowser::new("firefox.exe", "Firefox", BrowserFamily::Firefox);

    assert_eq!(vec!["-search", "kedi"], DefaultBrowserSearch::search_command(&firefox, "kedi").unwrap().arguments);
    assert!(DefaultBrowserSearch::search_command(&firefox, "-private").is_none());
}

#[test]
fn empty_term_gives_no_command() {
    assert!(
        DefaultBrowserSearch::search_command(&DefaultBrowser::new("chrome.exe", "Chrome", BrowserFamily::Chromium), "  ").is_none()
    );
}
