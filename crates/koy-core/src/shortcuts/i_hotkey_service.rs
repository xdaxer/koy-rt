//! Karşılığı: `Shortcuts/IHotkeyService.cs`.

use super::hotkey::Hotkey;
use crate::event::Event;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HotkeyRegistrationResult {
    #[default]
    Success,

    /// Başka bir uygulama (ya da Windows) bu kısayolu zaten tutuyor.
    AlreadyInUse,

    /// Başka bir nedenle kaydedilemedi.
    Failed,
}

/// Sistem çapında kısayol kaydı sözleşmesi. Platform uygulaması RegisterHotKey / UnregisterHotKey kullanır.
/// C# `IDisposable` karşılığı `dispose` (birden çok çağrıya dayanıklı olmalı).
pub trait IHotkeyService: Send + Sync {
    /// Kayıtlı bir kısayola basıldı; değer `register`'a verilen kimliktir. (C#: `HotkeyPressed`)
    fn hotkey_pressed(&self) -> &Event<i32>;

    fn register(&self, id: i32, hotkey: Hotkey) -> HotkeyRegistrationResult;

    fn unregister(&self, id: i32);

    fn dispose(&self);
}
