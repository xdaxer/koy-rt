use crate::clipboard::*;
use crate::time::{add_span, DateTimeOffset, TimeSpan};
use chrono::{FixedOffset, TimeZone};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn classify(text: &str) -> ClipboardKind {
    ClipboardClassifier::classify(Some(text))
}

#[test]
fn classifies_simple_kinds() {
    let cases = [
        ("https://koy.app/path?q=1", ClipboardKind::Link),
        ("  http://example.com  ", ClipboardKind::Link),
        ("ftp://example.com", ClipboardKind::Text),
        ("https://a.com ve baska metin", ClipboardKind::Text),
        ("example.com", ClipboardKind::Text),
        ("#FFF", ClipboardKind::Color),
        ("#a78bfa", ClipboardKind::Color),
        ("#A78BFA80", ClipboardKind::Color),
        ("#12345", ClipboardKind::Text),
        ("#GGGGGG", ClipboardKind::Text),
        ("rgb(10, 20, 30)", ClipboardKind::Color),
        ("rgba(10,20,30,0.5)", ClipboardKind::Color),
        ("RGB(255,255,255)", ClipboardKind::Color),
        ("rgb(10, 20)", ClipboardKind::Text),
        ("Merhaba dünya", ClipboardKind::Text),
        ("", ClipboardKind::Text),
        ("   ", ClipboardKind::Text),
    ];
    for (text, expected) in cases {
        assert_eq!(expected, classify(text), "{text:?}");
    }
}

#[test]
fn null_is_text() {
    assert_eq!(ClipboardKind::Text, ClipboardClassifier::classify(None));
}

#[test]
fn multiline_code_with_braces_and_semicolons_is_code() {
    let code = "public int Add(int a, int b)\n{\n    return a + b;\n}";
    assert_eq!(ClipboardKind::Code, classify(code));
}

#[test]
fn python_with_indentation_and_keyword_is_code() {
    let code = "def topla(a, b):\n    sonuc = a + b\n    return sonuc";
    assert_eq!(ClipboardKind::Code, classify(code));
}

#[test]
fn html_block_is_code() {
    assert_eq!(ClipboardKind::Code, classify("<div class=\"a\">\n  <span>x</span>\n</div>"));
}

#[test]
fn single_line_with_strong_signal_is_code() {
    for line in ["var x = Foo(1);", "const f = (a) => a + 1", "if (x == 1) {"] {
        assert_eq!(ClipboardKind::Code, classify(line), "{line}");
    }
}

#[test]
fn plain_multiline_prose_is_text() {
    let prose = "Merhaba,\nYarınki toplantı saat 10'da.\nGörüşürüz.";
    assert_eq!(ClipboardKind::Text, classify(prose));
}

#[test]
fn single_line_prose_with_a_semicolon_or_parenthesis_is_text() {
    assert_eq!(ClipboardKind::Text, classify("Geldim, gördüm; yendim (neredeyse)"));
    assert_eq!(ClipboardKind::Text, classify("Bunu sonra konuşuruz (belki)."));
}

// --- ClipboardHistoryTests ---

fn new_history() -> ClipboardHistory {
    // Her çağrıda bir saniye ilerleyen saat.
    let t: Mutex<DateTimeOffset> = Mutex::new(FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2026, 11, 6, 10, 0, 0).unwrap());
    let t = Arc::new(t);
    new_history_with(t)
}

fn new_history_with(t: Arc<Mutex<DateTimeOffset>>) -> ClipboardHistory {
    ClipboardHistory::new(Some(Box::new(move || {
        let mut g = t.lock().unwrap();
        *g = add_span(*g, TimeSpan::from_secs(1));
        *g
    })))
}

fn texts(items: &[ClipboardItem]) -> Vec<String> {
    items.iter().map(|i| i.text.clone()).collect()
}

fn query(h: &ClipboardHistory) -> Vec<ClipboardItem> {
    h.query(ClipboardFilter::All, None)
}

#[test]
fn add_puts_newest_first_and_classifies() {
    let h = new_history();
    h.add(Some("bir"));
    h.add(Some("https://a.com"));

    let all = query(&h);
    assert_eq!(vec!["https://a.com", "bir"], texts(&all));
    assert_eq!(ClipboardKind::Link, all[0].kind);
}

#[test]
fn blank_text_is_ignored() {
    for text in [None, Some(""), Some("  \n ")] {
        let h = new_history();
        assert!(h.add(text).is_none(), "{text:?}");
        assert!(query(&h).is_empty(), "{text:?}");
    }
}

#[test]
fn duplicate_moves_to_front_without_adding() {
    let h = new_history();
    let first = h.add(Some("a")).unwrap();
    h.add(Some("b"));
    let again = h.add(Some("a")).unwrap();

    assert_eq!(first.id, again.id);
    assert_eq!(vec!["a", "b"], texts(&query(&h)));
}

#[test]
fn long_text_is_truncated() {
    let h = new_history();
    let item = h.add(Some(&"x".repeat(ClipboardHistory::MAX_TEXT_LENGTH as usize + 500))).unwrap();
    assert_eq!(ClipboardHistory::MAX_TEXT_LENGTH as usize, item.text.encode_utf16().count());
}

#[test]
fn keeps_at_most_50_unpinned_dropping_oldest() {
    let h = new_history();
    for i in 0..60 {
        h.add(Some(&format!("öğe {i}")));
    }

    let all = query(&h);
    assert_eq!(ClipboardHistory::MAX_UNPINNED as usize, all.len());
    assert_eq!("öğe 59", all[0].text);
    assert!(!all.iter().any(|x| x.text == "öğe 0"));
}

#[test]
fn pinned_items_do_not_count_toward_the_limit_and_survive_trimming() {
    let h = new_history();
    let pinned = h.add(Some("sabit")).unwrap();
    h.set_pinned(pinned.id, true);
    for i in 0..70 {
        h.add(Some(&format!("öğe {i}")));
    }

    assert_eq!(ClipboardHistory::MAX_UNPINNED as usize + 1, query(&h).len());
    assert!(h.query(ClipboardFilter::Pinned, None).iter().any(|x| x.text == "sabit"));
}

#[test]
fn duplicate_of_pinned_keeps_it_pinned() {
    let h = new_history();
    let item = h.add(Some("a")).unwrap();
    h.set_pinned(item.id, true);
    let again = h.add(Some("a")).unwrap();
    assert!(again.pinned);
    assert_eq!(1, query(&h).len());
}

#[test]
fn search_is_case_insensitive() {
    let h = new_history();
    h.add(Some("Merhaba Dünya"));
    h.add(Some("başka"));

    assert_eq!(1, h.query(ClipboardFilter::All, Some("merhaba")).len());
    assert_eq!(1, h.query(ClipboardFilter::All, Some("DÜNYA")).len());
    assert!(h.query(ClipboardFilter::All, Some("yok")).is_empty());
}

#[test]
fn filters_select_by_kind_and_pinned() {
    let h = new_history();
    h.add(Some("https://a.com"));
    h.add(Some("#FF0000"));
    h.add(Some("düz"));
    let code = h.add(Some("var x = Foo(1);")).unwrap();
    h.set_pinned(code.id, true);

    assert_eq!(1, h.query(ClipboardFilter::Link, None).len());
    assert_eq!(1, h.query(ClipboardFilter::Color, None).len());
    assert_eq!(1, h.query(ClipboardFilter::Code, None).len());
    let pinned = h.query(ClipboardFilter::Pinned, None);
    assert_eq!(1, pinned.len());
    assert_eq!(code.id, pinned[0].id);
    assert_eq!(4, h.query(ClipboardFilter::All, None).len());
}

#[test]
fn unpinning_over_the_limit_trims_oldest() {
    let h = new_history();
    let old = h.add(Some("eski")).unwrap();
    h.set_pinned(old.id, true);
    for i in 0..ClipboardHistory::MAX_UNPINNED {
        h.add(Some(&format!("öğe {i}")));
    }

    h.set_pinned(old.id, false);

    assert_eq!(ClipboardHistory::MAX_UNPINNED as usize, query(&h).len());
    assert!(!query(&h).iter().any(|x| x.text == "eski"));
}

#[test]
fn remove_deletes_single_item_even_if_pinned() {
    let h = new_history();
    let a = h.add(Some("a")).unwrap();
    h.add(Some("b"));
    h.set_pinned(a.id, true);

    assert!(h.remove(a.id));
    assert!(!h.remove(a.id));
    assert_eq!(vec!["b"], texts(&query(&h)));
}

#[test]
fn clear_keeps_pinned() {
    let h = new_history();
    let a = h.add(Some("a")).unwrap();
    h.add(Some("b"));
    h.add(Some("c"));
    h.set_pinned(a.id, true);

    assert_eq!(2, h.clear());
    assert_eq!(vec!["a"], texts(&query(&h)));
}

#[test]
fn load_pinned_restores_marked_pinned_and_skips_duplicates() {
    let t = Arc::new(Mutex::new(FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2026, 11, 6, 10, 0, 0).unwrap()));
    let h = new_history_with(Arc::clone(&t));
    h.add(Some("canlı"));
    let id = Guid::new_guid();
    let now = *t.lock().unwrap();
    h.load_pinned([
        ClipboardItem::new(id, "diskten", ClipboardKind::Text, false, now),
        ClipboardItem::new(Guid::new_guid(), "canlı", ClipboardKind::Text, true, now),
    ]);

    assert_eq!(2, query(&h).len());
    assert!(h.get(id).unwrap().pinned);
}

// --- ClipboardPinnedStoreTests ---

/// C# testindeki `IDisposable` geçici klasör.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        TempDir(std::env::temp_dir().join(format!("koy-clip-{}", Guid::new_guid())))
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn unix_epoch() -> DateTimeOffset {
    FixedOffset::east_opt(0).unwrap().timestamp_opt(0, 0).unwrap()
}

#[test]
fn round_trips_only_pinned() {
    let dir = TempDir::new();
    let store = ClipboardPinnedStore::new(Some(dir.file("p.json")), None);
    let pinned = ClipboardItem::new(Guid::new_guid(), "sabit", ClipboardKind::Code, true, unix_epoch());
    let loose = ClipboardItem::new(Guid::new_guid(), "geçici", ClipboardKind::Text, false, unix_epoch());

    assert!(store.save([&pinned, &loose]));

    assert_eq!(vec![pinned], store.load());
}

#[test]
fn missing_file_is_empty_without_error() {
    let dir = TempDir::new();
    let errors: Arc<Mutex<Vec<String>>> = Arc::default();
    let e = Arc::clone(&errors);
    let store = ClipboardPinnedStore::new(
        Some(dir.file("yok.json")),
        Some(Arc::new(move |s: &str, _: &dyn std::error::Error| e.lock().unwrap().push(s.to_string()))),
    );

    assert!(store.load().is_empty());
    assert!(errors.lock().unwrap().is_empty());
}

#[test]
fn corrupt_file_is_empty_and_reported() {
    let dir = TempDir::new();
    std::fs::create_dir_all(&dir.0).unwrap();
    let path = dir.file("p.json");
    std::fs::write(&path, "{ bozuk").unwrap();
    let errors: Arc<Mutex<Vec<String>>> = Arc::default();
    let e = Arc::clone(&errors);
    let store = ClipboardPinnedStore::new(Some(path), Some(Arc::new(move |s: &str, _: &dyn std::error::Error| e.lock().unwrap().push(s.to_string()))));

    assert!(store.load().is_empty());
    assert_eq!(1, errors.lock().unwrap().len());
}
