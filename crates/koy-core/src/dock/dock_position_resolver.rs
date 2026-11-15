use super::dock_edge::DockEdge;
use super::dock_layout_limits::DockLayoutLimits;
use super::dock_layout_math::{DockLayoutMath, DockRectDip};
use super::dock_proximity_mode::{DockProximityMode, DockStyleMode};
use crate::display::DisplayMonitor;
use crate::shell::WindowRect;

/// Dock'un konum ve geometrik durum sonucu.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DockResolvedState {
    pub left_dip: f64,
    pub top_dip: f64,
    pub width_dip: f64,
    pub height_dip: f64,
    pub corner_radius_top_left: f64,
    pub corner_radius_top_right: f64,
    pub corner_radius_bottom_right: f64,
    pub corner_radius_bottom_left: f64,
    pub translation_y: f64,
    pub opacity: f64,
    pub is_snapped: bool,
    pub is_handle_visible: bool,
    pub is_hidden: bool,
    pub is_full_width: bool,
    pub app_bar_reserved_height_dip: i32,
    /// C# varsayılanı 0.0.
    pub translation_x: f64,
}

impl DockResolvedState {
    pub fn has_uniform_radius(&self) -> bool {
        self.corner_radius_top_left == self.corner_radius_top_right
            && self.corner_radius_top_right == self.corner_radius_bottom_right
            && self.corner_radius_bottom_right == self.corner_radius_bottom_left
    }
}

/// Dock'un ekran üzerindeki konumunu, boyutlarını, köşe yuvarlaklıklarını ve görünürlük durumunu
/// pencere yakınlığı (DockDown, ReserveSpace, Dodge, AlwaysHidden, AlwaysFloating) ve dock stiline (FloatingCapsule, FullWidth)
/// göre hesaplayan saf mantık çözücüsü (Core).
pub struct DockPositionResolver;

// Köşe yarıçapları (sol-üst, sağ-üst, sağ-alt, sol-alt).
type Corners = (f64, f64, f64, f64);

/// Sonuç kaydını kuran küçük yardımcı (C#'taki adlandırılmış bağımsız değişkenli yapıcının karşılığı).
#[allow(clippy::too_many_arguments)]
fn state(
    outer: DockRectDip,
    corners: Corners,
    translation: (f64, f64),
    opacity: f64,
    is_snapped: bool,
    is_handle_visible: bool,
    is_hidden: bool,
    is_full_width: bool,
    reserved: i32,
) -> DockResolvedState {
    DockResolvedState {
        left_dip: outer.left,
        top_dip: outer.top,
        width_dip: outer.width,
        height_dip: outer.height,
        corner_radius_top_left: corners.0,
        corner_radius_top_right: corners.1,
        corner_radius_bottom_right: corners.2,
        corner_radius_bottom_left: corners.3,
        translation_y: translation.1,
        opacity,
        is_snapped,
        is_handle_visible,
        is_hidden,
        is_full_width,
        app_bar_reserved_height_dip: reserved,
        translation_x: translation.0,
    }
}

impl DockPositionResolver {
    pub const FLOATING_HEIGHT_DIP: f64 = 60.0;
    pub const FLOATING_BOTTOM_MARGIN_DIP: f64 = 10.0;
    pub const FLOATING_CORNER_RADIUS_DIP: f64 = 18.0;

    pub const SNAPPED_HEIGHT_DIP: f64 = 46.0;
    pub const SNAPPED_CORNER_RADIUS_DIP: f64 = 14.0;

    /// Tam genişlik çubuğun yüksekliği: Windows 11 görev çubuğu ölçüsü. Dock penceresi, AppBar payı, XAML ve gizlenme
    /// kayması hep bu sabitten okur; başka yerde 48 yazılmaz.
    pub const FULL_WIDTH_HEIGHT_DIP: f64 = 48.0;
    pub const DODGE_HANDLE_WIDTH_DIP: f64 = 120.0;
    pub const DODGE_HANDLE_HEIGHT_DIP: f64 = 4.0;
    pub const DODGE_HANDLE_BOTTOM_MARGIN_DIP: f64 = 4.0;

    /// Pencerenin dock alanına yaklaştığını tespit etmek için kullanılan eşik mesafe (DIP).
    /// Dock'un alt kenardan yüksekliği 60 + 10 = 70 px olduğu için 80 px eşiği kullanılır.
    pub const PROXIMITY_THRESHOLD_DIP: f64 = 80.0;

    /// C# isteğe bağlı parametre varsayılanları: `is_window_maximized`, `is_mouse_at_bottom_edge`, `is_mouse_over_dock`,
    /// `is_window_fullscreen` = false; `hide_in_fullscreen` = true; `edge_reveal_enabled` = true; `edge` = Bottom;
    /// `length_percent` = `DockLayoutLimits::DEFAULT_LENGTH_PERCENT`; `edge_offset_dip` / `inner_padding_dip` = 0.0.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        monitor: Option<&DisplayMonitor>,
        dock_style: DockStyleMode,
        proximity_mode: DockProximityMode,
        content_width_dip: f64,
        foreground_window_bounds_dip: WindowRect,
        is_window_maximized: bool,
        is_mouse_at_bottom_edge: bool,
        is_mouse_over_dock: bool,
        is_window_fullscreen: bool,
        hide_in_fullscreen: bool,
        edge_reveal_enabled: bool,
        edge: DockEdge,
        length_percent: i32,
        edge_offset_dip: f64,
        inner_padding_dip: f64,
    ) -> DockResolvedState {
        let length_percent = DockLayoutLimits::clamp_length_percent(length_percent);
        let edge_offset_dip = DockLayoutLimits::clamp_edge_offset_dip(edge_offset_dip);
        let inner_padding_dip = DockLayoutLimits::clamp_inner_padding_dip(inner_padding_dip);

        // Gerçek tam ekran (oyun, F11, tam ekran video): dock HİÇBİR koşulda çıkmaz, kenar tetiği de yoktur
        // (hideInFullscreen kapalıysa kullanıcı dock'u tam ekranda da istiyor demektir, o zaman normal akış sürer).
        // Büyütülmüş pencerede ise dock gizlenir ama fare alt kenara gelince (edgeRevealEnabled) geri çıkar.
        let monitor_left = monitor.map_or(0.0, |m| m.left_dip());
        let monitor_top = monitor.map_or(0.0, |m| m.top_dip());
        let monitor_width = match monitor {
            Some(m) if m.width_dip() > 0.0 => m.width_dip(),
            _ => 1920.0,
        };
        let monitor_height = match monitor {
            Some(m) if m.height_dip() > 0.0 => m.height_dip(),
            _ => 1080.0,
        };

        let monitor_bottom = monitor_top + monitor_height;
        let monitor_right = monitor_left + monitor_width;

        let fg = foreground_window_bounds_dip;
        let (fg_left, fg_top, fg_right, fg_bottom) = (fg.left as f64, fg.top as f64, fg.right as f64, fg.bottom as f64);
        let threshold = Self::PROXIMITY_THRESHOLD_DIP;

        // Yakınlık / Çakışma denetimi (seçili kenara göre ProximityThresholdDip şeridi)
        let edge_intersects = match edge {
            DockEdge::Top => fg_top <= monitor_top + threshold && fg_bottom > monitor_top && fg_right > monitor_left && fg_left < monitor_right,
            DockEdge::Left => fg_left <= monitor_left + threshold && fg_right > monitor_left && fg_bottom > monitor_top && fg_top < monitor_bottom,
            DockEdge::Right => {
                fg_right >= monitor_right - threshold && fg_left < monitor_right && fg_bottom > monitor_top && fg_top < monitor_bottom
            }
            DockEdge::Bottom => {
                fg_bottom >= monitor_bottom - threshold && fg_top < monitor_bottom && fg_right > monitor_left && fg_left < monitor_right
            }
        };

        let window_intersects = is_window_maximized || (!fg.is_empty() && edge_intersects);

        let snapped_corners = || -> Corners {
            let r = Self::SNAPPED_CORNER_RADIUS_DIP;
            match edge {
                DockEdge::Top => (0.0, 0.0, r, r),
                DockEdge::Left => (0.0, r, r, 0.0),
                DockEdge::Right => (r, 0.0, 0.0, r),
                DockEdge::Bottom => (r, r, 0.0, 0.0),
            }
        };

        // (tx, ty)
        let translation = |is_hidden: bool, thickness: f64, offset: f64| -> (f64, f64) {
            if !is_hidden {
                return (0.0, 0.0);
            }
            let shift = thickness + offset + 10.0;
            match edge {
                DockEdge::Top => (0.0, -shift),
                DockEdge::Left => (-shift, 0.0),
                DockEdge::Right => (shift, 0.0),
                DockEdge::Bottom => (0.0, shift),
            }
        };

        let flat: Corners = (0.0, 0.0, 0.0, 0.0);

        // Tam Genişlik Modu (M6.3 / DESIGN.md)
        if dock_style == DockStyleMode::FullWidth {
            let monitor_work_area = DockRectDip::new(monitor_left, monitor_top, monitor_width, monitor_height);
            let layout = DockLayoutMath::compute(
                edge,
                length_percent,
                edge_offset_dip,
                inner_padding_dip,
                Self::FULL_WIDTH_HEIGHT_DIP,
                monitor_work_area,
            );
            let outer = layout.outer;
            let reserved_height = if proximity_mode == DockProximityMode::ReserveSpace { Self::FULL_WIDTH_HEIGHT_DIP as i32 } else { 0 };

            if is_window_fullscreen && hide_in_fullscreen {
                let t = translation(true, Self::FULL_WIDTH_HEIGHT_DIP, edge_offset_dip);
                return state(outer, flat, t, 0.0, true, false, true, true, reserved_height);
            }

            if is_window_maximized && hide_in_fullscreen && proximity_mode != DockProximityMode::ReserveSpace {
                let is_hovered = (edge_reveal_enabled && is_mouse_at_bottom_edge) || is_mouse_over_dock;
                let t = translation(!is_hovered, Self::FULL_WIDTH_HEIGHT_DIP, edge_offset_dip);
                return state(outer, flat, t, if is_hovered { 1.0 } else { 0.0 }, true, false, !is_hovered, true, 0);
            }

            return match proximity_mode {
                DockProximityMode::Dodge => {
                    if window_intersects {
                        let is_hovered = is_mouse_at_bottom_edge || is_mouse_over_dock;
                        let t = translation(!is_hovered, Self::FULL_WIDTH_HEIGHT_DIP, edge_offset_dip);
                        state(outer, flat, t, if is_hovered { 1.0 } else { 0.0 }, true, false, !is_hovered, true, 0)
                    } else {
                        state(outer, flat, (0.0, 0.0), 1.0, true, false, false, true, 0)
                    }
                }

                DockProximityMode::AlwaysHidden => {
                    let is_hovered = is_mouse_at_bottom_edge || is_mouse_over_dock;
                    let t = translation(!is_hovered, Self::FULL_WIDTH_HEIGHT_DIP, edge_offset_dip);
                    state(outer, flat, t, if is_hovered { 1.0 } else { 0.0 }, true, false, !is_hovered, true, 0)
                }

                // DockDown, ReserveSpace (ve AlwaysFloating: C# `default` kolu)
                _ => state(outer, flat, (0.0, 0.0), 1.0, true, false, false, true, reserved_height),
            };
        }

        // Yüzen Kapsül Modu (FloatingCapsule)
        let compute_capsule_rect = |is_snapped: bool| -> DockRectDip {
            let thickness = if is_snapped { Self::SNAPPED_HEIGHT_DIP } else { Self::FLOATING_HEIGHT_DIP };
            let offset = if is_snapped { edge_offset_dip } else { Self::FLOATING_BOTTOM_MARGIN_DIP + edge_offset_dip };
            let capsule_length = (if content_width_dip > 0.0 { content_width_dip } else { 520.0 }).max(160.0);

            if matches!(edge, DockEdge::Bottom | DockEdge::Top) {
                let width = capsule_length.min(monitor_width);
                let left = monitor_left + 0.0_f64.max((monitor_width - width) / 2.0);
                let top = if edge == DockEdge::Bottom { monitor_bottom - offset - thickness } else { monitor_top + offset };
                DockRectDip::new(left, top, width, thickness)
            } else {
                let height = capsule_length.min(monitor_height);
                let top = monitor_top + 0.0_f64.max((monitor_height - height) / 2.0);
                let left = if edge == DockEdge::Left { monitor_left + offset } else { monitor_right - offset - thickness };
                DockRectDip::new(left, top, thickness, height)
            }
        };

        let r = Self::FLOATING_CORNER_RADIUS_DIP;
        let round: Corners = (r, r, r, r);
        let float_offset = Self::FLOATING_BOTTOM_MARGIN_DIP + edge_offset_dip;

        if is_window_fullscreen && hide_in_fullscreen {
            let outer = compute_capsule_rect(false);
            let t = translation(true, Self::FLOATING_HEIGHT_DIP, float_offset);
            return state(outer, round, t, 0.0, false, false, true, false, 0);
        }

        // Aşağı yapış'ta büyütülmüş pencere dock'u gizlemez: dock alta yapışık kalır (aşağıdaki DockDown kolu), yer ayırmaz.
        // Yalnız gerçek tam ekran (yukarıdaki dal) gizler. Çekil ve Hep gizli büyütülmüş pencerede gizlemeyi sürdürür.
        // Hep havada da gizlemez: pencere ne kadar yaklaşırsa yaklaşsın kapsül yüzer kalır.
        if is_window_maximized
            && hide_in_fullscreen
            && !matches!(
                proximity_mode,
                DockProximityMode::ReserveSpace | DockProximityMode::DockDown | DockProximityMode::AlwaysFloating
            )
        {
            let outer = compute_capsule_rect(false);
            let is_hovered = (edge_reveal_enabled && is_mouse_at_bottom_edge) || is_mouse_over_dock;
            let t = translation(!is_hovered, Self::FLOATING_HEIGHT_DIP, float_offset);
            return state(outer, round, t, if is_hovered { 1.0 } else { 0.0 }, false, !is_hovered, !is_hovered, false, 0);
        }

        match proximity_mode {
            DockProximityMode::DockDown => {
                if window_intersects {
                    // Aşağı yapış: Kenara oturur, 60 px -> 46 px incelir, içe bakan iki köşe 14 px yuvarlak
                    let outer = compute_capsule_rect(true);
                    state(outer, snapped_corners(), (0.0, 0.0), 1.0, true, false, false, false, 0)
                } else {
                    // Boşta / Pencere uzakta: Yüzen kapsül (60 px, kenardan 10 px + ofset, 18 px yarıçap)
                    let outer = compute_capsule_rect(false);
                    state(outer, round, (0.0, 0.0), 1.0, false, false, false, false, 0)
                }
            }

            DockProximityMode::ReserveSpace => {
                // Yer ayır: Masaüstü çalışma alanından (AppBar) 46 px ayırır, dock kenara oturur
                let outer = compute_capsule_rect(true);
                state(outer, snapped_corners(), (0.0, 0.0), 1.0, true, false, false, false, Self::SNAPPED_HEIGHT_DIP as i32)
            }

            DockProximityMode::Dodge => {
                let outer = compute_capsule_rect(false);

                if window_intersects {
                    let is_hovered = is_mouse_at_bottom_edge || is_mouse_over_dock;
                    if is_hovered {
                        // Fare kenara geldiğinde dock kayarak geri çıkar
                        state(outer, round, (0.0, 0.0), 1.0, false, false, false, false, 0)
                    } else {
                        // Pencere üstüne binince dock çekilir, altta 120×4 tutamak kalır
                        let t = translation(true, Self::FLOATING_HEIGHT_DIP, float_offset);
                        state(outer, round, t, 0.0, false, true, true, false, 0)
                    }
                } else {
                    // Çakışan pencere yok, normal yüzer
                    state(outer, round, (0.0, 0.0), 1.0, false, false, false, false, 0)
                }
            }

            DockProximityMode::AlwaysHidden => {
                let outer = compute_capsule_rect(false);
                let is_hovered = is_mouse_at_bottom_edge || is_mouse_over_dock;
                let t = translation(!is_hovered, Self::FLOATING_HEIGHT_DIP, float_offset);
                state(outer, round, t, if is_hovered { 1.0 } else { 0.0 }, false, false, !is_hovered, false, 0)
            }

            DockProximityMode::AlwaysFloating => {
                // Hep havada: pencere yakınlığına (windowIntersects) ve büyütülmüşlüğe bakılmaz; hep yüzen kapsül.
                let outer = compute_capsule_rect(false);
                state(outer, round, (0.0, 0.0), 1.0, false, false, false, false, 0)
            }
        }
    }
}
