//! Yerel (Win32) açılır menü: alt menüler, ayırıcılar, devre dışı başlık satırları. Seçim beklenir ve kimlik döner.
//! WPF `ContextMenu` karşılığı olarak dock'un sağ tık menülerinde kullanılır; dışarı tıklama ve Esc sistemce kapatır.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, HMENU, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, PostMessageW, SetForegroundWindow,
    TPM_BOTTOMALIGN, TPM_CENTERALIGN, TPM_LEFTALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TPM_TOPALIGN, TPM_VCENTERALIGN,
    TRACK_POPUP_MENU_FLAGS, TrackPopupMenuEx, WM_NULL,
};
use windows::core::HSTRING;

/// Menü satırı.
#[derive(Debug, Clone)]
pub enum MenuEntry {
    Item { id: u32, label: String, enabled: bool },
    Separator,
    /// Tıklanamaz bölüm başlığı (atlama listesi bölümleri).
    Header(String),
    Submenu { label: String, items: Vec<MenuEntry> },
}

impl MenuEntry {
    pub fn item(id: u32, label: impl Into<String>) -> Self {
        MenuEntry::Item { id, label: label.into(), enabled: true }
    }
}

/// Menünün nokta ile ilişkisi (dock kenarına göre: alttaki dock'ta menü noktanın üstüne açılır).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAnchor {
    Above,
    Below,
    Right,
    Left,
}

fn escape(label: &str) -> String {
    // "&" kısayol imi sayılmasın.
    label.replace('&', "&&")
}

fn build(items: &[MenuEntry]) -> windows::core::Result<HMENU> {
    // SAFETY: menü tanıtıcıları bu işlevde oluşturulur; alt menüler üst menüye bağlanınca onunla birlikte yok edilir.
    unsafe {
        let menu = CreatePopupMenu()?;
        for e in items {
            match e {
                MenuEntry::Item { id, label, enabled } => {
                    let flags = if *enabled { MF_STRING } else { MF_STRING | MF_GRAYED };
                    AppendMenuW(menu, flags, *id as usize, &HSTRING::from(escape(label)))?;
                }
                MenuEntry::Separator => AppendMenuW(menu, MF_SEPARATOR, 0, None)?,
                MenuEntry::Header(label) => AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, &HSTRING::from(escape(label)))?,
                MenuEntry::Submenu { label, items } => {
                    let sub = build(items)?;
                    AppendMenuW(menu, MF_POPUP, sub.0 as usize, &HSTRING::from(escape(label)))?;
                }
            }
        }
        Ok(menu)
    }
}

/// Menüyü ekran noktasında (piksel) açar ve seçimi bekler. Seçilen kimlik; vazgeçildiyse `None`.
pub fn show(owner: isize, x: i32, y: i32, anchor: MenuAnchor, items: &[MenuEntry]) -> Option<u32> {
    if items.is_empty() {
        return None;
    }
    let menu = build(items).ok()?;
    let align: TRACK_POPUP_MENU_FLAGS = match anchor {
        MenuAnchor::Above => TPM_CENTERALIGN | TPM_BOTTOMALIGN,
        MenuAnchor::Below => TPM_CENTERALIGN | TPM_TOPALIGN,
        MenuAnchor::Right => TPM_LEFTALIGN | TPM_VCENTERALIGN,
        MenuAnchor::Left => windows::Win32::UI::WindowsAndMessaging::TPM_RIGHTALIGN | TPM_VCENTERALIGN,
    };
    let hwnd = HWND(owner as *mut _);
    // SAFETY: menü bu işlevde yaratılıp yok edilir. Sahibi öne almak menünün dışarı tıklamada kapanması için gerekir
    // (TrackPopupMenu belgelenmiş davranışı); sonrasındaki WM_NULL menü döngüsünü temizler.
    let cmd = unsafe {
        let _ = SetForegroundWindow(hwnd);
        let r = TrackPopupMenuEx(menu, (align | TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON).0, x, y, hwnd, None);
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        r.0 as u32
    };
    (cmd != 0).then_some(cmd)
}
