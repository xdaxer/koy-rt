/// Arka plan türü (`BackgroundAnimationPolicy` ve `LiveBackgroundPolicy` tarafından kullanılır).
/// Sayı değerleri C#'takiyle aynıdır (ayar dosyasından sayı olarak da gelebilir).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum BackgroundKind {
    None = 0,
    StillImage = 1,
    AnimatedImage = 2,
    Video = 3,
}

impl BackgroundKind {
    /// `(BackgroundKind)raw` dönüşümünün tanımlı değerler için karşılığı; tanımsız sayı (örn. 99) için `None`.
    pub fn from_i32(raw: i32) -> Option<BackgroundKind> {
        match raw {
            0 => Some(BackgroundKind::None),
            1 => Some(BackgroundKind::StillImage),
            2 => Some(BackgroundKind::AnimatedImage),
            3 => Some(BackgroundKind::Video),
            _ => None,
        }
    }
}

/// Pencere, güç ve hareket durumu. Arka planın hareket edip etmeyeceğine bunlar karar verir.
/// Varsayılan (`default`) değer "görünmez ve hareket yok" demektir: unutulan alan animasyonu açmaz.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BackgroundPlaybackState {
    /// Çentik kapalı ya da boşta (genişletilmemiş) ama ekranda görünüyor.
    pub is_notch_idle: bool,
    /// Pil tasarrufu açık ya da cihaz pilde.
    pub battery_saver: bool,
    /// Tam ekran bir uygulama önde.
    pub fullscreen: bool,
    /// Çentik ekranda görünüyor (VisibilityController.IsVisible). Gizliyken kimse görmez; "boşta da oynat" tercihi bunu aşamaz.
    pub is_notch_visible: bool,
    /// `MotionPolicy::scale` sonucu; 0 (ya da sayı değil) ise Windows'un animasyonları azalt ayarı
    /// ya da Koy'un Animasyon: Kapalı seçimi var, arka plan hareket etmez. Pil teriminin çift sayılmaması için
    /// `battery_saver_reduce_motion` burada false verilir; pil kararı `battery_saver` ile tercihe bağlıdır.
    pub motion_scale: f64,
}

impl BackgroundPlaybackState {
    pub const fn new(is_notch_idle: bool, battery_saver: bool, fullscreen: bool, is_notch_visible: bool, motion_scale: f64) -> Self {
        BackgroundPlaybackState { is_notch_idle, battery_saver, fullscreen, is_notch_visible, motion_scale }
    }
}

/// Kullanıcı tercihi: boşta ya da pilde de hareket izni. Varsayılan ikisi de kapalı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BackgroundAnimationOptions {
    /// Çentik boştayken de hareket etsin mi.
    pub animate_when_idle: bool,
    /// Pilde (pil tasarrufu) de hareket etsin mi.
    pub animate_on_battery: bool,
}

impl BackgroundAnimationOptions {
    pub const fn new(animate_when_idle: bool, animate_on_battery: bool) -> Self {
        BackgroundAnimationOptions { animate_when_idle, animate_on_battery }
    }
}

/// Arka planın hareket edip etmesini belirleyen saf karar. Durağan görsel hiçbir zaman hareket etmez.
/// Gizli çentik, tam ekran ve azaltılmış hareket her zaman durdurur (AGENTS.md #5, #6): tercihler bunları aşamaz.
/// Dekoder, oynatıcı ve WPF çizimi bu kararı uygulayan ayrı katmanlardır (Core dışı).
pub struct BackgroundAnimationPolicy;

impl BackgroundAnimationPolicy {
    /// Verilen tür ve durumda arka plan hareket etmeli mi.
    pub fn should_animate(kind: BackgroundKind, state: BackgroundPlaybackState, options: BackgroundAnimationOptions) -> bool {
        // Beyaz liste: ayar dosyasından gelen tanımsız sayı (örn. 99) hareket sayılmaz.
        if !matches!(kind, BackgroundKind::AnimatedImage | BackgroundKind::Video) {
            return false;
        }

        if !state.is_notch_visible || state.fullscreen {
            return false;
        }

        // NaN'da "> 0" yanlış çıkar; böylece bozuk çarpan da hareketsiz sayılır.
        if !(state.motion_scale > 0.0) {
            return false;
        }

        if state.is_notch_idle && !options.animate_when_idle {
            return false;
        }

        if state.battery_saver && !options.animate_on_battery {
            return false;
        }

        true
    }

    /// `ShouldAnimate(kind, state)`: tercihsiz (varsayılan `BackgroundAnimationOptions`) aşırı yükleme.
    pub fn should_animate_default(kind: BackgroundKind, state: BackgroundPlaybackState) -> bool {
        Self::should_animate(kind, state, BackgroundAnimationOptions::default())
    }

    /// Ham sayıyla çağrı: tanımsız tür sayısı (C#'ta `(BackgroundKind)99`) hareket sayılmaz.
    pub fn should_animate_raw(raw_kind: i32, state: BackgroundPlaybackState, options: BackgroundAnimationOptions) -> bool {
        match BackgroundKind::from_i32(raw_kind) {
            Some(kind) => Self::should_animate(kind, state, options),
            None => false,
        }
    }
}
