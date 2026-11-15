use std::sync::Arc;

use crate::time::TimeSpan;
use crate::time::testing::FakeClock;
use crate::tray::*;

/// C# `BuildBalloonData(...)` varsayılanlarıyla (message=Modify, hwnd=0x1234, id=7, flags=Info, length=964).
struct BalloonData {
    message: u32,
    hwnd: u32,
    id: u32,
    flags: TrayIconFields,
    info: &'static str,
    info_title: &'static str,
    info_flags: TrayBalloonFlags,
    timeout_or_version: u32,
    balloon_icon: u32,
    guid: Guid,
    length: usize,
}

impl Default for BalloonData {
    fn default() -> Self {
        BalloonData {
            message: TrayMessage::Modify as u32,
            hwnd: 0x1234,
            id: 7,
            flags: TrayIconFields::INFO,
            info: "",
            info_title: "",
            info_flags: TrayBalloonFlags::NONE,
            timeout_or_version: 0,
            balloon_icon: 0,
            guid: Guid::EMPTY,
            length: 964,
        }
    }
}

fn unicode_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn build_balloon_data(d: BalloonData) -> Vec<u8> {
    let mut full = vec![0u8; 964];
    full[0..4].copy_from_slice(&TrayMessageParser::SIGNATURE.to_le_bytes());
    write_u32(&mut full, 4, d.message);
    full[8..12].copy_from_slice(&956i32.to_le_bytes());
    write_u32(&mut full, 12, d.hwnd);
    write_u32(&mut full, 16, d.id);
    write_u32(&mut full, 20, d.flags.0);
    let info = unicode_bytes(d.info);
    full[296..296 + info.len()].copy_from_slice(&info);
    write_u32(&mut full, 808, d.timeout_or_version);
    let title = unicode_bytes(d.info_title);
    full[812..812 + title.len()].copy_from_slice(&title);
    write_u32(&mut full, 940, d.info_flags.0);
    full[944..960].copy_from_slice(&d.guid.to_bytes());
    write_u32(&mut full, 960, d.balloon_icon);

    let mut data = vec![0u8; d.length];
    let n = d.length.min(full.len());
    data[..n].copy_from_slice(&full[..n]);
    data
}

fn notify(message: TrayMessage, fields: TrayIconFields, tooltip: &str, info: &str, info_title: &str) -> TrayNotifyData {
    TrayNotifyData {
        info: info.to_owned(),
        info_title: info_title.to_owned(),
        ..TrayNotifyData::new(message, 10, 1, fields, 0, 0, tooltip, 0, 0, 0, Guid::EMPTY)
    }
}

#[test]
fn try_parse_reads_balloon_fields_when_info_flag_set() {
    let bytes = build_balloon_data(BalloonData {
        flags: TrayIconFields::INFO | TrayIconFields::TIP,
        info: "Yedekleme tamamlandı",
        info_title: "Bulut Sürücü",
        info_flags: TrayBalloonFlags::USER | TrayBalloonFlags::NO_SOUND,
        timeout_or_version: 15000,
        balloon_icon: 0xBEEF,
        ..Default::default()
    });

    let data = TrayMessageParser::try_parse(&bytes).expect("çözülmeli");
    assert_eq!("Yedekleme tamamlandı", data.info);
    assert_eq!("Bulut Sürücü", data.info_title);
    assert_eq!(TrayBalloonFlags::USER | TrayBalloonFlags::NO_SOUND, data.info_flags);
    assert_eq!(15000u32, data.version);
    assert_eq!(0xBEEF, data.balloon_icon_handle);
}

#[test]
fn try_parse_ignores_balloon_fields_when_info_flag_not_set() {
    let bytes = build_balloon_data(BalloonData {
        flags: TrayIconFields::TIP,
        info: "Gizli kalmalı",
        info_title: "Başlık",
        info_flags: TrayBalloonFlags::WARNING,
        balloon_icon: 0x1234,
        ..Default::default()
    });

    let data = TrayMessageParser::try_parse(&bytes).expect("çözülmeli");
    assert_eq!("", data.info);
    assert_eq!("", data.info_title);
    assert_eq!(TrayBalloonFlags::NONE, data.info_flags);
    assert_eq!(0, data.balloon_icon_handle);
}

#[test]
fn try_parse_shorter_block_with_info_flag_does_not_throw_and_defaults_missing_tail() {
    // 944 bayt: szInfo (296..808), uTimeout (808..812), szInfoTitle (812..940), dwInfoFlags (940..944) var,
    // ancak guidItem (944..960) ve hBalloonIcon (960..964) yok.
    let bytes = build_balloon_data(BalloonData {
        flags: TrayIconFields::INFO,
        info: "Kısa blok",
        info_title: "Başlık",
        info_flags: TrayBalloonFlags::ERROR,
        balloon_icon: 0x9999,
        length: 944,
        ..Default::default()
    });

    let data = TrayMessageParser::try_parse(&bytes).expect("çözülmeli");
    assert_eq!("Kısa blok", data.info);
    assert_eq!("Başlık", data.info_title);
    assert_eq!(TrayBalloonFlags::ERROR, data.info_flags);
    assert_eq!(0, data.balloon_icon_handle);
}

#[test]
fn filter_rejects_non_info_and_empty_dismiss_messages() {
    let mut filter = TrayBalloonFilter::new(Some(Arc::new(FakeClock::new())));

    let no_flag = notify(TrayMessage::Modify, TrayIconFields::TIP, "App", "Metin", "Başlık");
    assert!(filter.try_accept(&no_flag).is_none());

    // Win32 uygulamaları balonu kapatmak için boş szInfo gönderir.
    let empty_dismiss = notify(TrayMessage::Modify, TrayIconFields::INFO, "App", "   ", "");
    assert!(filter.try_accept(&empty_dismiss).is_none());

    let delete_msg = notify(TrayMessage::Delete, TrayIconFields::INFO, "App", "Metin", "Başlık");
    assert!(filter.try_accept(&delete_msg).is_none());
}

#[test]
fn filter_deduplicates_within_cooldown_and_allows_after_cooldown() {
    let clock = Arc::new(FakeClock::new());
    let mut filter = TrayBalloonFilter::new(Some(clock.clone()));

    let msg = notify(TrayMessage::Modify, TrayIconFields::INFO, "App", "  Bağlantı kesildi  ", "  VPN  ");

    let (title, body, _) = filter.try_accept(&msg).expect("kabul edilmeli");
    assert_eq!("VPN", title);
    assert_eq!("Bağlantı kesildi", body);

    // Aynı içerik bekleme süresi içinde reddedilir.
    clock.advance(TimeSpan::from_secs(2));
    assert!(filter.try_accept(&msg).is_none());

    // Farklı içerik aynı simgeden hemen kabul edilir.
    let different = TrayNotifyData { info: "Yeniden bağlandı".to_owned(), ..msg.clone() };
    assert!(filter.try_accept(&different).is_some());

    // Bekleme süresi dolunca aynı içerik yeniden kabul edilir.
    clock.advance(TrayBalloonFilter::DUPLICATE_COOLDOWN);
    assert!(filter.try_accept(&different).is_some());
}
