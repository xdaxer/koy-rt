/// Görünürlük durumunun sonucu: görünürlük ve tıklama geçirgenlik durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VisibilityResult {
    pub is_visible: bool,
    pub is_click_through: bool,
}

impl VisibilityResult {
    pub const fn new(is_visible: bool, is_click_through: bool) -> Self {
        VisibilityResult { is_visible, is_click_through }
    }
}
