/// Ekran koordinatlarında pencere sınırları (Win32 fiziksel piksel koordinatları).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct WindowRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl WindowRect {
    /// `WindowRect.Empty` (`default`): dört kenar da 0.
    pub const EMPTY: WindowRect = WindowRect { left: 0, top: 0, right: 0, bottom: 0 };

    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        WindowRect { left, top, right, bottom }
    }

    pub fn width(&self) -> i32 {
        0.max(self.right - self.left)
    }

    pub fn height(&self) -> i32 {
        0.max(self.bottom - self.top)
    }

    pub fn is_empty(&self) -> bool {
        self.width() == 0 || self.height() == 0
    }

    pub fn intersects(&self, other: WindowRect) -> bool {
        self.left < other.right && self.right > other.left && self.top < other.bottom && self.bottom > other.top
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}
