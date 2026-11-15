use super::dock_edge::DockEdge;
use crate::shell::WindowRect;
use crate::time::TimeSpan;

/// Alt kenardan dock açma kararı (saf mantık). Fare kenarda kesintisiz `delay` kadar kalınca "silahlanır";
/// kenardan ayrılınca ya da bir fare tuşu basılıyken (pencere sürükleme/boyutlandırma, seçim) sıfırlanır.
/// Zaman dışarıdan verilir; kare/zamanlayıcı aralığından bağımsız ve testlenebilir.
#[derive(Debug, Clone)]
pub struct EdgeRevealTracker {
    since: Option<TimeSpan>,
    delay: TimeSpan,
    is_armed: bool,
}

impl EdgeRevealTracker {
    /// Varsayılan bekleme: kasıtlı bir niyetle yanlışlıkla geçişi ayırır.
    pub const DEFAULT_DELAY: TimeSpan = TimeSpan::from_millis(300);

    /// Kenar bölgesi (piksel): fare gizliyken bu kadar yakınsa kenarda sayılır.
    pub const EDGE_ZONE_PX: i32 = 4;

    /// Dock açıkken fare bu kadar (DIP) yukarıya kadar "dock bölgesi" sayılır; kenardan dock'a yürürken kapanmaz.
    pub const REVEALED_ZONE_DIP: f64 = 80.0;

    pub fn default_delay() -> TimeSpan {
        Self::DEFAULT_DELAY
    }

    pub fn new(delay: TimeSpan) -> Self {
        EdgeRevealTracker { since: None, delay, is_armed: false }
    }

    pub fn delay(&self) -> TimeSpan {
        self.delay
    }

    /// Şu an silahlı mı (dock açılmalı mı).
    pub fn is_armed(&self) -> bool {
        self.is_armed
    }

    /// * `at_edge`: Fare bu monitörün alt kenar bölgesinde mi.
    /// * `button_pressed`: Herhangi bir fare tuşu basılı mı.
    /// * `now`: Tekdüze artan zaman (örn. Environment.TickCount64).
    pub fn update(&mut self, at_edge: bool, button_pressed: bool, now: TimeSpan) -> bool {
        if !at_edge || button_pressed {
            self.since = None;
            self.is_armed = false;
            return false;
        }

        let since = *self.since.get_or_insert(now);
        if now - since >= self.delay {
            self.is_armed = true;
        }

        self.is_armed
    }

    pub fn reset(&mut self) {
        self.since = None;
        self.is_armed = false;
    }

    /// Fare bu monitörün kenar bölgesinde mi. Gizliyken ince bir şerit, açıkken (dock'a ulaşmak için) daha yüksek bir bölge.
    /// Koordinatlar fiziksel pikseldir; `dpi_scale_y` DIP'ten piksele çevirir.
    pub fn is_in_zone(x: i32, y: i32, left: i32, right: i32, bottom: i32, revealed: bool, dpi_scale_y: f64) -> bool {
        if x < left || x >= right {
            return false;
        }

        let zone = if revealed { Self::revealed_zone_px(dpi_scale_y) } else { Self::EDGE_ZONE_PX };
        y >= bottom - zone && y <= bottom
    }

    /// Fare bu monitörün seçili kenar bölgesinde mi (Bottom, Top, Left, Right).
    pub fn is_in_zone_with_edge(
        edge: DockEdge,
        x: i32,
        y: i32,
        monitor_bounds: WindowRect,
        revealed: bool,
        dpi_scale_x: f64,
        dpi_scale_y: f64,
    ) -> bool {
        match edge {
            DockEdge::Top => {
                if x < monitor_bounds.left || x >= monitor_bounds.right {
                    return false;
                }
                let zone = if revealed { Self::revealed_zone_px(dpi_scale_y) } else { Self::EDGE_ZONE_PX };
                y >= monitor_bounds.top && y <= monitor_bounds.top + zone
            }
            DockEdge::Left => {
                if y < monitor_bounds.top || y >= monitor_bounds.bottom {
                    return false;
                }
                let zone = if revealed { Self::revealed_zone_px(dpi_scale_x) } else { Self::EDGE_ZONE_PX };
                x >= monitor_bounds.left && x <= monitor_bounds.left + zone
            }
            DockEdge::Right => {
                if y < monitor_bounds.top || y >= monitor_bounds.bottom {
                    return false;
                }
                let zone = if revealed { Self::revealed_zone_px(dpi_scale_x) } else { Self::EDGE_ZONE_PX };
                x >= monitor_bounds.right - zone && x <= monitor_bounds.right
            }
            DockEdge::Bottom => {
                Self::is_in_zone(x, y, monitor_bounds.left, monitor_bounds.right, monitor_bounds.bottom, revealed, dpi_scale_y)
            }
        }
    }

    // (int)Math.Ceiling(RevealedZoneDip * Math.Max(dpiScale, 1.0))
    fn revealed_zone_px(dpi_scale: f64) -> i32 {
        (Self::REVEALED_ZONE_DIP * dpi_scale.max(1.0)).ceil() as i32
    }
}
