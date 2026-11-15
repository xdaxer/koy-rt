use crate::notch::NotchCardKind;
use crate::notifications::*;
use crate::time::{DateTimeOffset, TimeSpan, add_span};
use chrono::{TimeZone, Utc};

fn now() -> DateTimeOffset {
    Utc.with_ymd_and_hms(2026, 11, 11, 12, 0, 0).unwrap().fixed_offset()
}

fn labels() -> RelativeTimeLabels {
    RelativeTimeLabels::new("just now", "{0} min ago", "{0} h ago")
}

/// C# `Item(app = "Mail", title = "Title", body = "Body", time = Now, icon = null)`.
fn item_full(app: &str, title: &str, body: &str, time: DateTimeOffset, icon: Option<Vec<u8>>) -> NotificationItem {
    NotificationItem { icon, ..NotificationItem::new(1, app, title, body, time) }
}

fn item() -> NotificationItem {
    item_full("Mail", "Title", "Body", now(), None)
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

#[test]
fn builds_header_title_and_body() {
    let card = NotificationPresentation::to_card(&item(), now(), false, &labels());

    assert_eq!(card.kind, NotchCardKind::Notification);
    assert_eq!(card.header, "Mail · just now");
    assert_eq!(card.title, "Title");
    assert_eq!(card.subtitle, "Body");
    assert!(!card.has_primary_action());
}

#[test]
fn carries_app_name_time_and_icon() {
    let icon = vec![1u8, 2, 3];
    let card = NotificationPresentation::to_card(&item_full("Mail", "Title", "Body", now(), Some(icon.clone())), now(), false, &labels());

    assert_eq!(card.app_name, "Mail");
    assert_eq!(card.timestamp, Some(now()));
    assert_eq!(card.icon, Some(icon));
}

#[test]
fn hide_body_keeps_only_app_and_title() {
    let card = NotificationPresentation::to_card(&item_full("Mail", "Title", "secret", now(), None), now(), true, &labels());

    assert_eq!(card.title, "Title");
    assert_eq!(card.subtitle, "");
    assert!(!card.has_subtitle());
}

#[test]
fn body_only_notification_uses_body_as_title() {
    let card = NotificationPresentation::to_card(&item_full("Mail", "", "Just text", now(), None), now(), false, &labels());

    assert_eq!(card.title, "Just text");
    assert_eq!(card.subtitle, "");
}

#[test]
fn missing_app_name_drops_the_separator() {
    let card = NotificationPresentation::to_card(&item_full("", "Title", "Body", now(), None), now(), false, &labels());
    assert_eq!(card.header, "just now");
}

#[test]
fn formats_relative_time() {
    let cases: [(i64, &str); 7] = [
        (0, "just now"),
        (59, "just now"),
        (60, "1 min ago"),
        (5 * 60 + 30, "5 min ago"),
        (59 * 60, "59 min ago"),
        (3600, "1 h ago"),
        (5 * 3600, "5 h ago"),
    ];
    for (seconds_ago, expected) in cases {
        let time = add_span(now(), TimeSpan::from_secs(-seconds_ago));
        assert_eq!(NotificationPresentation::format_age(time, now(), &labels()), expected, "secondsAgo = {seconds_ago}");
    }
}

#[test]
fn future_timestamp_counts_as_now() {
    let time = add_span(now(), TimeSpan::from_mins(3));
    assert_eq!(NotificationPresentation::format_age(time, now(), &labels()), "just now");
}

#[test]
fn truncates_long_text_with_ellipsis() {
    let result = NotificationPresentation::truncate(Some(&"a".repeat(200)), 20);

    assert_eq!(utf16_len(&result), 20);
    assert!(result.ends_with('…'));
}

#[test]
fn leaves_short_text_alone() {
    assert_eq!(NotificationPresentation::truncate(Some("short"), 20), "short");
}

#[test]
fn collapses_newlines_and_repeated_spaces() {
    assert_eq!(NotificationPresentation::truncate(Some("  a\r\n\n b \t c  "), 20), "a b c");
}

#[test]
fn does_not_split_a_surrogate_pair() {
    // 9 harf + 3 emoji (her biri 2 char); 11'de kesince çift ortadan bölünmemeli.
    let text = format!("{}😀😀😀", "a".repeat(9));
    let result = NotificationPresentation::truncate(Some(&text), 11);

    assert!(utf16_len(&result) <= 11);
    // Sondaki "…" atılınca son birim yüksek vekil olmamalı (Rust dizesinde yarım çift zaten olamaz; kesim `a`'larda biter).
    let without_ellipsis: String = result.chars().take(result.chars().count() - 1).collect();
    let last_unit = without_ellipsis.encode_utf16().last();
    assert!(!last_unit.is_some_and(|u| (0xD800..=0xDBFF).contains(&u)));
    assert_eq!(result, format!("{}…", "a".repeat(9)));
}

#[test]
fn long_title_body_and_app_fit_the_card_limits() {
    let long = item_full(&"x".repeat(100), &"t".repeat(500), &"b".repeat(500), now(), None);
    let card = NotificationPresentation::to_card(&long, now(), false, &labels());

    assert!(utf16_len(&card.title) <= NotificationPresentation::MAX_TITLE_LENGTH);
    assert!(utf16_len(&card.subtitle) <= NotificationPresentation::MAX_BODY_LENGTH);
    assert!(utf16_len(&card.app_name) <= NotificationPresentation::MAX_APP_NAME_LENGTH);
}

#[test]
fn null_or_blank_text_becomes_empty() {
    assert_eq!(NotificationPresentation::truncate(None, 10), "");
    assert_eq!(NotificationPresentation::truncate(Some("   "), 10), "");
}

#[test]
fn each_card_keeps_its_own_icons_and_fallback() {
    let labels = RelativeTimeLabels::new("now", "{0}m", "{0}h");
    let now = chrono::Utc::now().fixed_offset();
    let (a, b, fa) = (vec![1u8], vec![2u8], vec![3u8]);

    let card_a = NotificationPresentation::to_card(&NotificationItem::new(1, "A", "t", "b", now).with_icon(a.clone()).with_fallback_icon(fa.clone()), now, false, &labels);
    let card_b = NotificationPresentation::to_card(&NotificationItem::new(2, "B", "t", "b", now).with_icon(b.clone()), now, false, &labels);

    assert_eq!(card_a.icon, Some(a));
    assert_eq!(card_a.fallback_icon, Some(fa));
    assert_eq!(card_b.icon, Some(b));
    assert_eq!(card_b.fallback_icon, None);
}
