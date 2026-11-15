//! Karşılığı: `Input/HotCornerMath.cs`.

use super::input_binding::ScreenCorner;

/// Bir ekranın sınırları (sol, üst, genişlik, yükseklik). Sağ ve alt kenar dışarıdadır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ScreenRect {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

impl ScreenRect {
    pub const fn new(left: i32, top: i32, width: i32, height: i32) -> Self {
        ScreenRect { left, top, width, height }
    }

    pub fn right(&self) -> i32 {
        self.left.wrapping_add(self.width)
    }

    pub fn bottom(&self) -> i32 {
        self.top.wrapping_add(self.height)
    }
}

/// Çok ekranlı masaüstünde bulunan köşe: hangi ekran (listedeki sıra) ve hangi köşe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CornerHit {
    pub screen_index: i32,
    pub corner: ScreenCorner,
}

impl CornerHit {
    pub const fn new(screen_index: i32, corner: ScreenCorner) -> Self {
        CornerHit { screen_index, corner }
    }
}

/// Köşe tespiti için saf matematik. Fare kancası ve zamanlayıcı Platform'da; burada yalnız koordinat kararı var.
/// Koordinatlar ve köşe boyu aynı birimde olmalı (fiziksel piksel); ekran başına DPI farkı varsa çağıran köşe boyunu
/// o ekranın ölçeğiyle çarpıp verir.
pub struct HotCornerMath;

impl HotCornerMath {
    /// Nokta ekranın bir köşesinin `corner_size` piksel içindeyse o köşeyi döndürür; değilse `None`.
    /// Ekran dışı noktalar ve geçersiz boyutlar `None` verir. Köşe boyu ekranın yarısından büyükse
    /// nokta en yakın yatay ve dikey kenara göre seçilir (iki köşe çakışmaz).
    pub fn hit_test(x: i32, y: i32, screen: ScreenRect, corner_size: i32) -> Option<ScreenCorner> {
        if corner_size <= 0 || screen.width <= 0 || screen.height <= 0 {
            return None;
        }

        // Sağ ve alt kenar sınırın dışındadır (exclusive).
        if x < screen.left || x >= screen.right() || y < screen.top || y >= screen.bottom() {
            return None;
        }

        let left = x - screen.left <= screen.right() - 1 - x;
        let top = y - screen.top <= screen.bottom() - 1 - y;

        let dx = if left { x - screen.left } else { screen.right() - 1 - x };
        let dy = if top { y - screen.top } else { screen.bottom() - 1 - y };
        if dx >= corner_size || dy >= corner_size {
            return None;
        }

        Some(match (top, left) {
            (true, true) => ScreenCorner::TopLeft,
            (true, false) => ScreenCorner::TopRight,
            (false, true) => ScreenCorner::BottomLeft,
            _ => ScreenCorner::BottomRight,
        })
    }

    /// Çok ekranlı masaüstü için `hit_test`. Komşu ekrana geçilebilen kenardaki köşe
    /// gerçek köşe sayılmaz: yan yana iki ekranda ortak kenarın köşeleri imleci durdurmaz, yanlışlıkla tetiklenirdi.
    /// Yalnız masaüstünün dış köşeleri (ya da komşusuz kenarla oluşan köşeler) döner. Negatif koordinatlar desteklenir.
    /// (C#: `HitTest(int, int, IReadOnlyList<ScreenRect>, int)` aşırı yüklemesi)
    pub fn hit_test_screens(x: i32, y: i32, screens: &[ScreenRect], corner_size: i32) -> Option<CornerHit> {
        for (i, s) in screens.iter().enumerate() {
            let Some(corner) = Self::hit_test(x, y, *s, corner_size) else {
                continue;
            };

            let right = matches!(corner, ScreenCorner::TopRight | ScreenCorner::BottomRight);
            let bottom = matches!(corner, ScreenCorner::BottomLeft | ScreenCorner::BottomRight);

            // İmleç ekranın dış kenarına gidip bir piksel ötesine geçebiliyorsa o yönde komşu ekran vardır.
            let edge_x = if right { s.right() } else { s.left - 1 };
            let edge_y = if bottom { s.bottom() } else { s.top - 1 };
            if Self::contains_point(screens, i, edge_x, y) || Self::contains_point(screens, i, x, edge_y) {
                return None;
            }

            return Some(CornerHit { screen_index: i as i32, corner });
        }

        None
    }

    fn contains_point(screens: &[ScreenRect], skip: usize, x: i32, y: i32) -> bool {
        for (j, r) in screens.iter().enumerate() {
            if j == skip {
                continue;
            }

            if r.width > 0 && r.height > 0 && x >= r.left && x < r.right() && y >= r.top && y < r.bottom() {
                return true;
            }
        }

        false
    }

    /// İmleç köşeye `entered_at_ms` anında girdiyse, `now_ms` anında bekleme süresi doldu mu.
    /// Zamanlayıcı kurmaz; yalnız zamanları karşılaştırır.
    pub fn is_dwell_complete(entered_at_ms: i64, now_ms: i64, dwell_ms: i32) -> bool {
        if now_ms < entered_at_ms {
            return false;
        }
        now_ms.wrapping_sub(entered_at_ms) >= i64::from(dwell_ms.max(0))
    }
}
