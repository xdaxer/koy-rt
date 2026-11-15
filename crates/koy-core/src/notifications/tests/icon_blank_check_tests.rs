use crate::notifications::*;

#[test]
fn empty_data_is_blank() {
    assert!(IconBlankCheck::is_blank(&[]));
}

#[test]
fn fully_transparent_is_blank() {
    assert!(IconBlankCheck::is_blank(&[0u8; 64]));
}

#[test]
fn one_visible_pixel_is_not_blank() {
    let mut pixels = [0u8; 64];
    pixels[4 * 5 + 3] = 255;
    assert!(!IconBlankCheck::is_blank(&pixels));
}

#[test]
fn nearly_invisible_alpha_is_blank() {
    let mut pixels = [0u8; 16];
    pixels[3] = IconBlankCheck::VISIBLE_ALPHA;
    assert!(IconBlankCheck::is_blank(&pixels));
}

fn image(w: i32, h: i32, opaque: &[(i32, i32)]) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    for &(x, y) in opaque {
        px[((y * w + x) * 4 + 3) as usize] = 255;
    }
    px
}

#[test]
fn bounds_cover_only_visible_pixels() {
    let bounds = IconBlankCheck::get_visible_bounds(&image(10, 8, &[(2, 1), (6, 5), (4, 3)]), 10, 8);
    assert_eq!(bounds, Some(IconBounds::new(2, 1, 5, 5)));
}

#[test]
fn bounds_of_fully_opaque_image_are_the_whole_image() {
    let px = image(2, 2, &[(0, 0), (1, 0), (0, 1), (1, 1)]);
    assert_eq!(IconBlankCheck::get_visible_bounds(&px, 2, 2), Some(IconBounds::new(0, 0, 2, 2)));
}

#[test]
fn bounds_of_blank_or_malformed_image_are_null() {
    assert_eq!(IconBlankCheck::get_visible_bounds(&[0u8; 16], 2, 2), None);
    assert_eq!(IconBlankCheck::get_visible_bounds(&[0u8; 4], 2, 2), None);
    assert_eq!(IconBlankCheck::get_visible_bounds(&[], 0, 0), None);
}

#[test]
fn bounds_of_single_pixel() {
    assert_eq!(IconBlankCheck::get_visible_bounds(&image(5, 4, &[(3, 2)]), 5, 4), Some(IconBounds::new(3, 2, 1, 1)));
}
