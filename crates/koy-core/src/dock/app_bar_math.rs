use super::dock_edge::DockEdge;
use crate::shell::WindowRect;

/// AppBar'ın alt kenardaki son alanı. ABM_QUERYPOS, önerilen alanı alt kenardaki öteki çubukların (Gezgin'in 48 px'lik
/// görev çubuğu) üstüne iter. Windows görev çubuğu gizliyken alt kenar Koy'undur; itilen alan kabul edilirse pay 94 px
/// (46 + 48) olur ve çalışma alanında boşluk kalır (Gezgin henüz otomatik gizlemeye geçmemişken kayıt yapılınca olur).
pub struct AppBarMath;

impl AppBarMath {
    /// * `edge`: Dock'un oturduğu ekran kenarı.
    /// * `monitor`: Monitörün tam sınırları (piksel).
    /// * `thickness_px`: AppBar kalınlığı (piksel; yatayda yükseklik, dikeyde genişlik).
    /// * `queried`: ABM_QUERYPOS'un döndürdüğü alan.
    /// * `owns_bottom_edge`: Windows görev çubuğu gizli: alt kenar Koy'un, Gezgin'in itmesi yok sayılır.
    pub fn final_rect_with_edge(edge: DockEdge, monitor: WindowRect, thickness_px: i32, queried: WindowRect, owns_bottom_edge: bool) -> WindowRect {
        match edge {
            DockEdge::Bottom => {
                if owns_bottom_edge {
                    return WindowRect::new(monitor.left, monitor.bottom - thickness_px, monitor.right, monitor.bottom);
                }

                // Görev çubuğu görünürken: Windows'un önerdiği alt sınırın hemen üstüne yığılır.
                WindowRect::new(queried.left, queried.bottom - thickness_px, queried.right, queried.bottom)
            }
            DockEdge::Top => WindowRect::new(queried.left, queried.top, queried.right, queried.top + thickness_px),
            DockEdge::Left => WindowRect::new(queried.left, queried.top, queried.left + thickness_px, queried.bottom),
            DockEdge::Right => WindowRect::new(queried.right - thickness_px, queried.top, queried.right, queried.bottom),
        }
    }

    /// * `monitor`: Monitörün tam sınırları (piksel).
    /// * `height_px`: AppBar yüksekliği (piksel).
    /// * `queried`: ABM_QUERYPOS'un döndürdüğü alan.
    /// * `owns_bottom_edge`: Windows görev çubuğu gizli: alt kenar Koy'un, Gezgin'in itmesi yok sayılır.
    pub fn final_rect(monitor: WindowRect, height_px: i32, queried: WindowRect, owns_bottom_edge: bool) -> WindowRect {
        Self::final_rect_with_edge(DockEdge::Bottom, monitor, height_px, queried, owns_bottom_edge)
    }
}
