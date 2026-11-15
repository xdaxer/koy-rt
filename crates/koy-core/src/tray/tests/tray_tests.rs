use super::new_guid;
use crate::tray::*;

/// UTF-16LE bayt dizisi (`Encoding.Unicode.GetBytes`).
fn unicode_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

/// shell32'nin gönderdiği SHELLTRAYDATA bloğunu (964 bayt) üretir. `length` bloğu kısaltır.
#[allow(clippy::too_many_arguments)]
fn build_shell_tray_data(
    message: u32,
    hwnd: u32,
    id: u32,
    flags: u32,
    callback: u32,
    icon: u32,
    tip: &str,
    state: u32,
    state_mask: u32,
    version: u32,
    guid: Guid,
    length: usize,
) -> Vec<u8> {
    let mut full = vec![0u8; 964];
    full[0..4].copy_from_slice(&TrayMessageParser::SIGNATURE.to_le_bytes());
    write_u32(&mut full, 4, message);
    full[8..12].copy_from_slice(&956i32.to_le_bytes());
    write_u32(&mut full, 12, hwnd);
    write_u32(&mut full, 16, id);
    write_u32(&mut full, 20, flags);
    write_u32(&mut full, 24, callback);
    write_u32(&mut full, 28, icon);
    let tip_bytes = unicode_bytes(tip);
    full[32..32 + tip_bytes.len()].copy_from_slice(&tip_bytes);
    write_u32(&mut full, 288, state);
    write_u32(&mut full, 292, state_mask);
    write_u32(&mut full, 808, version);
    full[944..960].copy_from_slice(&guid.to_bytes());
    let mut data = vec![0u8; length];
    let n = length.min(full.len());
    data[..n].copy_from_slice(&full[..n]);
    data
}

/// C# `BuildShellTrayData(message, hwnd, id, flags)` (kalan parametreler varsayılan).
fn build_basic(message: u32, hwnd: u32, id: u32, flags: u32) -> Vec<u8> {
    build_shell_tray_data(message, hwnd, id, flags, 0, 0, "", 0, 0, 0, Guid::EMPTY, 964)
}

mod tray_message_parser_tests {
    use super::*;

    #[test]
    fn try_parse_reads_all_fields() {
        let guid = new_guid();
        let bytes = build_shell_tray_data(1, 0x1234, 7, 0x2F, 0x8001, 0xABCD, "Discord", 1, 1, 4, guid, 964);

        let data = TrayMessageParser::try_parse(&bytes).expect("çözülmeli");
        assert_eq!(TrayMessage::Modify, data.message);
        assert_eq!(0x1234, data.owner_hwnd);
        assert_eq!(7u32, data.id);
        assert_eq!(
            TrayIconFields::MESSAGE | TrayIconFields::ICON | TrayIconFields::TIP | TrayIconFields::STATE | TrayIconFields::GUID,
            data.fields
        );
        assert_eq!(0x8001u32, data.callback_message);
        assert_eq!(0xABCD, data.icon_handle);
        assert_eq!("Discord", data.tooltip);
        assert_eq!(1u32, data.state);
        assert_eq!(1u32, data.state_mask);
        assert_eq!(4u32, data.version);
        assert_eq!(guid, data.guid);
    }

    #[test]
    fn try_parse_sign_extends_handles_like_windows() {
        let bytes = build_basic(0, 0xFFFF1234, 1, 0);

        let data = TrayMessageParser::try_parse(&bytes).expect("çözülmeli");
        assert_eq!(0xFFFF1234u32 as i32 as isize, data.owner_hwnd);
    }

    #[test]
    fn try_parse_rejects_wrong_signature_short_block_and_unknown_message() {
        let mut wrong_signature = build_basic(0, 1, 1, 0);
        wrong_signature[0..4].copy_from_slice(&0x12345678i32.to_le_bytes());

        assert!(TrayMessageParser::try_parse(&wrong_signature).is_none());
        assert!(TrayMessageParser::try_parse(&build_shell_tray_data(0, 1, 1, 0, 0, 0, "", 0, 0, 0, Guid::EMPTY, 100)).is_none());
        assert!(TrayMessageParser::try_parse(&build_basic(9, 1, 1, 0)).is_none());
    }

    #[test]
    fn try_parse_shorter_block_without_version_and_guid_defaults_them() {
        let bytes = build_shell_tray_data(0, 5, 2, 0x4, 0, 0, "Steam", 0, 0, 4, Guid::EMPTY, TrayMessageParser::MINIMUM_LENGTH as usize);

        let data = TrayMessageParser::try_parse(&bytes).expect("çözülmeli");
        assert_eq!("Steam", data.tooltip);
        assert_eq!(0u32, data.version);
        assert_eq!(Guid::EMPTY, data.guid);
    }

    #[test]
    fn is_acceptable_length_rejects_out_of_range_sizes() {
        let lengths = [
            0,
            -1,
            TrayMessageParser::MINIMUM_LENGTH as i64 - 1,
            TrayMessageParser::MAXIMUM_LENGTH as i64 + 1,
            i32::MAX as i64 + 1, // cbData işaretsiz 32 bit: i32'ye çevrilince negatif olurdu
            u32::MAX as i64,
        ];
        for length in lengths {
            assert!(!TrayMessageParser::is_acceptable_length(length), "{length}");
        }
    }

    #[test]
    fn is_acceptable_length_accepts_real_block_sizes() {
        for length in [TrayMessageParser::MINIMUM_LENGTH as i64, 964, TrayMessageParser::MAXIMUM_LENGTH as i64] {
            assert!(TrayMessageParser::is_acceptable_length(length), "{length}");
        }
    }

    #[test]
    fn try_parse_rejects_oversized_block_even_with_valid_signature() {
        let bytes = build_shell_tray_data(0, 1, 1, 0, 0, 0, "", 0, 0, 0, Guid::EMPTY, TrayMessageParser::MAXIMUM_LENGTH as usize + 1);

        assert!(TrayMessageParser::try_parse(&bytes).is_none());
    }

    #[test]
    fn try_parse_never_throws_on_truncated_or_garbage_blocks() {
        // Başka süreçler Koy'un penceresine her türlü veriyi gönderebilir: hiçbir uzunluk panik etmemeli.
        let valid = build_shell_tray_data(0, 1, 1, 0x2F, 0, 0, "X", 0, 0, 0, Guid::EMPTY, 964);
        for length in 0..=valid.len() {
            let _ = TrayMessageParser::try_parse(&valid[..length]);
        }

        // `new Random(1234)` yerine küçük bir xorshift (tohumlu, tekrarlanabilir).
        let mut x: u64 = 1234 | 1;
        let mut garbage = [0u8; 2048];
        for round in 0..200 {
            for b in garbage.iter_mut() {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                *b = (x >> 24) as u8;
            }
            if round % 2 == 0 {
                garbage[0..4].copy_from_slice(&TrayMessageParser::SIGNATURE.to_le_bytes());
            }
            let _ = TrayMessageParser::try_parse(&garbage);
        }
    }

    #[test]
    fn try_parse_tooltip_without_terminator_is_truncated_to_field_size() {
        let mut bytes = build_shell_tray_data(0, 1, 1, 0x4, 0, 0, &"A".repeat(127), 0, 0, 0, Guid::EMPTY, 964);
        bytes[32 + 127 * 2..32 + 127 * 2 + 2].copy_from_slice(&('B' as u16).to_le_bytes()); // sonlandırıcı yok: alan tam dolu

        let data = TrayMessageParser::try_parse(&bytes).expect("çözülmeli");
        assert_eq!(128, data.tooltip.chars().count());
    }
}

mod tray_icon_store_tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn msg_full(
        message: TrayMessage,
        hwnd: isize,
        id: u32,
        fields: TrayIconFields,
        callback: u32,
        icon: isize,
        tip: &str,
        state: u32,
        mask: u32,
        version: u32,
        guid: Guid,
    ) -> TrayNotifyData {
        TrayNotifyData::new(message, hwnd, id, fields, callback, icon, tip, state, mask, version, guid)
    }

    const DEFAULT_FIELDS: TrayIconFields = TrayIconFields(TrayIconFields::MESSAGE.0 | TrayIconFields::ICON.0 | TrayIconFields::TIP.0);

    /// C# `Msg(message, hwnd: 100, id: 1, fields: Message|Icon|Tip, callback: 0x8001, icon: 500, tip: "App", ...)`.
    fn msg(message: TrayMessage) -> TrayNotifyData {
        msg_full(message, 100, 1, DEFAULT_FIELDS, 0x8001, 500, "App", 0, 0, 0, Guid::EMPTY)
    }

    fn tooltips(store: &TrayIconStore) -> Vec<String> {
        store.visible().iter().map(|i| i.tooltip.clone()).collect()
    }

    #[test]
    fn add_keeps_insertion_order() {
        let mut store = TrayIconStore::new();
        store.apply(&TrayNotifyData { owner_hwnd: 1, icon_handle: 11, tooltip: "A".into(), ..msg(TrayMessage::Add) });
        store.apply(&TrayNotifyData { owner_hwnd: 2, icon_handle: 22, tooltip: "B".into(), ..msg(TrayMessage::Add) });
        store.apply(&TrayNotifyData { owner_hwnd: 3, icon_handle: 33, tooltip: "C".into(), ..msg(TrayMessage::Add) });

        assert_eq!(vec!["A", "B", "C"], tooltips(&store));
    }

    #[test]
    fn add_beyond_limit_rejects_new_icon_and_returns_its_handle() {
        let mut store = TrayIconStore::new();
        for i in 1..=TrayIconStore::MAX_ICONS as isize {
            assert!(store.apply(&TrayNotifyData { owner_hwnd: i, icon_handle: 1000 + i, ..msg(TrayMessage::Add) }).success);
        }

        let result = store.apply(&TrayNotifyData { owner_hwnd: TrayIconStore::MAX_ICONS as isize + 1, icon_handle: 99999, ..msg(TrayMessage::Add) });

        assert!(!result.success);
        assert_eq!(vec![99999isize], result.released_icons); // çağıran kopyayı yok edebilsin (sızıntı olmasın)
        assert_eq!(TrayIconStore::MAX_ICONS, store.all().len());
    }

    #[test]
    fn add_at_limit_still_allows_updating_and_removing_existing_icons() {
        let mut store = TrayIconStore::new();
        for i in 1..=TrayIconStore::MAX_ICONS as isize {
            store.apply(&TrayNotifyData { owner_hwnd: i, icon_handle: 1000 + i, ..msg(TrayMessage::Add) });
        }

        assert!(store.apply(&TrayNotifyData { owner_hwnd: 5, icon_handle: 7777, tooltip: "Yeni".into(), ..msg(TrayMessage::Add) }).success);
        assert!(store.apply(&TrayNotifyData { owner_hwnd: 6, fields: TrayIconFields::NONE, ..msg(TrayMessage::Delete) }).success);
        assert!(store.apply(&TrayNotifyData { owner_hwnd: 9000, icon_handle: 8888, ..msg(TrayMessage::Add) }).success);
    }

    #[test]
    fn add_existing_icon_updates_in_place_and_releases_old_icon() {
        let mut store = TrayIconStore::new();
        store.apply(&TrayNotifyData { owner_hwnd: 1, icon_handle: 11, tooltip: "A".into(), ..msg(TrayMessage::Add) });
        store.apply(&TrayNotifyData { owner_hwnd: 2, icon_handle: 22, tooltip: "B".into(), ..msg(TrayMessage::Add) });

        let result = store.apply(&TrayNotifyData { owner_hwnd: 1, icon_handle: 99, tooltip: "A2".into(), ..msg(TrayMessage::Add) });

        assert!(result.success);
        assert_eq!(vec![11isize], result.released_icons);
        assert_eq!(vec!["A2", "B"], tooltips(&store));
        assert_eq!(99, store.visible()[0].icon_handle);
    }

    #[test]
    fn modify_only_changes_flagged_fields() {
        let mut store = TrayIconStore::new();
        store.apply(&TrayNotifyData { icon_handle: 11, tooltip: "Eski".into(), ..msg(TrayMessage::Add) });

        let result = store.apply(&TrayNotifyData { fields: TrayIconFields::TIP, icon_handle: 77, tooltip: "Yeni".into(), ..msg(TrayMessage::Modify) });

        assert!(result.success);
        assert!(result.released_icons.is_empty());
        let visible = store.visible();
        assert_eq!(1, visible.len());
        let icon = &visible[0];
        assert_eq!("Yeni", icon.tooltip);
        assert_eq!(11, icon.icon_handle);
        assert_eq!(0x8001u32, icon.callback_message);
    }

    #[test]
    fn modify_unknown_icon_fails_and_returns_incoming_copy() {
        let mut store = TrayIconStore::new();

        let result = store.apply(&TrayNotifyData { icon_handle: 44, ..msg(TrayMessage::Modify) });

        assert!(!result.success);
        assert_eq!(vec![44isize], result.released_icons);
        assert!(store.all().is_empty());
    }

    #[test]
    fn delete_removes_and_releases_icon() {
        let mut store = TrayIconStore::new();
        store.apply(&TrayNotifyData { icon_handle: 11, ..msg(TrayMessage::Add) });

        let result = store.apply(&TrayNotifyData { fields: TrayIconFields::NONE, ..msg(TrayMessage::Delete) });

        assert!(result.success);
        assert_eq!(vec![11isize], result.released_icons);
        assert!(store.all().is_empty());
        assert!(!store.apply(&TrayNotifyData { fields: TrayIconFields::NONE, ..msg(TrayMessage::Delete) }).success);
    }

    #[test]
    fn state_hidden_flag_hides_and_shows_icon() {
        let mut store = TrayIconStore::new();
        let state_fields = TrayIconFields::MESSAGE | TrayIconFields::ICON | TrayIconFields::STATE;
        store.apply(&TrayNotifyData { fields: state_fields, state: 1, state_mask: 1, ..msg(TrayMessage::Add) });
        assert!(store.visible().is_empty());
        assert_eq!(1, store.all().len());

        // Maske gizleme bitini içermiyorsa durum değişmez.
        store.apply(&TrayNotifyData { fields: TrayIconFields::STATE, state: 0, state_mask: 0, ..msg(TrayMessage::Modify) });
        assert!(store.visible().is_empty());

        store.apply(&TrayNotifyData { fields: TrayIconFields::STATE, state: 0, state_mask: 1, ..msg(TrayMessage::Modify) });
        assert_eq!(1, store.visible().len());
    }

    #[test]
    fn icon_without_image_is_not_visible() {
        let mut store = TrayIconStore::new();
        store.apply(&TrayNotifyData { fields: TrayIconFields::MESSAGE | TrayIconFields::TIP, icon_handle: 0, ..msg(TrayMessage::Add) });

        assert_eq!(1, store.all().len());
        assert!(store.visible().is_empty());
    }

    #[test]
    fn set_version_stores_version() {
        let mut store = TrayIconStore::new();
        store.apply(&msg(TrayMessage::Add));

        assert!(store.apply(&TrayNotifyData { fields: TrayIconFields::NONE, version: 4, ..msg(TrayMessage::SetVersion) }).success);
        assert_eq!(4u32, store.all()[0].version);
    }

    #[test]
    fn guid_icon_is_identified_by_guid_even_if_window_changes() {
        let guid = new_guid();
        let mut store = TrayIconStore::new();
        let guid_fields = TrayIconFields::MESSAGE | TrayIconFields::ICON | TrayIconFields::GUID;
        store.apply(&TrayNotifyData { owner_hwnd: 1, fields: guid_fields, guid, icon_handle: 11, ..msg(TrayMessage::Add) });

        store.apply(&TrayNotifyData {
            owner_hwnd: 2,
            fields: TrayIconFields::ICON | TrayIconFields::GUID,
            guid,
            icon_handle: 22,
            ..msg(TrayMessage::Modify)
        });

        assert_eq!(1, store.all().len());
        let icon = &store.all()[0];
        assert_eq!(2, icon.owner_hwnd);
        assert_eq!(22, icon.icon_handle);
    }

    #[test]
    fn remove_dead_drops_icons_of_closed_windows() {
        let mut store = TrayIconStore::new();
        store.apply(&TrayNotifyData { owner_hwnd: 1, icon_handle: 11, ..msg(TrayMessage::Add) });
        store.apply(&TrayNotifyData { owner_hwnd: 2, icon_handle: 22, ..msg(TrayMessage::Add) });

        let released = store.remove_dead(|hwnd| hwnd == 2);

        assert_eq!(vec![11isize], released);
        assert_eq!(1, store.all().len());
        assert_eq!(2, store.all()[0].owner_hwnd);
    }

    #[test]
    fn clear_releases_every_icon() {
        let mut store = TrayIconStore::new();
        store.apply(&TrayNotifyData { owner_hwnd: 1, icon_handle: 11, ..msg(TrayMessage::Add) });
        store.apply(&TrayNotifyData { owner_hwnd: 2, icon_handle: 22, ..msg(TrayMessage::Add) });

        assert_eq!(vec![11isize, 22], store.clear());
        assert!(store.all().is_empty());
    }
}

mod tray_callback_tests {
    use super::*;

    fn icon(version: u32, id: u32) -> TrayIcon {
        TrayIcon {
            key: TrayIconKey::new(100, id, Guid::EMPTY),
            owner_hwnd: 100,
            id,
            callback_message: 0x8001,
            icon_handle: 1,
            tooltip: "App".into(),
            is_hidden: false,
            version,
        }
    }

    #[test]
    fn version4_left_click_sends_down_up_select_with_cursor_and_id() {
        let messages = TrayCallback::build(&icon(4, 3), TrayMouseAction::LeftClick, 1500, 1040);

        assert_eq!(3, messages.len());
        for m in &messages {
            assert_eq!(0x8001u32, m.message);
            assert_eq!(((1040 << 16) | 1500) as isize, m.w_param);
        }
        assert_eq!(((3 << 16) | 0x0201) as isize, messages[0].l_param);
        assert_eq!(((3 << 16) | 0x0202) as isize, messages[1].l_param);
        assert_eq!(((3 << 16) | 0x0400) as isize, messages[2].l_param);
    }

    #[test]
    fn version4_right_click_ends_with_context_menu() {
        let messages = TrayCallback::build(&icon(4, 3), TrayMouseAction::RightClick, 10, 20);

        assert_eq!(vec![0x0204, 0x0205, 0x007B], messages.iter().map(|m| (m.l_param & 0xFFFF) as i32).collect::<Vec<_>>());
    }

    #[test]
    fn legacy_version_sends_id_and_plain_mouse_message() {
        let messages = TrayCallback::build(&icon(0, 9), TrayMouseAction::LeftClick, 10, 20);

        assert_eq!(2, messages.len());
        for m in &messages {
            assert_eq!(9, m.w_param);
        }
        assert_eq!(vec![0x0201isize, 0x0202], messages.iter().map(|m| m.l_param).collect::<Vec<_>>());
    }

    #[test]
    fn double_click_sends_double_click_then_up() {
        let messages = TrayCallback::build(&icon(0, 3), TrayMouseAction::LeftDoubleClick, 0, 0);

        assert_eq!(vec![0x0203isize, 0x0202], messages.iter().map(|m| m.l_param).collect::<Vec<_>>());
    }

    #[test]
    fn negative_cursor_on_left_monitor_is_packed_as_signed_words() {
        let messages = TrayCallback::build(&icon(4, 3), TrayMouseAction::LeftClick, -100, 50);

        let w = messages[0].w_param as u32;
        assert_eq!(-100, (w & 0xFFFF) as u16 as i16);
        assert_eq!(50, (w >> 16) as u16 as i16);
    }

    #[test]
    fn icon_without_callback_sends_nothing() {
        let icon = TrayIcon { callback_message: 0, ..icon(4, 3) };

        assert!(TrayCallback::build(&icon, TrayMouseAction::LeftClick, 0, 0).is_empty());
    }
}
