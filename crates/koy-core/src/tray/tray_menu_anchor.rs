use crate::shell::WindowRect;

/// Tepsi simgesine tıklanınca uygulamaya iletilen fare konumu (menünün çıpası). Uygulama bağlam menüsünü bu
/// noktaya göre açar; nokta dock'un üzerine düşerse menü dock'un içine ya da altına taşar. Böyle bir nokta
/// dock'un hemen üstüne çekilir: menü ekranın altına sığmayınca yukarı açıldığı için dock'un üst kenarının
/// üstünde kalır.
pub struct TrayMenuAnchor;

impl TrayMenuAnchor {
    /// Çıpa dock'un üst kenarının bu kadar piksel üstünde durur.
    pub const GAP: i32 = 1;

    pub fn clamp_above_dock(x: i32, y: i32, dock: WindowRect) -> (i32, i32) {
        if dock.is_empty() || !dock.contains(x, y) {
            return (x, y);
        }

        (x, dock.top - Self::GAP)
    }
}
