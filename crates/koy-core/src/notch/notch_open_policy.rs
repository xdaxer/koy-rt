use super::NotchOpenBehavior;
use crate::text;

/// Çentik açılıp kapanmasını tetikleyen imleç ve sürükleme olayları.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotchPointerEvent {
    Enter,
    Leave,
    Click,
    DragEnter,
    DragLeave,
}

/// Olaydan sonra yapılacak iş: hiçbiri, açmak ya da kapatmak.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NotchOpenAction {
    #[default]
    None,
    Open,
    Close,
}

/// Karar: ne yapılacağı ve kaç ms sonra (0 = hemen).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NotchOpenDecision {
    pub action: NotchOpenAction,
    pub delay_ms: i32,
}

impl NotchOpenDecision {
    pub const fn new(action: NotchOpenAction, delay_ms: i32) -> Self {
        NotchOpenDecision { action, delay_ms }
    }
}

/// Çentik açılma kipine göre olay → karar eşlemesi. Saf mantık; zamanlayıcıyı çağıran kod kurar.
/// Kural: imleç yeniden girerse (Enter) bekleyen kapanma iptal edilmelidir; bu karar onu yeniden söylemez.
pub struct NotchOpenPolicy;

impl NotchOpenPolicy {
    pub const MAX_DELAY_MS: i32 = 2000;

    /// İmleç çıkınca kapanma gecikmesinin bugünkü değeri (`NotchController` zamanlayıcısı).
    pub const DEFAULT_CLOSE_DELAY_MS: i32 = 400;

    pub fn clamp_delay(delay_ms: i32) -> i32 {
        text::clamp(delay_ms, 0, Self::MAX_DELAY_MS)
    }

    /// Sürüklenen öğe çentiğin üstünde `elapsed_ms` süredir duruyor: çentik açılsın mı. Yalnız
    /// "sürükleyince aç" kipinde açılma gecikmesi beklenir; diğer kiplerde hemen açılır (bugünkü davranış, Raf'a bırakmak için).
    /// Sürükleme olayları zaten sürekli geldiği için zamanlayıcı gerekmez.
    pub fn drag_should_open(behavior: NotchOpenBehavior, elapsed_ms: f64, open_delay_ms: i32) -> bool {
        behavior != NotchOpenBehavior::DragOver || elapsed_ms >= Self::clamp_delay(open_delay_ms) as f64
    }

    /// Kipe özgü olan yalnız çentiği neyin açtığıdır: üzerine gelmek, tıklamak ya da sürüklemek. Açılma gecikmesi
    /// kipin kendi tetikleyicisine uygulanır (yanlış tetiklemeyi önler); tıklamada gecikme yoktur, kullanıcı bilerek tıkladı.
    /// Her kipte ortak olanlar bugünkü davranıştır: imleç (ya da sürüklenen öğe) çıkınca açık çentik kapanma gecikmesiyle
    /// kapanır; dosya ya da metin sürüklenince çentik, Raf'a bırakılabilsin diye açılır.
    pub fn decide(behavior: NotchOpenBehavior, pointer_event: NotchPointerEvent, is_open: bool, open_delay_ms: i32, close_delay_ms: i32) -> NotchOpenDecision {
        let open = Self::clamp_delay(open_delay_ms);
        let close = Self::clamp_delay(close_delay_ms);

        match pointer_event {
            NotchPointerEvent::Leave | NotchPointerEvent::DragLeave if is_open => NotchOpenDecision::new(NotchOpenAction::Close, close),
            NotchPointerEvent::Enter if !is_open && behavior == NotchOpenBehavior::Hover => NotchOpenDecision::new(NotchOpenAction::Open, open),
            NotchPointerEvent::Click if behavior == NotchOpenBehavior::Click => {
                NotchOpenDecision::new(if is_open { NotchOpenAction::Close } else { NotchOpenAction::Open }, 0)
            }
            NotchPointerEvent::DragEnter if !is_open => {
                NotchOpenDecision::new(NotchOpenAction::Open, if behavior == NotchOpenBehavior::DragOver { open } else { 0 })
            }
            _ => NotchOpenDecision::new(NotchOpenAction::None, 0),
        }
    }
}
