/// Bir üst düzey pencerenin görev çubuğuna girip girmeyeceğini belirleyen olgular (Win32'den okunur).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TaskbarWindowFacts {
    /// WS_VISIBLE (IsWindowVisible).
    pub is_visible: bool,
    /// DWM gizlemesi (başka sanal masaüstü, askıdaki Mağaza uygulaması).
    pub is_cloaked: bool,
    /// Masaüstü, Progman, WorkerW, görev çubuğu penceresi.
    pub is_shell_window: bool,
    /// GW_OWNER boş değil (diyalog, alt pencere).
    pub has_owner: bool,
    /// WS_EX_APPWINDOW.
    pub is_app_window: bool,
    /// WS_EX_TOOLWINDOW.
    pub is_tool_window: bool,
    /// WS_EX_NOACTIVATE.
    pub is_no_activate: bool,
}

/// Windows görev çubuğunun kuralı: görünür ve gizlenmemiş pencere, WS_EX_APPWINDOW ise her zaman girer; değilse
/// sahipsiz olmalı ve araç penceresi ya da etkinleşmeyen pencere olmamalı. Başlığın boş olması tek başına eleme
/// sebebi değildir (başlığı sonradan konan pencereler eskiden listeden düşüyordu).
pub struct TaskbarWindowRule;

impl TaskbarWindowRule {
    pub fn should_show(facts: &TaskbarWindowFacts) -> bool {
        if !facts.is_visible || facts.is_cloaked || facts.is_shell_window {
            return false;
        }

        if facts.is_app_window {
            return true;
        }

        !facts.has_owner && !facts.is_tool_window && !facts.is_no_activate
    }
}
