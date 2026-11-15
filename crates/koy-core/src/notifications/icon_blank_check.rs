/// Çözülmüş simgenin "boş" olup olmadığına karar verir (kart boş daire yerine zil göstersin).
pub struct IconBlankCheck;

/// Resim içindeki dikdörtgen (piksel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IconBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl IconBounds {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        IconBounds { x, y, width, height }
    }
}

impl IconBlankCheck {
    /// Piksel başına bu alfa değerinin altı görünmez sayılır.
    pub const VISIBLE_ALPHA: u8 = 8;

    /// BGRA32 resimde görünür piksellerin (alfa > [`Self::VISIBLE_ALPHA`]) sınır kutusu; boşsa `None`.
    /// Bildirim logoları çoğu zaman saydam kenar boşluğuyla gelir; kart bu kutuya kırpıp daireyi dengeli doldurur.
    pub fn get_visible_bounds(bgra: &[u8], width: i32, height: i32) -> Option<IconBounds> {
        // C# `int` çarpımı taşarsa sarar; aynen.
        let needed = width.wrapping_mul(height).wrapping_mul(4);
        if width <= 0 || height <= 0 || (bgra.len() as i64) < needed as i64 {
            return None;
        }

        let (mut min_x, mut min_y, mut max_x, mut max_y) = (width, height, -1, -1);
        for y in 0..height {
            let row = (y * width * 4) as usize;
            for x in 0..width {
                if bgra[row + (x * 4) as usize + 3] <= Self::VISIBLE_ALPHA {
                    continue;
                }

                if x < min_x {
                    min_x = x;
                }
                if x > max_x {
                    max_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                max_y = y;
            }
        }

        if max_x < 0 { None } else { Some(IconBounds::new(min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)) }
    }

    /// BGRA32 pikselleri: veri yok ya da hiçbir pikselin alfası görünür değilse boştur.
    pub fn is_blank(bgra: &[u8]) -> bool {
        let mut i = 3;
        while i < bgra.len() {
            if bgra[i] > Self::VISIBLE_ALPHA {
                return false;
            }
            i += 4;
        }

        true
    }
}
