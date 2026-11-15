use super::dock_edge::DockEdge;
use crate::shell::WindowRect;

/// AppBar'ın alt kenarın sahibi olup olmadığı. Tek yerden hesaplanır; görev çubuğunun gizli durumu her değiştiğinde
/// (Hide başarılı, Restore, elle geri getirme, ayar değişimi) yeniden sorulur ve değişimde AppBar yeniden iddia edilir.
pub struct AppBarOwnership;

impl AppBarOwnership {
    /// * `hide_taskbar_setting`: "Windows görev çubuğunu gizle" ayarı.
    /// * `taskbar_hidden`: Görev çubuğu şu an gerçekten gizlendi (TaskbarService.Hide çalıştı, Restore edilmedi).
    /// * `restored_manually`: Kullanıcı görev çubuğunu elle geri getirdi ve dock yeniden gizlemedi.
    pub fn owns_bottom_edge(hide_taskbar_setting: bool, taskbar_hidden: bool, restored_manually: bool) -> bool {
        hide_taskbar_setting && taskbar_hidden && !restored_manually
    }

    /// Monitörün çalışma alanı AppBar'ın istediği payla uyuşmuyorsa true. Alt kenar Koy'undayken pay TAM olmalıdır:
    /// eksik ayrılmış (Gezgin sıfırlamış) da fazla ayrılmış (kayıt anında Gezgin'in 48 px'i de sayılmış, 94 px) da düzeltilir.
    /// Alt kenar Koy'un değilken (görev çubuğu görünür, başka çubuklar yığılır) yalnız eksik pay düzeltilir.
    /// Uyuşmazlık bir önceki yeniden iddiadan sonra da aynı kaldıysa (`unresolved_edge`, ör. başka bir
    /// uygulamanın çubuğu) tekrar denenmez: kendi SETPOS'umuzla döngüye girilmez.
    pub fn needs_reassert_with_edge(
        edge: DockEdge,
        monitor: WindowRect,
        work: WindowRect,
        thickness_px: i32,
        owns_bottom_edge: bool,
        unresolved_edge: Option<i32>,
    ) -> bool {
        let (mismatch, current_edge) = match edge {
            DockEdge::Top => {
                let expected = monitor.top + thickness_px;
                (work.top < expected, work.top)
            }
            DockEdge::Left => {
                let expected = monitor.left + thickness_px;
                (work.left < expected, work.left)
            }
            DockEdge::Right => {
                let expected = monitor.right - thickness_px;
                (work.right > expected, work.right)
            }
            DockEdge::Bottom => {
                let expected = monitor.bottom - thickness_px;
                (if owns_bottom_edge { work.bottom != expected } else { work.bottom > expected }, work.bottom)
            }
        };

        mismatch && unresolved_edge != Some(current_edge)
    }

    pub fn needs_reassert(
        monitor: WindowRect,
        work: WindowRect,
        height_px: i32,
        owns_bottom_edge: bool,
        unresolved_work_bottom: Option<i32>,
    ) -> bool {
        Self::needs_reassert_with_edge(DockEdge::Bottom, monitor, work, height_px, owns_bottom_edge, unresolved_work_bottom)
    }
}
