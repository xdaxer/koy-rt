//! Kontrol Merkezi durumunu Slint yapısına çevirir ve sabit metinleri kurar.

use slint::{ComponentHandle, ModelRc, VecModel};

use koy_core::bluetooth::BluetoothDeviceKind;

use super::control_center_view_model::{ControlCenterViewModel, ControlPage, RadioTile};
use crate::resources::strings::{self, Strings};
use crate::{BluetoothRowData, ConnectedDeviceData, ControlData, ControlText, RadioTileData, WifiRowData};

fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

fn tile(t: &RadioTile) -> RadioTileData {
    RadioTileData { on: t.is_on, unavailable: t.is_unavailable, status: t.status_text.clone().into() }
}

fn kind_index(k: BluetoothDeviceKind) -> i32 {
    match k {
        BluetoothDeviceKind::Headphones => 1,
        BluetoothDeviceKind::Speaker => 2,
        BluetoothDeviceKind::Mouse => 3,
        BluetoothDeviceKind::Keyboard => 4,
        BluetoothDeviceKind::Phone => 5,
        BluetoothDeviceKind::Tablet => 6,
        #[allow(unreachable_patterns)]
        _ => 0,
    }
}

pub fn to_data(vm: &ControlCenterViewModel) -> ControlData {
    let s = vm.state();
    let wp = vm.wifi_page.page();
    let devices = vm.bluetooth_devices.devices();
    ControlData {
        page: match s.page {
            ControlPage::Main => 0,
            ControlPage::WiFi => 1,
            ControlPage::Bluetooth => 2,
        },
        tile_order: model(s.tile_order.iter().map(|t| slint::SharedString::from(t.as_str())).collect()),
        wifi: tile(&s.wifi),
        bluetooth: tile(&s.bluetooth),
        dnd_status: s.do_not_disturb_status.clone().into(),
        volume_percent: s.volume_percent,
        is_muted: s.is_muted,
        brightness: s.brightness,
        brightness_supported: s.is_brightness_supported,
        brightness_resolved: s.is_brightness_resolved,
        show_connected_card: s.show_connected_card,
        connected: model(
            devices
                .iter()
                .filter(|d| d.is_connected)
                .map(|d| ConnectedDeviceData { name: d.name.clone().into(), battery: d.battery_text.clone().unwrap_or_default().into() })
                .collect(),
        ),
        show_connected_empty: s.show_connected_empty,
        connected_empty_text: s.connected_empty_text.clone().into(),
        wifi_show_list: wp.show_list,
        wifi_rows: model(
            vm.wifi_page
                .rows()
                .iter()
                .map(|r| WifiRowData {
                    ssid: r.ssid.clone().into(),
                    name: r.display_name.clone().into(),
                    bars: r.bars,
                    secured: r.is_secured,
                    connected: r.is_connected,
                    has_status: r.has_status(),
                    status: r.display_status().into(),
                    error: r.error_text.as_deref().is_some_and(|e| !e.is_empty()),
                    busy: r.is_busy,
                    password_open: r.is_password_open,
                    can_connect: r.can_connect,
                    clickable: r.is_clickable,
                })
                .collect(),
        ),
        wifi_message: wp.message_text.clone().into(),
        wifi_show_location_link: wp.show_location_link,
        bt_show_list: s.show_bluetooth_list,
        bt_rows: model(
            devices
                .iter()
                .map(|r| BluetoothRowData {
                    id: r.id.clone().into(),
                    name: r.name.clone().into(),
                    kind: kind_index(r.kind),
                    connected: r.is_connected,
                    is_audio: r.is_audio,
                    battery: r.battery_text.clone().unwrap_or_default().into(),
                    status: r.status_text.clone().into(),
                    hint: r.hint_text.clone().unwrap_or_default().into(),
                    error: r.error_text.as_deref().is_some_and(|e| !e.is_empty()),
                    busy: r.is_busy,
                    action: r.action_text.clone().into(),
                })
                .collect(),
        ),
        bt_message: s.bluetooth_message.clone().into(),
    }
}

pub fn install_text<C: ComponentHandle>(c: &C)
where
    for<'a> ControlText<'a>: slint::Global<'a, C>,
{
    let t = c.global::<ControlText>();
    let g = |k: &str| slint::SharedString::from(Strings::get(k));
    t.set_wifi(g("WiFi"));
    t.set_bluetooth(g("Bluetooth"));
    t.set_dnd(g("DoNotDisturb"));
    t.set_volume(g("ControlVolume"));
    t.set_brightness(g("ControlBrightness"));
    t.set_brightness_unavailable(g("BrightnessUnavailable"));
    t.set_connected_devices(g("BluetoothConnectedDevices"));
    t.set_wifi_disconnect(g("WifiDisconnect"));
    t.set_wifi_connect(g("WifiConnect"));
    t.set_wifi_password_hint(g("WifiPasswordHint"));
    t.set_wifi_location_link(g("WifiLocationLink"));
    t.set_wifi_more_settings(g("WifiMoreSettings"));
    t.set_bt_more_settings(g("BluetoothMoreSettings"));
    // Yüzde yazımı: Türkçede "%43", İngilizcede "43%".
    t.set_percent_format(if strings::Strings::is_turkish() { "tr".into() } else { "en".into() });
}
