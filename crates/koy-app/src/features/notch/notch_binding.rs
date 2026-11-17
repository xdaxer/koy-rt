//! Çentik durumunu (`NotchViewModel`) Slint'in `NotchData` yapısına çevirir.

use slint::{Color, ModelRc, SharedString, VecModel};

use koy_core::notch::{NotchPanel, NotchState};

use super::notch_view_model::{NotchTab, NotchViewModel};
use crate::{NotchData, NotchTabData};

pub fn state_index(s: NotchState) -> i32 {
    match s {
        NotchState::Collapsed => 0,
        NotchState::Peek => 1,
        NotchState::Expanded => 2,
    }
}

pub fn panel_index(p: NotchPanel) -> i32 {
    match p {
        NotchPanel::Home => 0,
        NotchPanel::Focus => 1,
        NotchPanel::Calendar => 2,
        NotchPanel::Shelf => 3,
        NotchPanel::Clipboard => 4,
        NotchPanel::System => 5,
        NotchPanel::ControlCenter => 6,
    }
}

pub fn panel_from_index(i: i32) -> NotchPanel {
    match i {
        1 => NotchPanel::Focus,
        2 => NotchPanel::Calendar,
        3 => NotchPanel::Shelf,
        4 => NotchPanel::Clipboard,
        5 => NotchPanel::System,
        6 => NotchPanel::ControlCenter,
        _ => NotchPanel::Home,
    }
}

fn tab(t: &NotchTab) -> NotchTabData {
    NotchTabData {
        panel: panel_index(t.panel),
        label: t.label.clone().into(),
        icon: t.icon.into(),
        accent: t.accent.into(),
        selected: t.is_selected,
    }
}

pub fn to_data(vm: &NotchViewModel) -> NotchData {
    let music = vm.music.state();
    let status = vm.status.state();
    let cc = vm.control_center.state();
    let st = vm.state();
    NotchData {
        state: state_index(st.state),
        current_panel: panel_index(st.current_panel),
        surface: Color::from_argb_encoded(st.surface),
        has_gradient: st.surface_gradient.len() >= 2,
        g1: gradient_at(&st.surface_gradient, 0.0),
        g2: gradient_at(&st.surface_gradient, 1.0 / 3.0),
        g3: gradient_at(&st.surface_gradient, 2.0 / 3.0),
        g4: gradient_at(&st.surface_gradient, 1.0),
        show_collapsed_layout: vm.show_collapsed_layout(),
        show_peek_layout: vm.show_peek_layout(),
        osd_active: vm.osd.is_active(),
        search_open: vm.is_search_open(),
        is_music_playing: vm.is_music_playing(),
        show_idle_classic_clock: vm.show_idle_classic_clock(),
        clock_time: st.clock_time.clone().into(),
        clock_extras: vm.clock_extras().into(),
        show_idle_timer: vm.show_idle_timer(),
        idle_timer_text: vm.idle_timer_text().into(),
        is_idle_ring: vm.is_idle_ring(),
        show_idle_ring: vm.show_idle_ring(),
        battery_ring_path: st.battery_ring_path.clone().into(),
        battery_ring_color: Color::from_argb_encoded(vm.battery_ring_color()),
        battery_percent_text: st.battery_percent_text.clone().into(),
        is_charging: st.is_charging,
        idle_day_time: st.idle_day_time.clone().into(),
        idle_peek_date: st.idle_peek_date.clone().into(),
        show_idle_peek_date: vm.show_idle_peek_date(),
        show_idle_battery: vm.show_idle_battery(),
        battery_fill_width: st.battery_fill_width as f32,
        has_battery: st.has_battery,
        battery_percent: st.battery_percent,
        show_idle_radio: vm.show_idle_radio(),
        wifi_present: cc.wifi.is_present,
        wifi_on: cc.wifi.is_on,
        bluetooth_present: cc.bluetooth.is_present,
        bluetooth_on: cc.bluetooth.is_on,
        has_privacy_activity: vm.has_privacy_activity(),
        camera_active: st.is_camera_active,
        microphone_active: st.is_microphone_active,
        music_title: music.title.clone().into(),
        music_artist: music.artist.clone().into(),
        music_has_track: music.has_track,
        has_thumbnail: music.thumbnail.is_some(),
        thumbnail: music.thumbnail.clone().unwrap_or_default(),
        show_music_waveform: vm.show_music_waveform(),
        waveform_bars: ModelRc::new(VecModel::from(vm.collapsed_bar_heights())),
        waveform_color: Color::from_argb_encoded(vm.waveform_color()),
        status_time: SharedString::from(status.current_time.as_str()),
        show_weather: vm.home.show_weather(),
        show_no_location: vm.home.show_no_location(),
        weather_icon: status.weather_icon_kind.clone().into(),
        temperature_short: status.temperature_short.clone().into(),
        is_music_page_selected: vm.is_music_page_selected(),
        is_home_page_selected: vm.is_home_page_selected(),
        show_tab_bar: vm.show_tab_bar(),
        side_tabs: ModelRc::new(VecModel::from(vm.side_tabs().iter().map(tab).collect::<Vec<_>>())),
        is_pinned: vm.is_pinned(),
    }
}

/// n duraklı gradyanın `pos` (0..1) noktasındaki rengi (Slint tarafı dört durakla çizer).
fn gradient_at(stops: &[u32], pos: f32) -> Color {
    match stops.len() {
        0 => Color::default(),
        1 => Color::from_argb_encoded(stops[0]),
        n => {
            let seg = pos.clamp(0.0, 1.0) * (n - 1) as f32;
            let i = (seg.floor() as usize).min(n - 2);
            let t = seg - i as f32;
            let (a, b) = (Color::from_argb_encoded(stops[i]), Color::from_argb_encoded(stops[i + 1]));
            let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
            Color::from_argb_u8(m(a.alpha(), b.alpha()), m(a.red(), b.red()), m(a.green(), b.green()), m(a.blue(), b.blue()))
        }
    }
}
