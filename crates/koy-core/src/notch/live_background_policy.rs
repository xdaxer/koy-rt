use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::background::{BackgroundAnimationPolicy, BackgroundKind, BackgroundPlaybackState};
use crate::text;

/// Canlı arka planın nereye uygulanacağı (issue #67). Değerler ayar dosyasına yazılır: mevcutlar değiştirilmez,
/// yeni üye yalnız sona eklenir. Varsayılan `None`: özellik kapalı, hiçbir şey çizilmez ve yüklenmez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(i32)]
pub enum LiveBackgroundTarget {
    #[default]
    None = 0,
    Notch = 1,
    MusicCard = 2,
    Both = 3,
}

/// Kaynağın nereden geldiği. Değerler kalıcıdır (ayar dosyası), yeni üye yalnız sona eklenir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(i32)]
pub enum LiveBackgroundSourceKind {
    /// Koy'un yerleşik, dosyasız ambiyans önayarı (renk geçişi, kod ile çizilir).
    #[default]
    BuiltInAmbient = 0,
    /// Kullanıcının seçtiği yerel GIF / video / görsel dosyası.
    LocalFile = 1,
}

/// Arka planın çizileceği yüzey.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LiveBackgroundSurface {
    /// Açık çentiğin bütün zemini.
    #[default]
    Notch,
    /// Ana sayfadaki müzik kartının zemini.
    MusicCard,
}

/// Oynatma kararı. Yalnız `Animate` kare üretir; diğerleri hareketsizdir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LiveBackgroundPlaybackMode {
    /// Hiçbir şey çizilmez ve bellekte tutulmaz (çentik kapalı / gizli ya da özellik bu yüzeyde kapalı).
    #[default]
    Off,
    /// Yalnız sabit ilk kare gösterilir (hareketi azalt / pil tasarrufu).
    StaticFirstFrame,
    /// Animasyon ya da video oynar.
    Animate,
}

/// Yerleşik ambiyans önayarı. Renkler "#RRGGBB" metni: Core WPF'e bağımlı değildir, App fırçayı kendisi kurar.
/// Hepsi koyu tonlardır; ayrıca üstüne koyu perde biner (okunabilirlik).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveBackgroundPreset {
    /// Ayar dosyasında saklanan kalıcı kimlik; değiştirilmez.
    pub id: String,
    /// Strings.resx anahtarı.
    pub name_key: String,
    /// Geçişte dönen renkler (en az 2).
    pub colors: Vec<String>,
    /// Bir tam geçiş döngüsünün süresi (yalnız renk geçişinde).
    pub cycle_seconds: i32,
    /// Doluysa renk geçişi yerine `assets/backgrounds/` altındaki bu MP4 döngüsü oynatılır.
    pub video_file: Option<String>,
}

impl LiveBackgroundPreset {
    fn new(id: &str, name_key: &str, colors: &[&str], cycle_seconds: i32, video_file: Option<&str>) -> Self {
        LiveBackgroundPreset {
            id: id.to_string(),
            name_key: name_key.to_string(),
            colors: colors.iter().map(|c| c.to_string()).collect(),
            cycle_seconds,
            video_file: video_file.map(str::to_string),
        }
    }
}

/// Yerleşik ambiyans önayarlarının küçük kataloğu.
pub struct LiveBackgroundPresets;

impl LiveBackgroundPresets {
    pub const DEFAULT_ID: &'static str = "aurora";

    /// `All`.
    pub fn all() -> &'static [LiveBackgroundPreset] {
        static ALL: OnceLock<Vec<LiveBackgroundPreset>> = OnceLock::new();
        ALL.get_or_init(|| {
            let p = LiveBackgroundPreset::new;
            vec![
                p("aurora", "LiveBgPresetAurora", &["#0F3A47", "#1E2A5E", "#3B1F5C"], 18, None),
                p("ember", "LiveBgPresetEmber", &["#3A1410", "#5C2A12", "#2B1033"], 20, None),
                p("ocean", "LiveBgPresetOcean", &["#0B2545", "#13496B", "#0E6B73"], 16, None),
                p("dusk", "LiveBgPresetDusk", &["#1F1B4B", "#4A1F4F", "#6B2A3A"], 22, None),
                // 620x352 30fps HD MP4 video döngüleri (assets/backgrounds/).
                p("plasma", "LiveBgPresetPlasma", &["#3B1F5C", "#0E6B73", "#1E2A5E", "#6B2A3A"], 0, Some("plasma.mp4")),
                p("lava", "LiveBgPresetLava", &["#1A0B1E", "#C2410C", "#F59E0B"], 0, Some("lava.mp4")),
                p("nebula", "LiveBgPresetNebula", &["#0B0B1E", "#6D28D9", "#DB2777"], 0, Some("nebula.mp4")),
                p("silk", "LiveBgPresetSilk", &["#1E1B4B", "#0F766E", "#7C3AED", "#BE185D"], 0, Some("silk.mp4")),
                p("northern", "LiveBgPresetNorthern", &["#071426", "#22C55E", "#8B5CF6"], 0, Some("northern.mp4")),
                p("waves", "LiveBgPresetWaves", &["#0B1020", "#1E3A8A", "#0E7490", "#2563EB"], 0, Some("waves.mp4")),
                p("contour", "LiveBgPresetContour", &["#0B1220", "#38BDF8", "#A78BFA"], 0, Some("contour.mp4")),
                p("horizon", "LiveBgPresetHorizon", &["#1A0B2E", "#7C2D82", "#F59E0B", "#EC4899", "#D946EF"], 0, Some("horizon.mp4")),
                p("underwater", "LiveBgPresetUnderwater", &["#03192B", "#0B4A6B", "#7DD3FC"], 0, Some("underwater.mp4")),
                p("pearl", "LiveBgPresetPearl", &["#4C1D95", "#0E7490", "#BE185D", "#A16207"], 0, Some("pearl.mp4")),
                p("nightsky", "LiveBgPresetNightSky", &["#05060E", "#1E1B4B", "#F8FAFC"], 0, Some("nightsky.mp4")),
                p("fireflies", "LiveBgPresetFireflies", &["#05070A", "#0F2A1E", "#FACC15"], 0, Some("fireflies.mp4")),
                p("snow", "LiveBgPresetSnow", &["#0F172A", "#1E293B", "#E2E8F0"], 0, Some("snow.mp4")),
                p("rain", "LiveBgPresetRain", &["#0B1320", "#1E293B", "#94A3B8"], 0, Some("rain.mp4")),
                p("bokeh", "LiveBgPresetBokeh", &["#0B0B12", "#1A1033", "#F472B6", "#60A5FA", "#FBBF24"], 0, Some("bokeh.mp4")),
                p("warp", "LiveBgPresetWarp", &["#04050C", "#1E3A8A", "#E0F2FE"], 0, Some("warp.mp4")),
                p("ripples", "LiveBgPresetRipples", &["#04121C", "#0EA5E9", "#A5F3FC"], 0, Some("ripples.mp4")),
            ]
        })
    }

    /// Kimliğe göre önayar; bilinmeyen ya da boş kimlikte varsayılan döner (ayar dosyası elle bozulmuş olabilir).
    pub fn find(id: Option<&str>) -> &'static LiveBackgroundPreset {
        let all = Self::all();
        id.and_then(|id| all.iter().find(|p| text::eq_ignore_case(&p.id, id))).unwrap_or(&all[0])
    }
}

/// Bellek ve çözümleme sınırları. Tuval 620×352 (AGENTS #13) büyütülmez, çözümleme ona kenetlenir.
pub struct LiveBackgroundLimits;

impl LiveBackgroundLimits {
    /// Çözümleme genişliği (açık çentiğin genişliği); DecodePixelWidth bu değerdir.
    pub const DECODE_PIXEL_WIDTH: i32 = 620;

    /// GIF için en çok tutulan kare sayısı.
    pub const MAX_GIF_FRAMES: i32 = 120;

    /// Çözülmüş karelerin toplam bellek sınırı (bayt). 620×352×4 ≈ 0,87 MB/kare: yaklaşık 55 kare.
    pub const MAX_DECODED_BYTES: i64 = 48 * 1024 * 1024;

    /// Kaynağın çözülmemiş (tam çözünürlük, kare başına 4 bayt) toplam boyutu için üst sınır. GIF kareleri düşük sıkıştırmalı
    /// dosyalarda bile çok daha büyük açılabilir; 64 MB'lık bir dosya bile gigabaytlarca kare üretebilir, bu yüzden kare sayısı ve
    /// çözünürlük birlikte sınırlanır.
    pub const MAX_SOURCE_DECODED_BYTES: i64 = 128 * 1024 * 1024;

    /// Kaynak çözülebilir mi: ölçüler geçerli ve ham kare belleği sınırın içinde ([`Self::MAX_SOURCE_DECODED_BYTES`]).
    pub fn is_source_within_budget(source_width: i32, source_height: i32, frame_count: i32) -> bool {
        if source_width <= 0 || source_height <= 0 || frame_count <= 0 {
            return false;
        }
        // double: uç değerlerde long taşması sınırı aşan girdiyi geçerli gösterirdi.
        source_width as f64 * source_height as f64 * 4.0 * frame_count as f64 <= Self::MAX_SOURCE_DECODED_BYTES as f64
    }

    /// Yerel dosya boyutu sınırı (bayt).
    pub const MAX_FILE_SIZE_BYTES: i64 = 64 * 1024 * 1024;

    pub const MIN_SCRIM_PERCENT: i32 = 30;
    pub const MAX_SCRIM_PERCENT: i32 = 90;
    pub const DEFAULT_SCRIM_PERCENT: i32 = 65;

    /// Koyu perde yüzdesini [30, 90] aralığına kenetler: perdesiz arka planda yazı okunmaz.
    pub fn clamp_scrim_percent(percent: i32) -> i32 {
        text::clamp(percent, Self::MIN_SCRIM_PERCENT, Self::MAX_SCRIM_PERCENT)
    }

    /// Perde opaklığı 0–1.
    pub fn scrim_opacity(percent: i32) -> f64 {
        Self::clamp_scrim_percent(percent) as f64 / 100.0
    }

    /// Kaynak boyutundan çözümleme boyutu: genişlik en çok 620 (küçük kaynak büyütülmez), oran korunur.
    /// Geçersiz (≤ 0) kaynak (0, 0) döner.
    pub fn decode_size(source_width: i32, source_height: i32) -> (i32, i32) {
        if source_width <= 0 || source_height <= 0 {
            return (0, 0);
        }

        let width = i32::min(source_width, Self::DECODE_PIXEL_WIDTH);
        let height = f64::max(1.0, text::round(source_height as f64 * width as f64 / source_width as f64)) as i32;
        (width, height)
    }
}

/// GIF karelerinin sınırlanması: kare sayısı ya da toplam bayt sınırı aşılırsa her N. kare tutulur, atlanan karelerin
/// süresi tutulan kareye eklenir (toplam süre ve hız bozulmaz).
pub struct GifFramePlan;

impl GifFramePlan {
    /// Tarayıcıların yaptığı gibi: 0–1 santisaniyelik gecikme 100 ms sayılır (aksi halde GIF çılgınca hızlı oynar).
    pub fn normalize_delay_ms(delay_centiseconds: i32) -> i32 {
        if delay_centiseconds <= 1 { 100 } else { delay_centiseconds * 10 }
    }

    /// Kaç karede bir kare tutulacak (1 = hepsi). Bayt sınırı kare başına 4 bayt (Pbgra32) üzerinden hesaplanır.
    pub fn stride(frame_count: i32, pixel_width: i32, pixel_height: i32) -> i32 {
        if frame_count <= 0 || pixel_width <= 0 || pixel_height <= 0 {
            return 1;
        }

        let bytes_per_frame = pixel_width as i64 * pixel_height as i64 * 4;
        let by_bytes = i64::max(1, i64::min(i32::MAX as i64, LiveBackgroundLimits::MAX_DECODED_BYTES / bytes_per_frame)) as i32;
        let cap = i32::min(LiveBackgroundLimits::MAX_GIF_FRAMES, by_bytes);
        if frame_count <= cap { 1 } else { (frame_count as f64 / cap as f64).ceil() as i32 }
    }

    /// Tutulacak kare sayısı.
    pub fn kept_frames(frame_count: i32, pixel_width: i32, pixel_height: i32) -> i32 {
        if frame_count <= 0 {
            return 0;
        }
        let stride = Self::stride(frame_count, pixel_width, pixel_height);
        (frame_count as f64 / stride as f64).ceil() as i32
    }

    /// Bu indeksteki kare bir tutulan grubun sonu mu (grubun toplam süresiyle birlikte yayımlanır).
    pub fn is_group_end(index: i32, frame_count: i32, stride: i32) -> bool {
        index == frame_count - 1 || (index + 1) % i32::max(1, stride) == 0
    }
}

/// Oynatma kararı: ne zaman hareket eder, ne zaman yalnız ilk kare, ne zaman hiç.
pub struct LiveBackgroundPlaybackPolicy;

impl LiveBackgroundPlaybackPolicy {
    /// Hedef bu yüzeyde arka plan istiyor mu.
    pub fn applies_to(target: LiveBackgroundTarget, surface: LiveBackgroundSurface) -> bool {
        match (target, surface) {
            (LiveBackgroundTarget::Both, _) => true,
            (LiveBackgroundTarget::Notch, LiveBackgroundSurface::Notch) => true,
            (LiveBackgroundTarget::MusicCard, LiveBackgroundSurface::MusicCard) => true,
            _ => false,
        }
    }

    /// Hareket etmeli mi. Yalnız çentik açıkken (Expanded) ve pencere görünürken; hareketi azalt ya da pil tasarrufunda
    /// asla. Mevcut `BackgroundAnimationPolicy` kararına devreder (tek kural kaynağı): açık çentik "boşta
    /// değil", pil tasarrufu açıkken tercih verilmediği için hareket yok. Hedef `None` ise false.
    pub fn should_animate(
        is_notch_expanded: bool,
        is_window_visible: bool,
        reduce_motion: bool,
        is_battery_saver: bool,
        target: LiveBackgroundTarget,
    ) -> bool {
        // `Enum.IsDefined(target)` Rust'ta her zaman doğru.
        if target == LiveBackgroundTarget::None {
            return false;
        }

        let state = BackgroundPlaybackState::new(
            !is_notch_expanded,
            is_battery_saver,
            false,
            is_window_visible,
            if reduce_motion { 0.0 } else { 1.0 },
        );

        // Tür fark etmez: ambiyans, GIF ve video aynı kurala bağlıdır (hareketli tür).
        BackgroundAnimationPolicy::should_animate_default(BackgroundKind::AnimatedImage, state)
    }

    /// Yüzey için tam karar. Hedef bu yüzeyi içermiyorsa, çentik kapalıysa ya da pencere görünmüyorsa `Off`
    /// (kare ve çözülmüş bellek serbest bırakılır). Açık ve görünürken hareketi azalt / pil tasarrufu varsa
    /// `StaticFirstFrame`, yoksa `Animate`.
    pub fn decide(
        is_notch_expanded: bool,
        is_window_visible: bool,
        reduce_motion: bool,
        is_battery_saver: bool,
        target: LiveBackgroundTarget,
        surface: LiveBackgroundSurface,
    ) -> LiveBackgroundPlaybackMode {
        if !Self::applies_to(target, surface) || !is_notch_expanded || !is_window_visible {
            return LiveBackgroundPlaybackMode::Off;
        }

        if Self::should_animate(is_notch_expanded, is_window_visible, reduce_motion, is_battery_saver, target) {
            LiveBackgroundPlaybackMode::Animate
        } else {
            LiveBackgroundPlaybackMode::StaticFirstFrame
        }
    }
}

/// Yerel dosya doğrulama sonucu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(i32)]
pub enum LiveBackgroundFileError {
    #[default]
    None = 0,
    MissingPath = 1,
    UnsupportedExtension = 2,
    NotLocalPath = 3,
    InvalidPath = 4,
    Empty = 5,
    TooLarge = 6,
}

/// Yerel dosya doğrulayıcı. Dosya sistemine dokunmaz: boyutu çağıran verir. Yol denetimi (UNC, URL, göreli yol,
/// aygıt adı, alternatif veri akışı reddi) burada yapılır (C# yorumu `BackgroundSourceValidator`'a devrediyordu).
pub struct LiveBackgroundFileValidator;

const DEVICE_NAMES: [&str; 32] = [
    "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT0", "LPT1", "LPT2", "LPT3",
    "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "COM¹", "COM²", "COM³", "LPT¹", "LPT²", "LPT³", "CONIN$", "CONOUT$",
];

impl LiveBackgroundFileValidator {
    /// Kabul edilen uzantılar (küçük harf, noktalı).
    pub fn allowed_extensions() -> &'static [&'static str] {
        &[".gif", ".mp4", ".webm", ".webp"]
    }

    /// OpenFileDialog süzgeci ("*.gif;*.mp4;*.webm;*.webp").
    pub fn dialog_pattern() -> String {
        Self::allowed_extensions().iter().map(|e| format!("*{e}")).collect::<Vec<_>>().join(";")
    }

    pub const MAX_PATH_LENGTH: usize = 259;

    /// Dosya yolunu ve boyutunu denetler; ilk hatayı döner.
    pub fn validate(path: Option<&str>, file_size_bytes: i64) -> LiveBackgroundFileError {
        if text::is_null_or_white_space(path) {
            return LiveBackgroundFileError::MissingPath;
        }
        let path = path.unwrap();

        let extension = Self::extension_of(path);
        if !extension.as_deref().is_some_and(|e| Self::allowed_extensions().contains(&e)) {
            return LiveBackgroundFileError::UnsupportedExtension;
        }

        // C# `string.Length` UTF-16 birimi sayar.
        if path.encode_utf16().count() > Self::MAX_PATH_LENGTH {
            return LiveBackgroundFileError::InvalidPath;
        }

        // C# `char` UTF-16 birimidir: BMP dışı karakterler vekil çift olduğundan Control/Format sayılmaz.
        if path.chars().any(|c| (c as u32) <= 0xFFFF && (c.is_control() || is_format_char(c))) {
            return LiveBackgroundFileError::InvalidPath;
        }

        if path.contains("://") || path.starts_with("\\\\") || !is_absolute_windows_path(path) {
            return LiveBackgroundFileError::NotLocalPath;
        }

        // Mutlak yol denetiminden geçti: ilk üç karakter ASCII, bayt dizini = UTF-16 dizini.
        if path[2..].contains(':') {
            return LiveBackgroundFileError::InvalidPath;
        }

        if path.contains(['<', '>', '"', '|', '?', '*']) {
            return LiveBackgroundFileError::InvalidPath;
        }

        let segments: Vec<&str> = path[3..].split('\\').collect();
        if segments.iter().any(|s| is_non_canonical_segment(s)) || segments.iter().any(|s| is_reserved_device_name(s)) {
            return LiveBackgroundFileError::InvalidPath;
        }

        if file_size_bytes <= 0 {
            return LiveBackgroundFileError::Empty;
        }
        if file_size_bytes > LiveBackgroundLimits::MAX_FILE_SIZE_BYTES {
            return LiveBackgroundFileError::TooLarge;
        }

        LiveBackgroundFileError::None
    }

    /// Yolun uzantısı (küçük harf, noktalı); yoksa `None`.
    pub fn extension_of(path: &str) -> Option<String> {
        let last_dot = path.rfind('.');
        let last_separator = [path.rfind('\\'), path.rfind('/')].into_iter().flatten().max();
        let last_dot = last_dot?;
        if last_separator.is_some_and(|s| last_dot < s) || last_dot == path.len() - 1 {
            return None;
        }
        Some(text::to_lower_invariant(&path[last_dot..]))
    }
}

fn is_non_canonical_segment(segment: &str) -> bool {
    segment.is_empty() || segment == "." || segment == ".." || segment.ends_with('.') || segment.ends_with(' ')
}

fn is_reserved_device_name(segment: &str) -> bool {
    let stem = text::to_upper_invariant(segment.split('.').next().unwrap_or("").trim_end_matches(' '));
    DEVICE_NAMES.contains(&stem.as_str())
}

fn is_absolute_windows_path(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\' && !path.contains('/')
}

/// `UnicodeCategory.Format` (Cf), yalnız BMP (UTF-16 tek birim) için.
fn is_format_char(c: char) -> bool {
    matches!(c as u32,
        0x00AD | 0x0600..=0x0605 | 0x061C | 0x06DD | 0x070F | 0x0890..=0x0891 | 0x08E2 | 0x180E
        | 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2064 | 0x2066..=0x206F | 0xFEFF | 0xFFF9..=0xFFFB)
}

/// Çizilecek içerik türü (ayardan çözülmüş).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(i32)]
pub enum LiveBackgroundContentKind {
    #[default]
    None = 0,
    Ambient = 1,
    AnimatedImage = 2,
    StillImage = 3,
    Video = 4,
}

/// Çözülmüş seçim: App bunu olduğu gibi çizer, karar vermez.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveBackgroundSelection {
    /// İçerik türü; `None` ise hiçbir şey çizilmez.
    pub kind: LiveBackgroundContentKind,
    /// Yalnız ambiyansta.
    pub preset: Option<LiveBackgroundPreset>,
    /// Yalnız yerel dosyada.
    pub file_path: Option<String>,
    /// Yerel dosya geçersizse nedeni (arayüzde gösterilir).
    pub error: LiveBackgroundFileError,
}

impl LiveBackgroundSelection {
    /// `LiveBackgroundSelection.None`.
    pub fn none() -> Self {
        LiveBackgroundSelection { kind: LiveBackgroundContentKind::None, preset: None, file_path: None, error: LiveBackgroundFileError::None }
    }
}

/// Ayar değerlerinden içerik seçimini çözer (saf, dosya sistemine dokunmaz).
pub struct LiveBackgroundResolver;

impl LiveBackgroundResolver {
    pub fn resolve(
        target: LiveBackgroundTarget,
        source: LiveBackgroundSourceKind,
        preset_id: Option<&str>,
        file_path: Option<&str>,
        file_size_bytes: i64,
    ) -> LiveBackgroundSelection {
        if target == LiveBackgroundTarget::None {
            return LiveBackgroundSelection::none();
        }

        if source == LiveBackgroundSourceKind::BuiltInAmbient {
            return LiveBackgroundSelection {
                kind: LiveBackgroundContentKind::Ambient,
                preset: Some(LiveBackgroundPresets::find(preset_id).clone()),
                file_path: None,
                error: LiveBackgroundFileError::None,
            };
        }

        // `source != LocalFile` kolu Rust'ta ulaşılamaz (enum yalnız iki değer); C# tanımsız sayı için None dönerdi.

        let error = LiveBackgroundFileValidator::validate(file_path, file_size_bytes);
        if error != LiveBackgroundFileError::None {
            return LiveBackgroundSelection { kind: LiveBackgroundContentKind::None, preset: None, file_path: None, error };
        }

        let path = file_path.unwrap();
        let kind = match LiveBackgroundFileValidator::extension_of(path).as_deref() {
            Some(".gif") => LiveBackgroundContentKind::AnimatedImage,
            Some(".mp4") | Some(".webm") => LiveBackgroundContentKind::Video,
            _ => LiveBackgroundContentKind::StillImage,
        };
        LiveBackgroundSelection { kind, preset: None, file_path: Some(path.to_string()), error: LiveBackgroundFileError::None }
    }
}
