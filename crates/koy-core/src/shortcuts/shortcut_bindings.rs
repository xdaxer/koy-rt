//! Karşılığı: `Shortcuts/ShortcutBindings.cs`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::hotkey::Hotkey;
use crate::settings::AppSettings;
use crate::text;

/// Kısayolla tetiklenebilen eylemler. Sıra, sayfadaki satır sırasıdır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(i32)]
pub enum ShortcutAction {
    #[default]
    ToggleNotch = 0,
    OpenShelf = 1,
    ToggleDock = 2,
    OpenMenu = 3,
    PlayPause = 4,
    OpenSearch = 5,
    OpenSearchClipboard = 6,
    VoiceCommand = 7,
    OpenKoyWindow = 8,
}

impl ShortcutAction {
    /// C#: `(ShortcutAction)value` dönüşümünün güvenli hâli (tanımsız değer için `None`).
    pub fn from_i32(value: i32) -> Option<ShortcutAction> {
        ShortcutBindings::all().iter().copied().find(|a| *a as i32 == value)
    }
}

/// Eylem ile ayar alanı ve varsayılan tuş eşlemesi.
pub struct ShortcutBindings;

impl ShortcutBindings {
    /// C#: `All` (`Enum.GetValues`, tanım sırasıyla).
    pub fn all() -> &'static [ShortcutAction] {
        const ALL: [ShortcutAction; 9] = [
            ShortcutAction::ToggleNotch,
            ShortcutAction::OpenShelf,
            ShortcutAction::ToggleDock,
            ShortcutAction::OpenMenu,
            ShortcutAction::PlayPause,
            ShortcutAction::OpenSearch,
            ShortcutAction::OpenSearchClipboard,
            ShortcutAction::VoiceCommand,
            ShortcutAction::OpenKoyWindow,
        ];
        &ALL
    }

    /// Varsayılanlar Ctrl+Alt+harf: Windows kendi kısayollarında Win+harf kullanır (Win+D, Win+K, Win+Alt+R Game Bar...),
    /// Ctrl+Alt+harf ise sistemde ayrılmış değildir. Arama paneli sık açıldığı için tek elle basılan Win+Shift+Q'dur.
    pub fn default_text(action: ShortcutAction) -> &'static str {
        match action {
            ShortcutAction::ToggleNotch => "Ctrl+Alt+N",
            ShortcutAction::OpenShelf => "Ctrl+Alt+R",
            ShortcutAction::ToggleDock => "Ctrl+Alt+D",
            ShortcutAction::OpenMenu => "Ctrl+Alt+K",
            ShortcutAction::PlayPause => "Ctrl+Alt+Space",
            ShortcutAction::OpenSearch => "Win+Shift+Q",
            ShortcutAction::OpenSearchClipboard => "Alt+Shift+F2",
            ShortcutAction::VoiceCommand => "Ctrl+Alt+V",
            // Atanmamış gelir: bugün bağlı olmayan bir kombinasyon yeni çakışma üretmesin.
            ShortcutAction::OpenKoyWindow => "",
        }
    }

    pub fn get_text(settings: &AppSettings, action: ShortcutAction) -> String {
        let s = &settings.shortcuts;
        match action {
            ShortcutAction::ToggleNotch => s.toggle_notch.clone(),
            ShortcutAction::OpenShelf => s.open_shelf.clone(),
            ShortcutAction::ToggleDock => s.toggle_dock.clone(),
            ShortcutAction::OpenMenu => s.open_menu.clone(),
            ShortcutAction::PlayPause => s.play_pause.clone(),
            ShortcutAction::OpenSearch => s.open_search.clone(),
            ShortcutAction::OpenSearchClipboard => s.open_search_clipboard.clone(),
            ShortcutAction::VoiceCommand => s.voice_command.clone(),
            ShortcutAction::OpenKoyWindow => s.open_koy_window.clone(),
        }
    }

    /// C#: `settings with { Shortcuts = settings.Shortcuts with { X = text } }`: yeni ayar nesnesi döner.
    pub fn with_text(settings: &AppSettings, action: ShortcutAction, value: &str) -> AppSettings {
        let mut result = settings.clone();
        let s = &mut result.shortcuts;
        let text = value.to_string();
        match action {
            ShortcutAction::ToggleNotch => s.toggle_notch = text,
            ShortcutAction::OpenShelf => s.open_shelf = text,
            ShortcutAction::ToggleDock => s.toggle_dock = text,
            ShortcutAction::OpenMenu => s.open_menu = text,
            ShortcutAction::PlayPause => s.play_pause = text,
            ShortcutAction::OpenSearch => s.open_search = text,
            ShortcutAction::OpenSearchClipboard => s.open_search_clipboard = text,
            ShortcutAction::VoiceCommand => s.voice_command = text,
            ShortcutAction::OpenKoyWindow => s.open_koy_window = text,
        }
        result
    }

    /// Metin, eylemin varsayılan kombinasyonu mu? Yazım sırası önemsizdir (Win+Shift+Q = Shift+Win+Q).
    pub fn is_default(action: ShortcutAction, input: &str) -> bool {
        let default_text = Self::default_text(action);
        if text::is_null_or_white_space(Some(default_text)) {
            text::is_null_or_white_space(Some(input))
        } else {
            match (Hotkey::try_parse(input), Hotkey::try_parse(default_text)) {
                (Some(current), Some(fallback)) => current == fallback,
                _ => false,
            }
        }
    }

    /// Bütün eylemlerin ayardaki metni (boş metin = atanmamış).
    pub fn read_all(settings: &AppSettings) -> HashMap<ShortcutAction, String> {
        Self::all().iter().map(|&a| (a, Self::get_text(settings, a))).collect()
    }
}
