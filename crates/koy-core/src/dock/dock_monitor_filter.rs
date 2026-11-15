use super::dock_item::DockItem;
use crate::shell::WindowRect;

/// "Yalnız bu monitörün pencereleri" süzgeci. Bir pencere, merkezi dock'un monitöründeyse o monitöre aittir.
/// Karar yalnız pencere sınırlarından verilir (yeni Win32 çağrısı yok); sınırlar zaten pencere taramasıyla gelir.
pub struct DockMonitorFilter;

impl DockMonitorFilter {
    // Küçültülmüş pencerenin GetWindowRect değeri (-32000, -32000). Hangi monitörde olduğu bilinemez; kaybolmasın diye tutulur.
    const MINIMIZED_SENTINEL: i32 = -30000;

    /// Pencerenin merkezi monitörün içinde mi (sol/üst dahil, sağ/alt hariç).
    pub fn is_on_monitor(window: WindowRect, monitor: WindowRect) -> bool {
        if window.left <= Self::MINIMIZED_SENTINEL && window.top <= Self::MINIMIZED_SENTINEL {
            return true;
        }

        let cx = window.left + (window.right - window.left) / 2;
        let cy = window.top + (window.bottom - window.top) / 2;
        cx >= monitor.left && cx < monitor.right && cy >= monitor.top && cy < monitor.bottom
    }

    /// Öğeleri monitöre göre süzer. Pencereleri hiç kalmayan sabitli öğe "çalışmıyor" olarak kalır; sabitlenmemiş olan düşer.
    /// Pencere bilgisi olmayan öğe (Windows listesi null) olduğu gibi geçer.
    ///
    /// `foreground`: Ön plan pencere (0 = yok): öğenin etkinliği süzülmüş pencerelerle yeniden hesaplanır.
    pub fn apply(items: &[DockItem], monitor: WindowRect, foreground: isize) -> Vec<DockItem> {
        let mut result = Vec::with_capacity(items.len());
        for item in items {
            let windows = match &item.windows {
                Some(w) if !w.is_empty() => w,
                _ => {
                    result.push(item.clone());
                    continue;
                }
            };

            let kept: Vec<_> = windows.iter().filter(|w| Self::is_on_monitor(w.bounds, monitor)).cloned().collect();
            if kept.len() == windows.len() {
                result.push(item.clone());
                continue;
            }

            if kept.is_empty() {
                if !item.is_pinned {
                    continue;
                }

                result.push(DockItem {
                    is_running: false,
                    is_active: false,
                    is_flashing: false,
                    badge_count: 0,
                    window_handles: Vec::new(),
                    windows: Some(Vec::new()),
                    process_id: 0,
                    ..item.clone()
                });
                continue;
            }

            let handles: Vec<isize> = kept.iter().map(|w| w.handle).collect();
            result.push(DockItem {
                is_active: foreground != 0 && handles.contains(&foreground),
                window_handles: handles,
                windows: Some(kept),
                ..item.clone()
            });
        }

        result
    }
}
