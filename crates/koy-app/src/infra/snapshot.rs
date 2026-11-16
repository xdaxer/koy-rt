//! Geliştirici yardımcısı: pencerenin çizimini PNG olarak kaydeder (`--snapshot <dosya>`). Arayüz taşınırken
//! görünümün WPF ile karşılaştırılması için; kullanıcı akışında çağrılmaz.

use koy_platform::shell::png_encoder::PngEncoder;

pub fn save(window: &slint::Window, path: &str) -> bool {
    let Ok(buffer) = window.take_snapshot() else { return false };
    let (w, h) = (buffer.width() as i32, buffer.height() as i32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for p in buffer.as_slice() {
        rgba.extend_from_slice(&[p.r, p.g, p.b, p.a]);
    }
    std::fs::write(path, PngEncoder::encode(&rgba, w, h)).is_ok()
}
