use crate::search::{SearchMatcher, SearchQuery, SearchText};

fn score(query: &str, text: &str) -> f64 {
    SearchMatcher::score(&SearchQuery::from_text(query), &SearchText::fold_str(text))
}

#[test]
fn fold_handles_turkish_letters() {
    let cases = [
        ("IŞIK", "isik"),
        ("ışık", "isik"),
        ("İstanbul", "istanbul"),
        ("ÇĞÖŞÜ", "cgosu"),
        ("i\u{307}", "i"),
        ("", ""),
    ];
    for (input, expected) in cases {
        assert_eq!(expected, SearchText::fold(Some(input)), "{input:?}");
    }
}

#[test]
fn query_splits_words_and_trims() {
    let q = SearchQuery::from_text("  Not   Defteri ");

    assert_eq!("Not   Defteri", q.raw);
    assert_eq!(vec!["not", "defteri"], q.words);
}

#[test]
fn empty_query_matches_nothing() {
    assert_eq!(0.0, score("   ", "Chrome"));
}

#[test]
fn exact_match_is_best() {
    assert_eq!(1.0, score("chrome", "Chrome"));
}

#[test]
fn turkish_query_written_without_diacritics_matches() {
    assert!(score("sifre", "Şifre Yöneticisi") > 0.0);
}

#[test]
fn prefix_beats_later_word_beats_contained() {
    let prefix = score("not", "Notepad");
    let word = score("not", "Microsoft Notes App");
    let inside = score("not", "Canoto Player");

    assert!(prefix > word);
    assert!(word > inside);
    assert!(inside > 0.0);
}

#[test]
fn shorter_name_wins_between_same_kind_of_match() {
    assert!(score("not", "Notes") > score("not", "Notes and Reminders Manager"));
}

#[test]
fn initials_match() {
    assert!(score("vsc", "Visual Studio Code") > 0.0);
}

#[test]
fn every_word_must_match() {
    assert!(score("not def", "Not Defteri") > 0.0);
    assert_eq!(0.0, score("not xyz", "Not Defteri"));
}

#[test]
fn one_typo_is_tolerated() {
    let cases = [
        ("chorme", "Chrome"), // yan yana iki harf
        ("chrme", "Chrome"),  // eksik harf
        ("chroome", "Chrome"), // fazla harf
        ("chrone", "Chrome"), // yanlış harf
    ];
    for (query, text) in cases {
        assert!(score(query, text) > 0.0, "{query} / {text}");
    }
}

#[test]
fn typo_ranks_below_real_match() {
    assert!(score("chrome", "Chrome Canary") > score("chrome", "Chrone"));
}

#[test]
fn short_terms_get_no_typo_tolerance() {
    assert_eq!(0.0, score("vsz", "Visual"));
}
