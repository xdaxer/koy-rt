//! Masaüstü duvar kağıdının ortalama rengi (okunabilirlik koruması için). Resim 16 px genişliğe küçültülerek çözülür;
//! yalnız ayar uygulanınca ve duvar kağıdı değişince çağrılır, yoklama yapılmaz. Duvar kağıdı resim değilse (düz renk)
//! "Control Panel\Colors\Background" okunur. Okunamazsa `None`: koruma o zaman seçilen malzemeyi aynen kullanır.
//! Çoklu duvar kağıdı ya da slayt gösterisinde Windows'un son çizdiği tek resim (TranscodedWallpaper) yeterli sayılır.

use std::path::{Path, PathBuf};

use koy_core::appearance::{ReadabilityGuard, Rgb};
use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapDecoder, BitmapInterpolationMode, BitmapPixelFormat, BitmapTransform, ColorManagementMode, ExifOrientationMode};
use windows::Storage::{FileAccessMode, StorageFile};
use windows::core::HSTRING;

use crate::registry::{self, Hive};

const SAMPLE_WIDTH: u32 = 16;

pub struct WallpaperSampler;

impl WallpaperSampler {
    pub fn sample() -> Option<Rgb> {
        match image_path() {
            None => solid_color(),
            Some(path) => decode_average(&path).ok().flatten(),
        }
    }
}

fn decode_average(path: &Path) -> windows::core::Result<Option<Rgb>> {
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path.as_os_str()))?.join()?;
    let stream = file.OpenAsync(FileAccessMode::Read)?.join()?;
    let decoder = BitmapDecoder::CreateAsync(&stream)?.join()?;
    let width = decoder.PixelWidth()?;
    let height = decoder.PixelHeight()?;
    if width == 0 || height == 0 {
        return Ok(None);
    }
    let transform = BitmapTransform::new()?;
    transform.SetScaledWidth(SAMPLE_WIDTH)?;
    // C#: Math.Round (bankacı).
    let scaled_height = koy_core::text::round(SAMPLE_WIDTH as f64 * height as f64 / width as f64) as u32;
    transform.SetScaledHeight(scaled_height.max(1))?;
    transform.SetInterpolationMode(BitmapInterpolationMode::Fant)?;
    let data = decoder
        .GetPixelDataTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Ignore,
            &transform,
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )?
        .join()?;
    let pixels = data.DetachPixelData()?;
    Ok(ReadabilityGuard::average_bgra(&pixels))
}

fn image_path() -> Option<PathBuf> {
    let path = registry::get_string(Hive::CurrentUser, r"Control Panel\Desktop", Some("WallPaper"), true);
    if let Some(p) = path.as_deref().filter(|p| !p.trim().is_empty())
        && Path::new(p).is_file()
    {
        return Some(PathBuf::from(p));
    }
    // Spotlight ve bazı slayt gösterilerinde kayıt boş kalır; Windows'un çizdiği kopya burada durur.
    let transcoded = PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join(r"Microsoft\Windows\Themes\TranscodedWallpaper");
    (transcoded.is_file() && path.as_deref().is_some_and(|p| !p.trim().is_empty())).then_some(transcoded)
}

fn solid_color() -> Option<Rgb> {
    let text = registry::get_string(Hive::CurrentUser, r"Control Panel\Colors", Some("Background"), true)?;
    let parts: Vec<&str> = text.split(' ').filter(|p| !p.is_empty()).collect();
    if let [r, g, b] = parts[..]
        && let (Ok(r), Ok(g), Ok(b)) = (r.parse::<u8>(), g.parse::<u8>(), b.parse::<u8>())
    {
        return Some(Rgb { r, g, b });
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn samples_without_failing() {
        // Sistem duvar kağıdına göre değişir; yalnız panik olmamalı.
        let _ = super::WallpaperSampler::sample();
    }
}
