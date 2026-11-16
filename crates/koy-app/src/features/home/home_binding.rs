//! Ana sayfa ve müzik kartı durumunu Slint yapılarına çevirir ve sabit metinleri kurar.

use slint::{ComponentHandle, ModelRc, VecModel};

use koy_core::bluetooth::BluetoothDeviceKind;
use koy_core::notch::{HomeCard, NotchVisualizerStyle};

use super::home_view_model::{HomeViewModel, UpcomingEventRow};
use crate::features::music::music_view_model::MusicViewModel;
use crate::resources::strings::Strings;
use crate::{HomeData, HomeText, MusicCardData, QuickItemData, UpcomingData};

fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

fn upcoming(i: usize, r: &UpcomingEventRow) -> UpcomingData {
    UpcomingData { index: i as i32, when: r.when.clone().into(), title: r.title.clone().into(), until: r.until.clone().into() }
}

pub fn music_data(m: &MusicViewModel) -> MusicCardData {
    let s = m.state();
    let lyr = &m.lyrics;
    MusicCardData {
        has_track: s.has_track,
        title: s.title.clone().into(),
        artist: s.artist.clone().into(),
        source_app: s.source_app_id.clone().into(),
        is_playing: s.is_playing,
        is_live: s.is_live(),
        has_thumbnail: s.thumbnail.is_some(),
        thumbnail: s.thumbnail.clone().unwrap_or_default(),
        is_wide_cover: s.is_wide_cover,
        progress: s.progress as f32,
        position_text: s.position_text.clone().into(),
        duration_text: s.duration_text.clone().into(),
        has_tint: s.paint.has_tint,
        card: s.paint.card.unwrap_or_default(),
        blend: model(s.paint.blend.clone()),
        sub_text: s.paint.sub_text.unwrap_or_default(),
        has_spectrum_color: s.paint.spectrum.is_some(),
        spectrum_color: s.paint.spectrum.unwrap_or_default(),
        has_backdrop: s.paint.has_backdrop,
        backdrop: s.paint.backdrop.clone().unwrap_or_default(),
        tint_overlay: s.paint.tint_overlay.unwrap_or_default(),
        show_spectrum: s.show_spectrum,
        visualizer_style: match m.visualizer_style {
            NotchVisualizerStyle::Wave => 1,
            NotchVisualizerStyle::Dots => 2,
            _ => 0,
        },
        bar_count: m.visualizer_bar_count,
        levels: model(m.spectrum.levels()),
        lyrics_can_toggle: lyr.can_toggle(),
        lyrics_enabled: lyr.is_enabled(),
        lyrics_shown: lyr.is_shown(),
        lyrics: model(lyr.lines().iter().map(|l| slint::SharedString::from(l.text.as_str())).collect()),
        lyrics_active: lyr.active_index(),
    }
}

pub fn home_data(h: &HomeViewModel) -> HomeData {
    let st = h.status.state();
    let f = h.focus.state();
    let stats = h.stats.state();
    let next = h.next_event();
    let later = h.later_events();
    // Sağ sütun: ayardaki sırayla yalnız görünen kartlar.
    let right: Vec<i32> = h
        .cards
        .iter()
        .filter_map(|c| match c {
            HomeCard::HeadphoneBattery if h.show_headphone_card() => Some(1),
            HomeCard::SystemNetwork if h.show_system_card() => Some(2),
            HomeCard::QuickLaunch if h.show_quick_launch_card() => Some(3),
            _ => None,
        })
        .collect();
    let head = h.bluetooth.as_ref().and_then(|b| b.primary_audio());
    HomeData {
        is_music_page: h.is_music_page(),
        greeting: st.greeting.clone().into(),
        day_date: st.day_date.clone().into(),
        show_weather: h.show_weather(),
        show_no_location: h.show_no_location(),
        weather_icon: st.weather_icon_kind.clone().into(),
        temperature_short: st.temperature_short.clone().into(),
        weather_description: st.weather_description.clone().into(),
        place_name: st.place_name().into(),
        has_next_event: next.is_some(),
        next_event: next.as_ref().map(|n| upcoming(0, n)).unwrap_or_default(),
        later_events: model(later.iter().enumerate().map(|(i, r)| upcoming(i + 1, r)).collect()),
        show_focus_row: h.show_focus_row(),
        focus_running: f.has_running,
        focus_progress: f.progress as f32,
        focus_home_line: f.home_line.clone().into(),
        focus_running_text: f.running_text.clone().into(),
        focus_can_skip: f.can_skip(),
        focus_time_text: f.time_text.clone().into(),
        right_cards: model(right),
        headphone_is_speaker: head.as_ref().is_some_and(|d| d.kind == BluetoothDeviceKind::Speaker),
        headphone_name: head.as_ref().map(|d| d.name.clone()).unwrap_or_default().into(),
        headphone_battery: head.as_ref().and_then(|d| d.battery_text.clone()).unwrap_or_default().into(),
        cpu_text: stats.cpu_text.clone().into(),
        top_process_text: stats.top_process_text.clone().into(),
        cpu_progress: stats.cpu_progress as f32,
        ram_text: stats.ram_text.clone().into(),
        memory_detail_text: stats.memory_detail_text.clone().into(),
        ram_progress: stats.ram_progress as f32,
        network_name: stats.network_name.clone().into(),
        download_text: stats.download_text.clone().into(),
        upload_text: stats.upload_text.clone().into(),
        quick_items: model(
            h.quick_launch
                .items()
                .iter()
                .enumerate()
                .map(|(i, q)| QuickItemData {
                    index: i as i32,
                    name: q.name.clone().into(),
                    initials: q.initials.clone().into(),
                    has_icon: q.icon.is_some(),
                    icon: q.icon.clone().unwrap_or_default(),
                })
                .collect(),
        ),
        quick_can_add: h.quick_launch.can_add(),
    }
}

pub fn install_text<C: ComponentHandle>(c: &C)
where
    for<'a> HomeText<'a>: slint::Global<'a, C>,
{
    let t = c.global::<HomeText>();
    let g = |k: &str| slint::SharedString::from(Strings::get(k));
    t.set_no_media(g("NoMediaPlaying"));
    t.set_live(g("MusicLive"));
    t.set_pick_location(g("WeatherPickLocation"));
    t.set_next_up(g("HomeNextUp"));
    t.set_no_events(g("HomeNoEvents"));
    t.set_focus_start(g("HomeFocusStart"));
    t.set_cpu(g("HomeCpu"));
    t.set_ram(g("HomeRam"));
    t.set_quick_launch(g("QuickLaunchTitle"));
    t.set_quick_remove(g("QuickLaunchRemove"));
    t.set_quick_add_file(g("QuickLaunchAddFile"));
    t.set_quick_add_folder(g("QuickLaunchAddFolder"));
}
