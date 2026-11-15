//! Karşılığı: `Shortcuts/Hotkey.cs`.

use std::fmt;
use std::ops::{BitAnd, BitOr, BitOrAssign, Not};

use serde::{Deserialize, Serialize};

use crate::text;

/// Win32 MOD_* bayraklarıyla aynı değerler (RegisterHotKey doğrudan kullanır). C#'ta `[Flags] enum`;
/// Rust'ta bayrak kümesi olarak `i32` sarmalayıcı (`HotkeyModifiers::Control | HotkeyModifiers::Alt`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HotkeyModifiers(pub i32);

#[allow(non_upper_case_globals)]
impl HotkeyModifiers {
    pub const None: HotkeyModifiers = HotkeyModifiers(0);
    pub const Alt: HotkeyModifiers = HotkeyModifiers(0x1);
    pub const Control: HotkeyModifiers = HotkeyModifiers(0x2);
    pub const Shift: HotkeyModifiers = HotkeyModifiers(0x4);
    pub const Win: HotkeyModifiers = HotkeyModifiers(0x8);

    /// `(m & flag) != 0`.
    pub fn intersects(self, flag: HotkeyModifiers) -> bool {
        self.0 & flag.0 != 0
    }

    /// `HasFlag`: verilen bayrakların hepsi açık mı.
    pub fn has_flag(self, flag: HotkeyModifiers) -> bool {
        self.0 & flag.0 == flag.0
    }
}

impl BitOr for HotkeyModifiers {
    type Output = HotkeyModifiers;
    fn bitor(self, rhs: HotkeyModifiers) -> HotkeyModifiers {
        HotkeyModifiers(self.0 | rhs.0)
    }
}
impl BitOrAssign for HotkeyModifiers {
    fn bitor_assign(&mut self, rhs: HotkeyModifiers) {
        self.0 |= rhs.0;
    }
}
impl BitAnd for HotkeyModifiers {
    type Output = HotkeyModifiers;
    fn bitand(self, rhs: HotkeyModifiers) -> HotkeyModifiers {
        HotkeyModifiers(self.0 & rhs.0)
    }
}
impl Not for HotkeyModifiers {
    type Output = HotkeyModifiers;
    fn not(self) -> HotkeyModifiers {
        HotkeyModifiers(!self.0)
    }
}

const NAMED_KEYS: [(&str, i32); 15] = [
    ("Space", 0x20),
    ("Enter", 0x0D),
    ("Tab", 0x09),
    ("Esc", 0x1B),
    ("Backspace", 0x08),
    ("Insert", 0x2D),
    ("Delete", 0x2E),
    ("Home", 0x24),
    ("End", 0x23),
    ("PageUp", 0x21),
    ("PageDown", 0x22),
    ("Left", 0x25),
    ("Up", 0x26),
    ("Right", 0x27),
    ("Down", 0x28),
];

/// Genel (sistem çapında) klavye kısayolu: değiştirici tuşlar + sanal tuş kodu (VK_*).
/// Metin biçimi "Ctrl+Alt+N": ayarlarda saklanır, arayüzde gösterilir. `virtual_key` 0 ise yalnız
/// değiştiricilerden oluşur ("Win", "Shift+Win").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Hotkey {
    pub modifiers: HotkeyModifiers,
    pub virtual_key: i32,
}

impl Hotkey {
    pub const fn new(modifiers: HotkeyModifiers, virtual_key: i32) -> Self {
        Hotkey { modifiers, virtual_key }
    }

    /// Ana tuşu yok, yalnız değiştiricilerden oluşuyor (tek başına Win gibi).
    pub fn is_modifier_only(&self) -> bool {
        self.virtual_key == 0 && self.modifiers != HotkeyModifiers::None
    }

    /// Kaydedilebilir mi: yalnız değiştiriciler ya da adlandırılabilir bir tuş.
    pub fn is_valid(&self) -> bool {
        self.is_modifier_only() || Self::name_of(self.virtual_key).is_some()
    }

    /// Bu kısayol, yazı yazarken kullanılan bir tuşu bütün uygulamalarda yakalar (uyarı gösterilir).
    pub fn captures_typing(&self) -> bool {
        (self.modifiers & !HotkeyModifiers::Shift) == HotkeyModifiers::None
            && matches!(self.virtual_key, 0x41..=0x5A | 0x30..=0x39 | 0x20 | 0x0D | 0x09)
    }

    /// Tuşun ayarlarda yazılan adı; desteklenmeyen tuş (değiştirici tuşlar dahil) için `None`.
    pub fn name_of(vk: i32) -> Option<String> {
        if (0x41..=0x5A).contains(&vk) || (0x30..=0x39).contains(&vk) {
            return Some(((vk as u8) as char).to_string());
        }
        if (0x70..=0x87).contains(&vk) {
            return Some(format!("F{}", vk - 0x70 + 1));
        }

        for (name, key) in NAMED_KEYS {
            if key == vk {
                return Some(name.to_string());
            }
        }

        None
    }

    fn virtual_key_of(name: &str) -> Option<i32> {
        let chars: Vec<char> = name.chars().collect();
        // C# `string.Length` UTF-16 birimidir; BMP dışı karakter 2 birim sayılır.
        let len16 = name.encode_utf16().count();

        if len16 == 1 {
            let c = text::to_upper_invariant(name).chars().next().unwrap_or(chars[0]);
            if c.is_ascii_uppercase() || c.is_ascii_digit() {
                return Some(c as i32);
            }
        }

        if (len16 == 2 || len16 == 3) && (chars[0] == 'F' || chars[0] == 'f') {
            // `int.TryParse(..., NumberStyles.None)`: yalnız ASCII rakamlar.
            let rest = &name[1..];
            if !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()) {
                if let Ok(n) = rest.parse::<i32>() {
                    if (1..=24).contains(&n) {
                        return Some(0x70 + n - 1);
                    }
                }
            }
        }

        for (key, vk) in NAMED_KEYS {
            if text::eq_ignore_case(key, name) {
                return Some(vk);
            }
        }

        match text::to_lower_invariant(name).as_str() {
            "escape" => Some(0x1B),
            "return" => Some(0x0D),
            "del" => Some(0x2E),
            "ins" => Some(0x2D),
            "pgup" => Some(0x21),
            "pgdn" => Some(0x22),
            _ => None,
        }
    }

    /// "Ctrl+Alt+N" gibi metni çözer. Geçersiz ya da kaydedilemeyecek bileşimlerde `None` döner.
    /// `text` hem `&str` hem `Option<&str>` olabilir (C#'ta `string?`; `None`/boş/boşluk → `None`).
    pub fn try_parse<'a>(input: impl Into<Option<&'a str>>) -> Option<Hotkey> {
        let input: Option<&str> = input.into();
        if text::is_null_or_white_space(input) {
            return None;
        }
        let input = input?;

        let mut modifiers = HotkeyModifiers::None;
        let mut key: Option<i32> = None;

        for part in input.split('+').map(text::trim) {
            if part.is_empty() {
                return None;
            }

            match text::to_lower_invariant(part).as_str() {
                "ctrl" | "control" => modifiers |= HotkeyModifiers::Control,
                "alt" => modifiers |= HotkeyModifiers::Alt,
                "shift" => modifiers |= HotkeyModifiers::Shift,
                "win" | "windows" => modifiers |= HotkeyModifiers::Win,
                _ => {
                    // Tek bir ana tuş olabilir.
                    if key.is_some() {
                        return None;
                    }
                    key = Self::virtual_key_of(part);
                    key?;
                }
            }
        }

        if key.is_none() && modifiers == HotkeyModifiers::None {
            return None;
        }

        let candidate = Hotkey::new(modifiers, key.unwrap_or(0));
        if !candidate.is_valid() {
            return None;
        }

        Some(candidate)
    }

    /// Kanonik metin: Ctrl+Alt+Shift+Win+Tuş sırası.
    pub fn format(&self) -> String {
        let mut parts: Vec<String> = Vec::with_capacity(5);
        if self.modifiers.intersects(HotkeyModifiers::Control) {
            parts.push("Ctrl".into());
        }
        if self.modifiers.intersects(HotkeyModifiers::Alt) {
            parts.push("Alt".into());
        }
        if self.modifiers.intersects(HotkeyModifiers::Shift) {
            parts.push("Shift".into());
        }
        if self.modifiers.intersects(HotkeyModifiers::Win) {
            parts.push("Win".into());
        }
        if self.virtual_key != 0 {
            parts.push(Self::name_of(self.virtual_key).unwrap_or_else(|| format!("0x{:02X}", self.virtual_key)));
        }
        parts.join("+")
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format())
    }
}
