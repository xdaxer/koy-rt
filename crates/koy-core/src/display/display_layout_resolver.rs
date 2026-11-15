use super::{DisplayMonitor, MonitorTargetMode};
use crate::text;

/// Monitör listesi ve hedefleme ayarlarına göre (Primary, All, Specific)
/// bileşenin (Notch/Dock) konumlandırılacağı hedef monitörleri çözümleyen saf etki alanı bileşeni.
pub struct DisplayLayoutResolver;

impl DisplayLayoutResolver {
    /// Verilen metinsel ayar değerini (örn. "Primary", "All", veya özel monitör kimliği)
    /// [`MonitorTargetMode`] numaralandırmasına çevirir.
    pub fn parse_mode(target_setting: Option<&str>) -> MonitorTargetMode {
        if target_setting.is_some_and(|s| text::eq_ignore_case(s, "All")) {
            return MonitorTargetMode::All;
        }

        if text::is_null_or_white_space(target_setting) || target_setting.is_some_and(|s| text::eq_ignore_case(s, "Primary")) {
            return MonitorTargetMode::Primary;
        }

        MonitorTargetMode::Specific
    }

    /// Belirtilen hedef modu ve ayar değerine göre hedef monitör listesini çözümler.
    pub fn resolve_target_monitors_with_mode(monitors: &[DisplayMonitor], target_setting: &str, mode: MonitorTargetMode) -> Vec<DisplayMonitor> {
        if monitors.is_empty() {
            return Vec::new();
        }

        match mode {
            MonitorTargetMode::All => monitors.to_vec(),
            MonitorTargetMode::Specific => {
                let found = monitors
                    .iter()
                    .find(|m| text::eq_ignore_case(&m.id, target_setting) || text::eq_ignore_case(&m.name, target_setting))
                    .or_else(|| monitors.iter().find(|m| m.is_primary))
                    .unwrap_or(&monitors[0]);
                vec![found.clone()]
            }
            MonitorTargetMode::Primary => vec![monitors.iter().find(|m| m.is_primary).unwrap_or(&monitors[0]).clone()],
        }
    }

    /// Metinsel ayar değerine göre otomatik mod tespiti yaparak hedef monitör listesini çözümler.
    pub fn resolve_target_monitors(monitors: &[DisplayMonitor], target_setting: &str) -> Vec<DisplayMonitor> {
        let mode = Self::parse_mode(Some(target_setting));
        Self::resolve_target_monitors_with_mode(monitors, target_setting, mode)
    }
}
