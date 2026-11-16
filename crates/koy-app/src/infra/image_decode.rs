//! Bellekteki resmi (PNG, JPEG, BMP, GIF…) WIC ile çözer (WPF `BitmapImage` + `DecodePixelWidth` karşılığı).
//! Her iş parçacığından çağrılabilir: WIC fabrikası çağrı başına kurulur (COM MTA/STA kendiliğinden açılır).

use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA, IWICBitmapSource, IWICImagingFactory, WICBitmapDitherTypeNone,
    WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};

/// Çözülmüş resim: BGRA (önçarpımsız), satır başına `width * 4` bayt.
#[derive(Debug, Clone)]
pub struct Bgra {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Bgra {
    /// Slint resmine çevirir (RGBA sırası).
    pub fn to_image(&self) -> slint::Image {
        let mut buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(self.width, self.height);
        for (dst, src) in buf.make_mut_slice().iter_mut().zip(self.pixels.chunks_exact(4)) {
            *dst = slint::Rgba8Pixel { r: src[2], g: src[1], b: src[0], a: src[3] };
        }
        slint::Image::from_rgba8(buf)
    }
}

/// Yalnız başlıktan piksel genişliği (piksel çözülmez); okunamazsa 0.
pub fn pixel_width(bytes: &[u8]) -> u32 {
    with_frame(bytes, |_, frame| {
        let (mut w, mut h) = (0u32, 0u32);
        // SAFETY: çıkış göstergeleri yerel değişkenlere.
        unsafe { frame.GetSize(&mut w, &mut h).ok()? };
        Some(w)
    })
    .unwrap_or(0)
}

/// Resmi çözer; `decode_width` verilirse en-boy oranı korunarak o genişliğe ölçeklenir (WPF `DecodePixelWidth`).
pub fn decode(bytes: &[u8], decode_width: Option<u32>) -> Option<Bgra> {
    with_frame(bytes, |factory, frame| {
        // SAFETY: WIC nesneleri bu kapsamda yaşar; çıkış tamponu boyutu stride * yükseklik kadardır.
        unsafe {
            let (mut w, mut h) = (0u32, 0u32);
            frame.GetSize(&mut w, &mut h).ok()?;
            if w == 0 || h == 0 {
                return None;
            }
            let mut source: IWICBitmapSource = frame.clone().into();
            if let Some(dw) = decode_width.filter(|dw| *dw > 0 && *dw != w) {
                let nh = ((h as u64 * dw as u64 + w as u64 / 2) / w as u64).max(1) as u32;
                let scaler = factory.CreateBitmapScaler().ok()?;
                scaler.Initialize(&source, dw, nh, WICBitmapInterpolationModeFant).ok()?;
                source = scaler.into();
                w = dw;
                h = nh;
            }
            let converter = factory.CreateFormatConverter().ok()?;
            converter
                .Initialize(&source, &GUID_WICPixelFormat32bppBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)
                .ok()?;
            let stride = w * 4;
            let mut pixels = vec![0u8; (stride * h) as usize];
            converter.CopyPixels(std::ptr::null(), stride, &mut pixels).ok()?;
            Some(Bgra { width: w, height: h, pixels })
        }
    })
}

fn with_frame<T>(bytes: &[u8], f: impl FnOnce(&IWICImagingFactory, &windows::Win32::Graphics::Imaging::IWICBitmapFrameDecode) -> Option<T>) -> Option<T> {
    if bytes.is_empty() {
        return None;
    }
    // SAFETY: COM bu iş parçacığında açılır (zaten açıksa S_FALSE / RPC_E_CHANGED_MODE yok sayılır); akış ve
    // çözücü yerel nesnelerdir ve `bytes` çağrı süresince yaşar.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
        let stream = factory.CreateStream().ok()?;
        stream.InitializeFromMemory(bytes).ok()?;
        let decoder = factory.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand).ok()?;
        let frame = decoder.GetFrame(0).ok()?;
        f(&factory, &frame)
    }
}
