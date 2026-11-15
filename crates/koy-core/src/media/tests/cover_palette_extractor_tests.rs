use crate::appearance::{ColorMath, Rgb};
use crate::media::CoverPaletteExtractor;

/// Piksel işlevinden BGRA görüntü kurar (C# testindeki `Image`). `stride` 0 ise `w * 4`.
fn image(w: i32, h: i32, pixel: impl Fn(i32, i32) -> (u8, u8, u8), stride: i32) -> Vec<u8> {
    let stride = if stride == 0 { w * 4 } else { stride };
    let mut data = vec![0u8; (stride * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let (r, g, b) = pixel(x, y);
            let o = (y * stride + x * 4) as usize;
            data[o] = b;
            data[o + 1] = g;
            data[o + 2] = r;
            data[o + 3] = 255;
        }
    }
    data
}

fn hue(c: Rgb) -> f64 {
    ColorMath::to_hsl(c).h
}

fn assert_in_range(actual: f64, low: f64, high: f64) {
    assert!(actual >= low && actual <= high, "{actual} not in [{low}, {high}]");
}

fn assert_eq_precision(expected: f64, actual: f64, precision: i32) {
    assert_eq!(crate::text::round_digits(expected, precision), crate::text::round_digits(actual, precision), "{expected} / {actual}");
}

#[test]
fn hsl_round_trips() {
    let rows: [(u8, u8, u8); 8] = [
        (0, 0, 0),
        (255, 255, 255),
        (255, 0, 0),
        (34, 197, 94),
        (14, 42, 24),
        (128, 128, 128),
        (244, 244, 245),
        (10, 200, 250),
    ];
    for (r, g, b) in rows {
        let c = Rgb::new(r, g, b);
        assert_eq!(c, ColorMath::from_hsl_value(ColorMath::to_hsl(c)), "{r} {g} {b}");
    }
}

#[test]
fn hsl_known_values() {
    let red = ColorMath::to_hsl(Rgb::new(255, 0, 0));
    assert_eq_precision(0.0, red.h, 3);
    assert_eq_precision(1.0, red.s, 3);
    assert_eq_precision(0.5, red.l, 3);
    assert_eq_precision(240.0, ColorMath::to_hsl(Rgb::new(0, 0, 255)).h, 3);
    assert_eq!(Rgb::new(0, 255, 0), ColorMath::from_hsl(120.0, 1.0, 0.5));
}

#[test]
fn pure_red_gives_red_card() {
    let px = image(8, 8, |_, _| (255, 0, 0), 0);
    let p = CoverPaletteExtractor::extract(&px, 8, 8, 32).expect("palet bekleniyordu");
    let h = hue(p.card);
    assert!(h < 5.0 || h > 355.0, "{h}");
    assert_in_range(ColorMath::to_hsl(p.card).l, 0.05, 0.16);
    assert_eq!(p.sub_text, p.spectrum);
    assert_in_range(ColorMath::to_hsl(p.sub_text).l, 0.78, 0.82);
}

#[test]
fn gray_gradient_gives_null() {
    let px = image(16, 16, |x, _| ((x * 16) as u8, (x * 16) as u8, (x * 16) as u8), 0);
    assert_eq!(None, CoverPaletteExtractor::extract(&px, 16, 16, 64));
}

#[test]
fn half_gray_half_blue_gives_blue() {
    let px = image(8, 8, |x, _| if x < 4 { (128, 128, 128) } else { (0, 0, 255) }, 0);
    let p = CoverPaletteExtractor::extract(&px, 8, 8, 32).expect("palet bekleniyordu");
    assert_in_range(hue(p.card), 235.0, 245.0);
}

#[test]
fn mostly_gray_with_few_colored_pixels_gives_null() {
    // 100 pikselin 5'i renkli: %10 eşiğinin altı.
    let px = image(10, 10, |x, y| if y == 0 && x < 5 { (255, 0, 0) } else { (120, 120, 120) }, 0);
    assert_eq!(None, CoverPaletteExtractor::extract(&px, 10, 10, 40));
}

#[test]
fn card_always_keeps_title_contrast() {
    let rows: [(u8, u8, u8); 5] = [(255, 255, 0), (0, 255, 255), (255, 0, 255), (255, 128, 0), (0, 255, 0)];
    for (r, g, b) in rows {
        let px = image(8, 8, |_, _| (r, g, b), 0);
        let p = CoverPaletteExtractor::extract(&px, 8, 8, 32).expect("palet bekleniyordu");
        assert!(ColorMath::contrast(Rgb::parse("#F4F4F5"), p.card) >= 7.0, "{r} {g} {b}");
    }
}

#[test]
fn stride_padding_is_ignored() {
    // Satır sonundaki dolgu baytları (beyaz) palete katılmamalı.
    let mut px = image(8, 8, |_, _| (0, 0, 255), 48);
    for y in 0..8usize {
        for i in 32..48usize {
            px[y * 48 + i] = 255;
        }
    }
    let p = CoverPaletteExtractor::extract(&px, 8, 8, 48).expect("palet bekleniyordu");
    assert_in_range(hue(p.card), 235.0, 245.0);
}

#[test]
fn empty_or_invalid_input_gives_null() {
    assert_eq!(None, CoverPaletteExtractor::extract(&[], 0, 0, 0));
    assert_eq!(None, CoverPaletteExtractor::extract(&[0u8; 4], 0, 1, 4));
    assert_eq!(None, CoverPaletteExtractor::extract(&[0u8; 10], 4, 4, 16)); // kısa dizi
    assert_eq!(None, CoverPaletteExtractor::extract(&[0u8; 64], 4, 4, 8)); // stride < width*4
    assert_eq!(None, CoverPaletteExtractor::extract(&[0u8; 64], 4, 4, 16)); // tamamen saydam
}

#[test]
fn single_pixel_works() {
    let p = CoverPaletteExtractor::extract(&[0, 255, 0, 255], 1, 1, 4).expect("palet bekleniyordu");
    assert_in_range(hue(p.card), 115.0, 125.0);
}

#[test]
fn many_coloured_logo_gives_a_blend_instead_of_one_tint() {
    // Chrome logosu gibi: dört çeyrek kırmızı, sarı, yeşil, mavi.
    let data = image(
        32,
        32,
        |x, y| match (x < 16, y < 16) {
            (true, true) => (234, 67, 53),
            (false, true) => (251, 188, 5),
            (true, false) => (52, 168, 83),
            _ => (66, 133, 244),
        },
        0,
    );

    let palette = CoverPaletteExtractor::extract(&data, 32, 32, 32 * 4).expect("palet bekleniyordu");

    let blend = palette.blend.as_ref().expect("karışım bekleniyordu");
    assert_eq!(4, blend.len());
    assert!(ColorMath::contrast(Rgb::new(0xF4, 0xF4, 0xF5), palette.card) >= 7.0);
}

#[test]
fn two_colour_cover_keeps_a_single_tint() {
    let data = image(32, 32, |x, _| if x < 16 { (124, 58, 237) } else { (219, 39, 119) }, 0);

    let palette = CoverPaletteExtractor::extract(&data, 32, 32, 32 * 4).expect("palet bekleniyordu");

    assert!(palette.blend.is_none());
}
