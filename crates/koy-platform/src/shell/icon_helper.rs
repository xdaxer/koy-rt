//! Kısayol (.lnk), çalıştırılabilir dosya (.exe) ya da kabuk yolundan simgeyi saydam PNG olarak çıkaran ve bellek
//! önbelleğinde tutan hafif yardımcı.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use koy_core::text;
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDIBits, GetObjectW, HBITMAP, HGDIOBJ,
};
use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGetFileInfoW, SIIGBF_ICONONLY};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};
use windows::core::HSTRING;

use super::png_encoder::PngEncoder;
use crate::search::com_apartment::ComApartment;

// 48 px: dock simgesi 24 DIP, %200 ölçekte bile büyütülmeden net kalır.
const ICON_SIZE: i32 = 48;

fn cache() -> &'static Mutex<HashMap<String, Option<Vec<u8>>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Vec<u8>>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub struct IconHelper;

impl IconHelper {
    /// Önbellek anahtarı büyük/küçük harf duyarsız (C# OrdinalIgnoreCase).
    pub fn get_icon_bytes(path: &str) -> Option<Vec<u8>> {
        if path.trim().is_empty() {
            return None;
        }
        let key = text::to_upper_invariant(path);
        if let Some(hit) = cache().lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
            return hit.clone();
        }
        let value = extract_icon_internal(path);
        cache().lock().unwrap_or_else(|e| e.into_inner()).entry(key).or_insert(value).clone()
    }

    /// Başka bir sürecin de olabilecek simge tanıtıcısını kendi boyutunda PNG'ye çevirir (tepsi simgeleri 16–32 px).
    /// Alfa kanalı olmayan eski simgelerde saydamlık maskeden alınır.
    pub fn hicon_to_png(hicon: isize) -> Option<Vec<u8>> {
        if hicon == 0 {
            return None;
        }
        let info = icon_info(HICON(hicon as *mut _))?;
        let result = (|| {
            if info.hbmColor.is_invalid() {
                return None;
            }
            let mut bitmap = BITMAP::default();
            // SAFETY: BITMAP yapısı boyutu kadar yazılır.
            let got = unsafe { GetObjectW(HGDIOBJ(info.hbmColor.0), std::mem::size_of::<BITMAP>() as i32, Some(&mut bitmap as *mut _ as *mut _)) };
            if got == 0 || bitmap.bmWidth <= 0 || bitmap.bmHeight <= 0 {
                return None;
            }
            let (w, h) = (bitmap.bmWidth, bitmap.bmHeight);
            let color = read_bgra(info.hbmColor, w, h)?;
            let mask = if info.hbmMask.is_invalid() { None } else { read_bgra(info.hbmMask, w, h) };
            // Simge (HICON) renk bit eşlemi düz alfadır; önceden çarpılmış sanıp bölmek koyu simgelerin kenarını parlatıyordu.
            Some(PngEncoder::encode(&to_straight_rgba(&color, mask.as_deref(), false), w, h))
        })();
        delete_icon_bitmaps(&info);
        result
    }
}

fn icon_info(hicon: HICON) -> Option<ICONINFO> {
    let mut info = ICONINFO::default();
    // SAFETY: çıktı yerel yapıya yazılır; bit eşlemler çağıran tarafından silinir.
    unsafe { GetIconInfo(hicon, &mut info) }.ok()?;
    Some(info)
}

fn delete_icon_bitmaps(info: &ICONINFO) {
    // SAFETY: GetIconInfo'nun oluşturduğu bit eşlemler bize aittir.
    unsafe {
        if !info.hbmColor.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
        }
        if !info.hbmMask.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
        }
    }
}

fn extract_icon_internal(path: &str) -> Option<Vec<u8>> {
    // Önce kabuğun resim fabrikası: .lnk için kısayol okunu eklemez ve "shell:AppsFolder\<AUMID>" ile mağaza
    // uygulamalarının gerçek simgesini verir. Olmazsa eski SHGetFileInfo yolu.
    // Kabuk simge arayüzleri STA ister: kısa ömürlü STA iş parçacığında çalıştırılır.
    let path = path.to_string();
    std::thread::spawn(move || {
        let _com = ComApartment::sta();
        extract_with_image_factory(&path).or_else(|| extract_with_file_info(&path))
    })
    .join()
    .ok()
    .flatten()
}

fn extract_with_image_factory(path: &str) -> Option<Vec<u8>> {
    // SAFETY: COM çağrıları; dönen bit eşlem silinir.
    unsafe {
        let factory: IShellItemImageFactory = SHCreateItemFromParsingName(&HSTRING::from(path), None).ok()?;
        let bitmap = factory.GetImage(SIZE { cx: ICON_SIZE, cy: ICON_SIZE }, SIIGBF_ICONONLY).ok()?;
        let png = convert_hbitmap_to_png(bitmap, ICON_SIZE, true);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        png
    }
}

fn extract_with_file_info(path: &str) -> Option<Vec<u8>> {
    if !Path::new(path).exists() {
        return None;
    }
    let mut info = SHFILEINFOW::default();
    // SAFETY: yapı boyutu doğru; dönen simge yok edilir.
    unsafe {
        let res = SHGetFileInfoW(&HSTRING::from(path), FILE_ATTRIBUTE_NORMAL, Some(&mut info), std::mem::size_of::<SHFILEINFOW>() as u32, SHGFI_ICON | SHGFI_LARGEICON);
        if res == 0 || info.hIcon.is_invalid() {
            return None;
        }
        let png = convert_hicon_to_png(info.hIcon);
        let _ = DestroyIcon(info.hIcon);
        png
    }
}

fn convert_hicon_to_png(hicon: HICON) -> Option<Vec<u8>> {
    let info = icon_info(hicon)?;
    // SHGFI_LARGEICON 32 px döner.
    let png = if info.hbmColor.is_invalid() { None } else { convert_hbitmap_to_png(info.hbmColor, 32, false) };
    delete_icon_bitmaps(&info);
    png
}

fn convert_hbitmap_to_png(bitmap: HBITMAP, size: i32, may_be_premultiplied: bool) -> Option<Vec<u8>> {
    let bgra = read_bgra(bitmap, size, size)?;
    Some(PngEncoder::encode(&to_straight_rgba(&bgra, None, may_be_premultiplied), size, size))
}

/// Bit eşlemi 32 bit, yukarıdan aşağı BGRA olarak okur.
fn read_bgra(bitmap: HBITMAP, width: i32, height: i32) -> Option<Vec<u8>> {
    if bitmap.is_invalid() {
        return None;
    }
    // SAFETY: bellek DC'si her yolda silinir; piksel arabelleği width*height*4 bayt.
    unsafe {
        let hdc = CreateCompatibleDC(None);
        let mut bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // yukarıdan aşağı satır sırası (PNG ile aynı)
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0, // BI_RGB
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bgra = vec![0u8; (width * height * 4) as usize];
        let lines = GetDIBits(hdc, bitmap, 0, height as u32, Some(bgra.as_mut_ptr() as *mut _), &mut bi, DIB_RGB_COLORS);
        if !hdc.is_invalid() {
            let _ = DeleteDC(hdc);
        }
        (lines != 0).then_some(bgra)
    }
}

/// BGRA'yı düz alfalı RGBA'ya çevirir. `may_be_premultiplied`: kabuk resim fabrikası önceden çarpılmış bit eşlem verir
/// (sezgisel denetim yapılır); simge (HICON) verisi her zaman düzdür, orada false geçilir. `mask_bgra`: simgenin AND
/// maskesi (32 bite açılmış): beyaz piksel saydamdır. Yalnız alfa yoksa kullanılır.
fn to_straight_rgba(bgra: &[u8], mask_bgra: Option<&[u8]>, may_be_premultiplied: bool) -> Vec<u8> {
    let mut has_alpha = false;
    let mut premultiplied = may_be_premultiplied;
    for p in bgra.chunks_exact(4) {
        let a = p[3];
        if a != 0 {
            has_alpha = true;
        }
        if p[0] > a || p[1] > a || p[2] > a {
            premultiplied = false;
        }
    }
    let mut rgba = vec![0u8; bgra.len()];
    for i in (0..bgra.len()).step_by(4) {
        let (mut b, mut g, mut r, mut a) = (bgra[i] as i32, bgra[i + 1] as i32, bgra[i + 2] as i32, bgra[i + 3] as i32);
        if !has_alpha {
            a = if mask_bgra.is_some_and(|m| m.len() == bgra.len() && m[i] != 0) { 0 } else { 255 };
        } else if premultiplied && a > 0 && a < 255 {
            r = r * 255 / a;
            g = g * 255 / a;
            b = b * 255 / a;
        }
        rgba[i] = r as u8;
        rgba[i + 1] = g as u8;
        rgba[i + 2] = b as u8;
        rgba[i + 3] = a as u8;
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpremultiplies_and_uses_mask() {
        // Önceden çarpılmış yarı saydam kırmızı.
        assert_eq!(to_straight_rgba(&[0, 0, 100, 200], None, true), vec![127, 0, 0, 200]);
        // Alfa yok: maske beyazsa saydam.
        assert_eq!(to_straight_rgba(&[1, 2, 3, 0], Some(&[255, 255, 255, 0]), false), vec![3, 2, 1, 0]);
        assert_eq!(to_straight_rgba(&[1, 2, 3, 0], None, false), vec![3, 2, 1, 255]);
    }

    #[test]
    fn extracts_notepad_icon() {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
        let png = IconHelper::get_icon_bytes(&format!(r"{windir}\notepad.exe"));
        if let Some(png) = png {
            assert_eq!(&png[1..4], b"PNG");
        }
    }
}
