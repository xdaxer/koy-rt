//! Karşılığı: `Calendar/CalendarMergeAndCacheTests.cs`.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{Duration, NaiveDate, NaiveTime, Utc};

use crate::calendar::*;
use crate::clock::TimeZoneInfo;

struct FakeProtector;

impl ITokenProtector for FakeProtector {
    fn protect(&self, plain_bytes: &[u8]) -> Vec<u8> {
        plain_bytes.iter().map(|b| b ^ 0x5A).collect()
    }

    fn unprotect(&self, protected_bytes: &[u8]) -> Vec<u8> {
        protected_bytes.iter().map(|b| b ^ 0x5A).collect()
    }
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn event_book_merges_local_and_external_events_and_protects_read_only_events() {
    let mut book = EventBook::default();
    let day = NaiveDate::from_ymd_opt(2026, 11, 15).unwrap();
    let at = |h: u32| day.and_time(NaiveTime::from_hms_opt(h, 0, 0).unwrap());
    let local = book.add_with_duration(Some("Yerel Toplantı"), day, NaiveTime::from_hms_opt(10, 0, 0).unwrap(), 30);
    let local = local.expect("yerel etkinlik eklenmeli");

    book.replace_external(
        CalendarEventSource::Google,
        &[CalendarEvent::with_source("google:1", "Google Sync", at(9), 45, CalendarEventSource::Google)],
    );
    book.replace_external(
        CalendarEventSource::Outlook,
        &[CalendarEvent::with_source("outlook:1", "Outlook Sync", at(11), 60, CalendarEventSource::Outlook)],
    );

    assert_eq!(3, book.events().len());
    assert_eq!("google:1", book.events()[0].id);
    assert_eq!(local.id, book.events()[1].id);
    assert_eq!("outlook:1", book.events()[2].id);

    // Yerel listede yalnız yerel etkinlikler kalır
    assert_eq!(1, book.local_events().len());

    // Harici etkinlik silinemez
    assert!(!book.remove("google:1"));
    assert_eq!(3, book.events().len());

    // Yerel etkinlik silinebilir
    assert!(book.remove(&local.id));
    assert_eq!(2, book.events().len());

    // Sağlayıcı bağlantısı kesilince yalnız o sağlayıcı temizlenir
    assert!(book.clear_external(Some(CalendarEventSource::Google)));
    assert_eq!(1, book.events().len());
    assert_eq!(CalendarEventSource::Outlook, book.events()[0].source);
}

#[test]
fn parsers_parse_google_and_outlook_responses_skipping_cancelled() {
    let google_json = r#"
        {
          "items": [
            {
              "id": "g1",
              "summary": "Standup",
              "status": "confirmed",
              "start": { "dateTime": "2026-11-15T09:00:00Z" },
              "end": { "dateTime": "2026-11-15T09:30:00Z" }
            },
            {
              "id": "g2",
              "summary": "Cancelled Event",
              "status": "cancelled",
              "start": { "date": "2026-11-16" },
              "end": { "date": "2026-11-17" }
            }
          ]
        }
        "#;

    let google_events = CalendarEventParsers::parse_google_events(google_json, Some(&TimeZoneInfo::utc())).unwrap();
    assert_eq!(1, google_events.len());
    assert_eq!("google:g1", google_events[0].id);
    assert_eq!("Standup", google_events[0].title);
    assert_eq!(30, google_events[0].duration_minutes);
    assert_eq!(CalendarEventSource::Google, google_events[0].source);

    let outlook_json = r#"
        {
          "value": [
            {
              "id": "o1",
              "subject": "Design Review",
              "isCancelled": false,
              "isAllDay": false,
              "start": { "dateTime": "2026-11-15T14:00:00.0000000", "timeZone": "UTC" },
              "end": { "dateTime": "2026-11-15T15:00:00.0000000", "timeZone": "UTC" }
            },
            {
              "id": "o2",
              "subject": "Cancelled",
              "isCancelled": true,
              "isAllDay": false,
              "start": { "dateTime": "2026-11-15T16:00:00.0000000", "timeZone": "UTC" },
              "end": { "dateTime": "2026-11-15T17:00:00.0000000", "timeZone": "UTC" }
            }
          ]
        }
        "#;

    let outlook_events = CalendarEventParsers::parse_outlook_events(outlook_json, Some(&TimeZoneInfo::utc())).unwrap();
    assert_eq!(1, outlook_events.len());
    assert_eq!("outlook:o1", outlook_events[0].id);
    assert_eq!("Design Review", outlook_events[0].title);
    assert_eq!(60, outlook_events[0].duration_minutes);
    assert_eq!(CalendarEventSource::Outlook, outlook_events[0].source);
}

#[test]
fn parsers_convert_zones_and_handle_all_day_events() {
    // Ek denetim (C# testinde yok): saat dilimi dönüşümü ve tüm gün etkinlikleri.
    let tokyo = TimeZoneInfo::create_custom_time_zone("t", crate::time::TimeSpan::from_hours(9), "t", "t");
    let google_json = r#"{"items":[
        {"id":"a","summary":" Toplantı ","start":{"dateTime":"2026-11-15T09:00:00+01:00"},"end":{"dateTime":"2026-11-15T10:30:00+01:00"}},
        {"id":"b","summary":"Tüm gün","start":{"date":"2026-11-16"},"end":{"date":"2026-11-17"}}]}"#;
    let events = CalendarEventParsers::parse_google_events(google_json, Some(&tokyo)).unwrap();
    assert_eq!("Toplantı", events[0].title);
    assert_eq!(NaiveDate::from_ymd_opt(2026, 11, 15).unwrap().and_hms_opt(17, 0, 0).unwrap(), events[0].start);
    assert_eq!(90, events[0].duration_minutes);
    assert_eq!(NaiveDate::from_ymd_opt(2026, 11, 16).unwrap().and_hms_opt(0, 0, 0).unwrap(), events[1].start);
    assert_eq!(0, events[1].duration_minutes);

    let outlook_json = r#"{"value":[{"id":"o","subject":"S","start":{"dateTime":"2026-11-15T14:00:00.0000000","timeZone":"UTC"},"end":{"dateTime":"2026-11-15T15:00:00.0000000","timeZone":"UTC"}}]}"#;
    let events = CalendarEventParsers::parse_outlook_events(outlook_json, Some(&tokyo)).unwrap();
    assert_eq!(NaiveDate::from_ymd_opt(2026, 11, 15).unwrap().and_hms_opt(23, 0, 0).unwrap(), events[0].start);

    // Hata yolları: bozuk JSON ve nesne olmayan kök istisna fırlatır; boş girdi boş liste.
    assert!(CalendarEventParsers::parse_google_events("{ bozuk", None).is_err());
    assert!(CalendarEventParsers::parse_google_events("[]", None).is_err());
    assert!(CalendarEventParsers::parse_google_events("  ", None).unwrap().is_empty());
    assert!(CalendarEventParsers::parse_outlook_events("{}", None).unwrap().is_empty());
}

#[test]
fn token_response_parsing_follows_the_csharp_defaults() {
    let now = Utc::now().fixed_offset();
    let token = CalendarEventParsers::parse_token_response(r#"{"access_token":"a","refresh_token":"r","expires_in":10}"#, now, "old", "me").unwrap().unwrap();
    assert_eq!("a", token.access_token);
    assert_eq!("r", token.refresh_token);
    assert_eq!(now + Duration::seconds(60), token.expires_at_utc);
    assert_eq!("me", token.account_label);

    let keeps = CalendarEventParsers::parse_token_response(r#"{"access_token":"a"}"#, now, "old", "").unwrap().unwrap();
    assert_eq!("old", keeps.refresh_token);
    assert_eq!(now + Duration::seconds(3600), keeps.expires_at_utc);

    assert!(CalendarEventParsers::parse_token_response(r#"{"error":"x"}"#, now, "", "").unwrap().is_none());
    assert!(CalendarEventParsers::parse_token_response(r#"{"access_token":"  "}"#, now, "", "").unwrap().is_none());
}

#[test]
fn token_store_and_cache_store_round_trip_with_protection() {
    let dir = TempDir(std::env::temp_dir().join(format!("koy-cal-test-{}", crate::calendar::event_book::new_guid_n())));
    std::fs::create_dir_all(&dir.0).unwrap();

    let token_store = CalendarTokenStore::new(Arc::new(FakeProtector), Some(dir.0.clone()));
    let token_set = OAuthTokenSet::new("acc-1", "ref-1", Utc::now().fixed_offset() + Duration::hours(1), "user@example.com");
    token_store.save(CalendarProvider::Google, &token_set).unwrap();

    assert!(token_store.exists(CalendarProvider::Google));
    let raw_bytes = std::fs::read(token_store.get_token_path(CalendarProvider::Google)).unwrap();
    assert!(!String::from_utf8_lossy(&raw_bytes).contains("acc-1"));

    let loaded_token = token_store.load(CalendarProvider::Google).expect("belirteç yüklenmeli");
    assert_eq!("acc-1", loaded_token.access_token);
    assert_eq!("ref-1", loaded_token.refresh_token);
    assert_eq!("user@example.com", loaded_token.account_label);
    assert_eq!(token_set, loaded_token);

    token_store.delete(CalendarProvider::Google);
    assert!(!token_store.exists(CalendarProvider::Google));

    let cache_path = dir.0.join("calendar-cache.json");
    let cache_store = CalendarCacheStore::new(Some(cache_path));
    let snapshot = CalendarCacheSnapshot::new(
        Some(Utc::now().fixed_offset()),
        vec![CalendarEvent::with_source(
            "google:1",
            "Cached Meeting",
            NaiveDate::from_ymd_opt(2026, 11, 15).unwrap().and_hms_opt(10, 0, 0).unwrap(),
            30,
            CalendarEventSource::Google,
        )],
    );
    cache_store.save(&snapshot).unwrap();

    let loaded_cache = cache_store.load();
    assert_eq!(1, loaded_cache.events.len());
    assert_eq!("Cached Meeting", loaded_cache.events[0].title);
    assert!(loaded_cache.last_sync_utc.is_some());

    cache_store.remove_provider(CalendarEventSource::Google).unwrap();
    assert!(cache_store.load().events.is_empty());
}

#[test]
fn settings_store_round_trips_and_falls_back_to_defaults() {
    let dir = TempDir(std::env::temp_dir().join(format!("koy-cal-settings-{}", crate::calendar::event_book::new_guid_n())));
    let path = dir.0.join("calendar-settings.json");
    let store = CalendarSettingsStore::new(Some(path.clone()));
    assert_eq!(CalendarSettings::default(), store.load());

    let settings = CalendarSettings { google_enabled: true, google_client_id: "id".into(), last_sync_unix: 42, ..Default::default() };
    store.save(&settings).unwrap();
    assert_eq!(settings, store.load());

    // C# özellik adları PascalCase; null metin alanları boş metne döner.
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"GoogleEnabled\": true"));
    std::fs::write(&path, r#"{"GoogleClientId":null,"OutlookEnabled":true}"#).unwrap();
    let loaded = store.load();
    assert_eq!("", loaded.google_client_id);
    assert!(loaded.outlook_enabled);

    std::fs::write(&path, "bozuk").unwrap();
    assert_eq!(CalendarSettings::default(), store.load());
}
