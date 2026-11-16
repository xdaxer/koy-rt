//! Canlı arka planın çentik ömrü boyunca değişmeyen seçimi (ayardan çözülmüş) ve yüzey başına anlık oynatma kararı.
//! Kararların hepsi core'dadır; burası girdileri toplar ve sonucu görünümlere iletir.
//! Karşılığı: `Features/Notch/LiveBackground/LiveBackgroundState.cs`.

use std::cell::Cell;

use koy_core::notch::{
    LiveBackgroundContentKind, LiveBackgroundFileError, LiveBackgroundFileValidator, LiveBackgroundLimits, LiveBackgroundPlaybackMode,
    LiveBackgroundPlaybackPolicy, LiveBackgroundResolver, LiveBackgroundSelection, LiveBackgroundSourceKind, LiveBackgroundSurface,
    LiveBackgroundTarget,
};
use koy_core::settings::NotchSettings;

use crate::infra::observable::Notifier;
use crate::resources::motion::Motion;

pub struct LiveBackgroundState {
    target: LiveBackgroundTarget,
    pub selection: LiveBackgroundSelection,
    /// Koyu perdenin opaklığı (0-1).
    pub scrim_opacity: f64,
    notch_mode: Cell<LiveBackgroundPlaybackMode>,
    music_card_mode: Cell<LiveBackgroundPlaybackMode>,
    pub changed: Notifier,
}

impl LiveBackgroundState {
    pub fn new(settings: &NotchSettings) -> Self {
        let target = settings.live_background_target;
        // Özellik kapalıyken dosya sistemine hiç dokunulmaz.
        let selection = if target == LiveBackgroundTarget::None { LiveBackgroundSelection::none() } else { resolve(settings) };
        LiveBackgroundState {
            target,
            selection,
            scrim_opacity: LiveBackgroundLimits::scrim_opacity(settings.live_background_scrim_percent),
            notch_mode: Cell::new(LiveBackgroundPlaybackMode::Off),
            music_card_mode: Cell::new(LiveBackgroundPlaybackMode::Off),
            changed: Notifier::default(),
        }
    }

    /// Açık çentiğin zemini için karar.
    pub fn notch_mode(&self) -> LiveBackgroundPlaybackMode {
        self.notch_mode.get()
    }

    /// Müzik kartının zemini için karar.
    pub fn music_card_mode(&self) -> LiveBackgroundPlaybackMode {
        self.music_card_mode.get()
    }

    /// Durum değişince (çentik açıldı/kapandı, görünürlük, pil) çağrılır.
    pub fn update(&self, is_notch_expanded: bool, is_window_visible: bool, is_battery_saver: bool) {
        let (n, m) = if self.selection.kind == LiveBackgroundContentKind::None {
            (LiveBackgroundPlaybackMode::Off, LiveBackgroundPlaybackMode::Off)
        } else {
            let reduce_motion = !Motion::enabled();
            let decide = |s| LiveBackgroundPlaybackPolicy::decide(is_notch_expanded, is_window_visible, reduce_motion, is_battery_saver, self.target, s);
            (decide(LiveBackgroundSurface::Notch), decide(LiveBackgroundSurface::MusicCard))
        };
        let changed = self.notch_mode.replace(n) != n || self.music_card_mode.replace(m) != m;
        if changed {
            self.changed.notify();
        }
    }
}

fn resolve(s: &NotchSettings) -> LiveBackgroundSelection {
    let path = s.live_background_file_path.as_str();
    let mut size = 0i64;
    // Boyut yalnız yol metni doğrulandıktan sonra okunur: UNC / URL yoluna ağ erişimi yapılmaz.
    if s.live_background_source == LiveBackgroundSourceKind::LocalFile
        && LiveBackgroundFileValidator::validate(Some(path), 1) == LiveBackgroundFileError::None
    {
        size = file_length(path);
    }
    LiveBackgroundResolver::resolve(s.live_background_target, s.live_background_source, Some(&s.live_background_preset), Some(path), size)
}

/// Dosya boyutu; yoksa ya da okunamıyorsa 0.
pub fn file_length(path: &str) -> i64 {
    std::fs::metadata(path).map(|m| if m.is_file() { m.len() as i64 } else { 0 }).unwrap_or(0)
}
