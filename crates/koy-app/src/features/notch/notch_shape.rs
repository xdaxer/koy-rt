//! Çentik şekli. Ekrana yapışıkken üst sol ve sağda içbükey kulaklar, altta dışbükey yuvarlak köşeler; kulak yarıçapı
//! 0 iken (yüzen ada) üst köşeler de yuvarlanır ve şekil kapsüle döner. Karşılığı: `Features/Notch/NotchShape.cs`.
//!
//! Yazılım çizici saydam pencere desteklemediği için şekil iki yerde kullanılır: Slint'te zemin yolu (SVG komutları,
//! 640×364 tuvalde) ve pencere bölgesi (SetWindowRgn, piksel çokgeni) — bölgenin dışı hem görünmez hem tıklanmaz
//! (WPF'teki `HitTestCore` karşılığı).

use std::f64::consts::PI;

/// Çizilecek şekil: tuvaldeki sol üst köşe, boyut, yarıçaplar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeSpec {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
    pub corner_radius: f64,
    pub ear_radius: f64,
}

fn clamp_radii(s: &ShapeSpec) -> Option<(f64, f64)> {
    if !(s.width.is_finite() && s.height.is_finite()) || s.width <= 0.0 || s.height <= 0.0 {
        return None;
    }
    let ear = s.ear_radius.min((s.width / 4.0).min(s.height / 2.0)).max(0.0);
    let bot = s.corner_radius.min(((s.width - 2.0 * ear) / 2.0).min(s.height / 2.0)).max(0.0);
    Some((ear, bot))
}

/// SVG yol komutları (C# BuildGeometry ile aynı sıra).
pub fn path_commands(s: &ShapeSpec) -> String {
    let Some((ear, bot)) = clamp_radii(s) else { return String::new() };
    let (x, y, w, h) = (s.left, s.top, s.width, s.height);
    let f = |v: f64| format!("{v:.2}");
    let mut c = String::new();
    let top = if ear > 0.01 { 0.0 } else { bot };
    c += &format!("M {} {} ", f(x), f(y + top));
    if ear > 0.01 {
        // Sol üst içbükey kulak.
        c += &format!("A {e} {e} 0 0 1 {} {} ", f(x + ear), f(y + ear), e = f(ear));
    }
    c += &format!("L {} {} ", f(x + ear), f(y + h - bot));
    if bot > 0.01 {
        c += &format!("A {b} {b} 0 0 0 {} {} ", f(x + ear + bot), f(y + h), b = f(bot));
    }
    c += &format!("L {} {} ", f(x + w - ear - bot), f(y + h));
    if bot > 0.01 {
        c += &format!("A {b} {b} 0 0 0 {} {} ", f(x + w - ear), f(y + h - bot), b = f(bot));
    }
    c += &format!("L {} {} ", f(x + w - ear), f(y + if ear > 0.01 { ear } else { top }));
    if ear > 0.01 {
        c += &format!("A {e} {e} 0 0 1 {} {} ", f(x + w), f(y), e = f(ear));
        c += &format!("L {} {} ", f(x), f(y));
    } else {
        if top > 0.01 {
            c += &format!("A {t} {t} 0 0 0 {} {} ", f(x + w - top), f(y), t = f(top));
        }
        c += &format!("L {} {} ", f(x + top), f(y));
        if top > 0.01 {
            c += &format!("A {t} {t} 0 0 0 {} {} ", f(x), f(y + top), t = f(top));
        }
    }
    c + "Z"
}

fn arc(points: &mut Vec<(f64, f64)>, cx: f64, cy: f64, r: f64, a0: f64, a1: f64) {
    let n = ((r * (a1 - a0).abs()) / 2.0).ceil().clamp(2.0, 24.0) as usize;
    for i in 1..=n {
        let a = a0 + (a1 - a0) * i as f64 / n as f64;
        points.push((cx + r * a.cos(), cy + r * a.sin()));
    }
}

/// Şeklin çokgen yaklaşımı (tuval koordinatlarında). Yaylar 2 birimlik parçalara bölünür.
pub fn polygon(s: &ShapeSpec) -> Vec<(f64, f64)> {
    let Some((ear, bot)) = clamp_radii(s) else { return Vec::new() };
    let (x, y, w, h) = (s.left, s.top, s.width, s.height);
    let mut p = Vec::new();
    if ear > 0.01 {
        p.push((x, y));
        // Sol kulak: merkez (x, y+ear), açı -90° → 0°.
        arc(&mut p, x, y + ear, ear, -PI / 2.0, 0.0);
    } else {
        p.push((x, y + bot));
    }
    p.push((x + ear, y + h - bot));
    if bot > 0.01 {
        // Sol alt köşe: merkez (x+ear+bot, y+h-bot), 180° → 90°.
        arc(&mut p, x + ear + bot, y + h - bot, bot, PI, PI / 2.0);
    }
    p.push((x + w - ear - bot, y + h));
    if bot > 0.01 {
        arc(&mut p, x + w - ear - bot, y + h - bot, bot, PI / 2.0, 0.0);
    }
    if ear > 0.01 {
        p.push((x + w - ear, y + ear));
        // Sağ kulak: merkez (x+w, y+ear), 180° → 270°.
        arc(&mut p, x + w, y + ear, ear, PI, 3.0 * PI / 2.0);
    } else {
        p.push((x + w, y + bot));
        if bot > 0.01 {
            arc(&mut p, x + w - bot, y + bot, bot, 0.0, -PI / 2.0);
        }
        p.push((x + bot, y));
        if bot > 0.01 {
            arc(&mut p, x + bot, y + bot, bot, -PI / 2.0, -PI);
        }
    }
    p
}

/// Pencere bölgesini şekle göre ayarlar (`scale`: DIP → piksel). Boş şekilde bölge sıfırlanır (hiçbir şey görünmez).
pub fn apply_region(hwnd: isize, spec: &ShapeSpec, scale: f64) {
    use windows::Win32::Foundation::{HWND, POINT};
    use windows::Win32::Graphics::Gdi::{CreatePolygonRgn, CreateRectRgn, SetWindowRgn, WINDING};
    let pts: Vec<POINT> =
        polygon(spec).iter().map(|(x, y)| POINT { x: (x * scale).round() as i32, y: (y * scale).round() as i32 }).collect();
    // SAFETY: bölge tanıtıcısı SetWindowRgn başarılıysa sistemin olur; HWND çağıranın canlı penceresidir.
    unsafe {
        let rgn = if pts.len() >= 3 { CreatePolygonRgn(&pts, WINDING) } else { CreateRectRgn(0, 0, 0, 0) };
        let _ = SetWindowRgn(HWND(hwnd as *mut _), Some(rgn), true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_shape_has_ears_and_closes() {
        let s = ShapeSpec { left: 202.0, top: 0.0, width: 236.0, height: 34.0, corner_radius: 18.0, ear_radius: 12.0 };
        let c = path_commands(&s);
        assert!(c.starts_with("M 202.00 0.00 A 12.00 12.00 0 0 1 214.00 12.00"));
        assert!(c.ends_with('Z'));
        let p = polygon(&s);
        assert!(p.len() > 10);
        assert!(p.iter().all(|(x, y)| *x >= 201.9 && *x <= 438.1 && *y >= -0.1 && *y <= 34.1));
    }

    #[test]
    fn floating_shape_is_capsule_and_empty_is_empty() {
        let s = ShapeSpec { left: 0.0, top: 0.0, width: 100.0, height: 30.0, corner_radius: 15.0, ear_radius: 0.0 };
        assert!(path_commands(&s).starts_with("M 0.00 15.00"));
        assert!(path_commands(&ShapeSpec { width: 0.0, ..s }).is_empty());
    }
}
