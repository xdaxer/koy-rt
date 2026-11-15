//! DWM yardımcıları: `BackdropService` (Windows 11 pencere malzemesi) ve `DwmThumbnailHelper` (canlı küçük resim).

use koy_core::shell::WindowRect;
use windows::Win32::Foundation::{RECT, SIZE};
use windows::Win32::Graphics::Dwm::{
    DWM_THUMBNAIL_PROPERTIES, DWMWA_SYSTEMBACKDROP_TYPE, DWMWINDOWATTRIBUTE, DwmQueryThumbnailSourceSize, DwmRegisterThumbnail,
    DwmSetWindowAttribute, DwmUnregisterThumbnail, DwmUpdateThumbnailProperties,
};

use super::shell_action_service::os_build;
use super::window_styles::hwnd;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum WindowBackdropType {
    None = 1,
    Mica = 2,
    Acrylic = 3,
    MicaAlt = 4,
}

/// DWM köşe tercihi (DWM_WINDOW_CORNER_PREFERENCE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum WindowCornerStyle {
    Default = 0,
    Square = 1,
    Round = 2,
    RoundSmall = 3,
}

const USE_IMMERSIVE_DARK_MODE: i32 = 20; // DWMWA_USE_IMMERSIVE_DARK_MODE
const WINDOW_CORNER_PREFERENCE: i32 = 33; // DWMWA_WINDOW_CORNER_PREFERENCE

fn set(h: isize, attribute: DWMWINDOWATTRIBUTE, value: i32) {
    if h == 0 {
        return;
    }
    // SAFETY: 4 baytlık değer adresi ve boyutu verilir.
    let _ = unsafe { DwmSetWindowAttribute(hwnd(h), attribute, &value as *const i32 as *const _, 4) };
}

/// Windows 11 pencere malzemesi: sistem arka planı (akrilik / mika), koyu çerçeve ve köşe yuvarlaması DWM'den. Yalnız
/// katmansız pencerelerde etkilidir. Hata vermez: eski Windows'ta köşe ve sistem arka planı öznitelikleri atlanır.
pub struct BackdropService;

impl BackdropService {
    /// DWM sistem arka planı ve köşe yuvarlama desteği (Windows 11 / 22000+).
    pub fn is_supported() -> bool {
        os_build() >= 22000
    }

    pub fn apply_backdrop(h: isize, kind: WindowBackdropType) {
        if os_build() < 22621 {
            return;
        }
        set(h, DWMWA_SYSTEMBACKDROP_TYPE, kind as i32);
    }

    /// Arka plan, koyu / açık çerçeve (akriliğin tonu da buna uyar) ve köşe tercihi birlikte.
    pub fn apply_window_frame(h: isize, kind: WindowBackdropType, dark: bool, corners: WindowCornerStyle) {
        set(h, DWMWINDOWATTRIBUTE(USE_IMMERSIVE_DARK_MODE), i32::from(dark));
        let build = os_build();
        if build >= 22000 {
            set(h, DWMWINDOWATTRIBUTE(WINDOW_CORNER_PREFERENCE), corners as i32);
        }
        if build >= 22621 {
            set(h, DWMWA_SYSTEMBACKDROP_TYPE, kind as i32);
        }
    }
}

const DWM_TNP_RECTDESTINATION: u32 = 0x1;
const DWM_TNP_OPACITY: u32 = 0x4;
const DWM_TNP_VISIBLE: u32 = 0x8;

/// DWM pencere küçük resim kayıt ve güncelleme yardımcısı. Hedef pencere üst düzey olmalıdır.
pub struct DwmThumbnailHelper;

impl DwmThumbnailHelper {
    pub fn register(destination: isize, source: isize) -> isize {
        if destination == 0 || source == 0 {
            return 0;
        }
        // SAFETY: pencere tanıtıcıları çağıranın; kayıt tanıtıcısı döner.
        unsafe { DwmRegisterThumbnail(hwnd(destination), hwnd(source)) }.unwrap_or(0)
    }

    pub fn unregister(thumb: isize) {
        if thumb != 0 {
            // SAFETY: kayıt tanıtıcısı bir kez bırakılır.
            let _ = unsafe { DwmUnregisterThumbnail(thumb) };
        }
    }

    /// Kaynak pencerenin küçük resim boyutu (fiziksel piksel); bilinmiyorsa (0, 0).
    pub fn query_source_size(thumb: isize) -> (i32, i32) {
        if thumb == 0 {
            return (0, 0);
        }
        // SAFETY: yalnız sorgu.
        unsafe { DwmQueryThumbnailSourceSize(thumb) }.map_or((0, 0), |s: SIZE| (s.cx, s.cy))
    }

    /// Küçük resmi hedef pencerenin istemci alanında verilen dikdörtgene (fiziksel piksel) yerleştirir.
    pub fn update(thumb: isize, destination: WindowRect, opacity: u8) -> bool {
        if thumb == 0 || destination.is_empty() {
            return false;
        }
        let props = DWM_THUMBNAIL_PROPERTIES {
            dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE | DWM_TNP_OPACITY,
            rcDestination: RECT { left: destination.left, top: destination.top, right: destination.right, bottom: destination.bottom },
            opacity,
            fVisible: true.into(),
            ..Default::default()
        };
        // SAFETY: özellik yapısı yerel.
        unsafe { DwmUpdateThumbnailProperties(thumb, &props) }.is_ok()
    }
}
