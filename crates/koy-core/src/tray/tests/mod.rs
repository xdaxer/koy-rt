mod taskbar_created_policy_tests;
mod tray_balloon_tests;
mod tray_host_policy_tests;
mod tray_icon_visibility_policy_tests;
mod tray_menu_anchor_tests;
mod tray_popup_dismiss_policy_tests;
mod tray_recovery_tests;
mod tray_tests;

use std::sync::atomic::{AtomicU64, Ordering};

use super::Guid;

/// `Guid.NewGuid()` karşılığı (yalnız testler için): her çağrıda farklı, boş olmayan bir değer.
pub(super) fn new_guid() -> Guid {
    static COUNTER: AtomicU64 = AtomicU64::new(0x9E37_79B9_7F4A_7C15);
    let a = COUNTER.fetch_add(0x1234_5678_9ABC_DEF1, Ordering::SeqCst);
    let b = a.rotate_left(17) ^ 0xD6E8_FEB8_6659_FD93;
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&a.to_le_bytes());
    bytes[8..].copy_from_slice(&b.to_le_bytes());
    Guid(bytes)
}
