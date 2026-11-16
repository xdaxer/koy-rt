//! Çentik özelliğinin kartları: güncelleme kartı, bildirim kartları ve sesli komut göstergesi.
//! Karşılığı: `NotchFeature.cs` içindeki `TryShowUpdateCard`, `PumpNotifications` ve `UpdateVoiceOsd` bölümleri.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use super::notch_feature::Inner;

/// Kart ve sesli komut durumu.
#[derive(Default)]
pub(crate) struct CardState {
    /// Özelliğin kendisine zayıf başvuru (zamanlayıcı ve olay geri çağrıları için).
    pub me: RefCell<Weak<Inner>>,
}

pub(super) fn start(me: &Rc<Inner>) {
    *me.cards.me.borrow_mut() = Rc::downgrade(me);
}

pub(super) fn stop(_me: &Inner) {}

pub(super) fn toggle_voice_command(_me: &Rc<Inner>) {}

pub(super) fn confirm_voice_command(_me: &Rc<Inner>) {}

pub(super) fn cancel_voice_command(_me: &Rc<Inner>) {}

pub(super) fn open_voice_reply(_me: &Rc<Inner>) {}
