//! Gösterge durumunu Slint yapısına çevirir.

use koy_core::notch::NotchCardKind;
use koy_core::power::BatteryAlertKind;

use super::osd_indicator_view_model::{OsdState, OsdType};
use crate::OsdData;

pub fn kind_index(k: OsdType) -> i32 {
    match k {
        OsdType::None => 0,
        OsdType::Volume => 1,
        OsdType::Brightness => 2,
        OsdType::Battery => 3,
        OsdType::Bluetooth => 4,
        OsdType::Voice => 5,
        OsdType::Card => 6,
    }
}

pub fn to_data(s: &OsdState) -> OsdData {
    let card = s.card.as_ref();
    OsdData {
        kind: kind_index(s.kind),
        percent: s.percent,
        is_muted: s.is_muted,
        percent_text: s.percent_text.clone().into(),
        battery_kind: match s.battery_kind {
            BatteryAlertKind::PluggedIn => 1,
            BatteryAlertKind::Unplugged => 2,
            BatteryAlertKind::LowBattery => 3,
            BatteryAlertKind::FullyCharged => 4,
            _ => 0,
        },
        battery_status_text: s.battery_status_text.clone().into(),
        bluetooth_device_name: s.bluetooth_device_name.clone().into(),
        bluetooth_connected: s.bluetooth_connected,
        bluetooth_status_text: s.bluetooth_status_text.clone().into(),
        voice_prompt: s.voice_prompt.clone().into(),
        voice_status_text: s.voice_status_text.clone().into(),
        voice_editing: s.is_voice_editing(),
        voice_input: s.voice_input.clone().into(),
        voice_detail: s.voice_detail.clone().unwrap_or_default().into(),
        card_is_notification: card.is_some_and(|c| c.kind == NotchCardKind::Notification),
        card_header: card.map(|c| c.header.clone()).unwrap_or_default().into(),
        card_title: card.map(|c| c.title.clone()).unwrap_or_default().into(),
        card_subtitle: card.map(|c| c.subtitle.clone()).unwrap_or_default().into(),
        card_action: card.map(|c| c.primary_action_label.clone()).unwrap_or_default().into(),
        has_card_icon: s.card_icon.is_some(),
        card_icon: s.card_icon.clone().unwrap_or_default(),
    }
}
