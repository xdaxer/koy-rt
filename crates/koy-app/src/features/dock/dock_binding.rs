//! Dock durumunu Slint yapılarına çevirir ve sabit metinleri kurar.

use slint::{ComponentHandle, ModelRc, VecModel};

use koy_core::dock::items::DockItemKind;
use koy_core::dock::{DockEdge, DockIconAlignment, DockLabelMode};

use super::app_icons::StartIcon;
use super::dock_view_model::DockViewModel;
use super::entries::DockEntry;
use super::presentation::DockPresentationStyle;
use crate::resources::strings::Strings;
use crate::{DockData, DockEntryData, DockPresentationData, DockText};

pub fn presentation_data(p: &DockPresentationStyle) -> DockPresentationData {
    DockPresentationData {
        draws_mark: p.draws_mark(),
        use_line: p.use_line(),
        is_highlight: p.is_highlight(),
        has_indicator_color: p.indicator_color.is_some(),
        indicator_color: p.indicator_color.unwrap_or_default(),
        has_highlight_color: p.highlight_color.is_some(),
        highlight_color: p.highlight_color.unwrap_or_default(),
        badge_dot_only: p.badge_dot_only(),
        has_badge_color: p.badge_color.is_some(),
        badge_color: p.badge_color.unwrap_or_default(),
        badge_right: p.badge_right,
        badge_top: p.badge_top,
        label_mode: match p.label_mode {
            DockLabelMode::Always => 1,
            DockLabelMode::OnHover => 2,
            #[allow(unreachable_patterns)]
            _ => 0,
        },
    }
}

fn extra_kind(k: DockItemKind) -> i32 {
    match k {
        DockItemKind::Separator => 0,
        DockItemKind::Spacer => 1,
        DockItemKind::Clock => 2,
        DockItemKind::Trash => 3,
        _ => 4,
    }
}

pub fn entry_data(e: &DockEntry) -> DockEntryData {
    let mut d = DockEntryData::default();
    let m = |d: &mut DockEntryData, m: &super::entries::EntryMotion| {
        d.icon_size = m.icon_size as f32;
        d.item_size = m.item_size as f32;
        d.scale = m.scale_value as f32;
        d.press_scale = m.press_scale() as f32;
        d.press_opacity = m.press_opacity() as f32;
        d.translate_x = m.translate_x() as f32;
        d.translate_y = m.translate_y() as f32;
        d.target_glow = m.is_target_glow;
    };
    match e {
        DockEntry::App(a) => {
            let a = a.borrow();
            m(&mut d, &a.motion);
            d.kind = 0;
            d.id = a.id().into();
            d.title = a.title.clone().into();
            d.has_icon = a.icon.is_some();
            d.icon = a.icon.clone().unwrap_or_default();
            d.active = a.is_active;
            d.running = a.is_running;
            d.attention = a.is_attention;
            d.indicator_count = a.indicator_count;
            d.show_badge = a.show_badge;
            d.badge_text = a.badge_text.clone().into();
        }
        DockEntry::Folder(f) => {
            let f = f.borrow();
            m(&mut d, &f.motion);
            d.kind = 1;
            d.id = f.id().into();
            d.title = f.title.clone().into();
            d.running = f.is_running();
            d.preview_icons = ModelRc::new(VecModel::from(f.preview_icons().into_iter().map(Option::unwrap_or_default).collect::<Vec<_>>()));
        }
        DockEntry::Extra(x) => {
            let x = x.borrow();
            m(&mut d, &x.motion);
            d.kind = 2;
            d.id = x.id().into();
            d.title = x.title.clone().into();
            d.extra_kind = extra_kind(x.kind());
            d.glyph = x.glyph().into();
            d.time_text = x.time_text.clone().into();
            d.date_text = x.date_text.clone().into();
        }
    }
    d
}

pub fn entries(vm: &DockViewModel) -> ModelRc<DockEntryData> {
    ModelRc::new(VecModel::from(vm.items().iter().map(entry_data).collect::<Vec<_>>()))
}

pub fn dock_data(vm: &DockViewModel) -> DockData {
    let st = vm.state();
    let vertical = vm.is_vertical();
    let (sl, st_, sr, sb) = vm.slot_margin();
    let spacing = if vertical { st_ + sb } else { sl + sr };
    let (fl, ft, fr, fb) = vm.full_width_border_thickness();
    let (hw, hh) = vm.dodge_handle_size();
    let (start_kind, start_image) = match &vm.start_icon {
        Some(StartIcon::Windows) => (1, Default::default()),
        Some(StartIcon::Custom(i)) => (2, i.clone()),
        _ => (0, Default::default()),
    };
    let (ml, mt, mr, mb) = st.dock_margin;
    let (tl, tr, br, bl) = st.capsule_corner_radius;
    DockData {
        vertical,
        full_width: st.is_full_width_mode,
        item_size: st.item_size as f32,
        slot_spacing: spacing as f32,
        inner_padding: vm.inner_padding() as f32,
        capsule_surface: st.capsule_surface,
        full_surface: st.full_width_surface,
        dock_height: st.dock_height as f32,
        margin_left: ml as f32,
        margin_top: mt as f32,
        margin_right: mr as f32,
        margin_bottom: mb as f32,
        radius_tl: tl as f32,
        radius_tr: tr as f32,
        radius_br: br as f32,
        radius_bl: bl as f32,
        full_border_left: fl as f32,
        full_border_top: ft as f32,
        full_border_right: fr as f32,
        full_border_bottom: fb as f32,
        translate_x: st.translation_x as f32,
        translate_y: st.translation_y as f32,
        opacity: st.opacity as f32,
        is_visible: st.is_visible,
        show_start: st.show_start_button,
        start_kind,
        start_image,
        show_folders: st.show_folders,
        show_recents: vm.show_recents(),
        show_full_tray: vm.show_full_tray(),
        show_small_tray: vm.show_small_tray(),
        show_right_section: vm.show_right_section(),
        small_tray_width: vm.small_tray_width() as f32,
        tray_open: st.is_tray_popup_open,
        recents_open: st.is_recents_open,
        has_now_playing: st.has_now_playing,
        now_playing_title: st.now_playing_title.clone().into(),
        now_playing_artist: st.now_playing_artist.clone().into(),
        has_cover: st.now_playing_cover.is_some(),
        now_playing_cover: st.now_playing_cover.clone().unwrap_or_default(),
        strip_alignment: match vm.icon_alignment() {
            DockIconAlignment::Left => 1,
            DockIconAlignment::Right => 2,
            _ => 0,
        },
        items_max_length: if st.items_max_width.is_finite() { st.items_max_width as f32 } else { 100000.0 },
        strip_clipped: st.is_strip_clipped,
        handle_visible: st.is_handle_visible,
        handle_width: hw as f32,
        handle_height: hh as f32,
        edge: match vm.edge() {
            DockEdge::Bottom => 0,
            DockEdge::Top => 1,
            DockEdge::Left => 2,
            DockEdge::Right => 3,
        },
        edge_line_visible: st.is_edge_line_visible,
    }
}

pub fn install_text<C: ComponentHandle>(c: &C)
where
    for<'a> DockText<'a>: slint::Global<'a, C>,
{
    let t = c.global::<DockText>();
    let g = |k: &str| slint::SharedString::from(Strings::get(k));
    t.set_start_menu(g("StartMenu"));
    t.set_downloads(g("DockDownloads"));
    t.set_recent_files(g("RecentFiles"));
    t.set_system_tray(g("SystemTray"));
    t.set_koy_menu(g("DockKoyMenu"));
    t.set_now_playing(g("DockNowPlayingTooltip"));
    t.set_show_dock(g("DockShowTooltip"));
}
