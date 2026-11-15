//! Karşılığı: `Shortcuts/IKeyboardHookService.cs`.

use std::collections::HashMap;

use super::hotkey::Hotkey;
use crate::event::Event;

/// RegisterHotKey'in yapamadığı kısayollar için klavye kancası sözleşmesi: yalnız değiştiriciler (tek başına Win) ve
/// Windows'un kendi kısayolları. Kanca yalnız böyle bir kısayol atanmışken ya da yakalama sürerken kuruludur (AGENTS #5).
pub trait IKeyboardHookService: Send + Sync {
    /// Kancayla izlenen bir kısayola basıldı; değer `set_bindings`'e verilen kimliktir. (C#: `HotkeyPressed`)
    fn hotkey_pressed(&self) -> &Event<i32>;

    /// İzlenecek kısayolları değiştirir; boşsa kanca kaldırılır. Kanca kurulamazsa `false` döner.
    fn set_bindings(&self, bindings: &HashMap<i32, Hotkey>) -> bool;

    /// Bir sonraki kombinasyonu yakalar, tuşlar Windows'a gitmez. Kanca kurulamazsa `false` döner.
    fn begin_capture(&self, on_captured: Box<dyn Fn(Hotkey) + Send + Sync>) -> bool;

    fn end_capture(&self);

    fn dispose(&self);
}
