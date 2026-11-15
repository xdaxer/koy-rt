//! Windows'un kayıtlı Wi-Fi profil adları (wlanapi). WinRT'nin profil listesi yalnız bağlı ağı verdiği için "kayıtlı
//! profili var mı" sorusu için tek yol bu; yalnız adlar okunur, profil XML'ine (şifre) dokunulmaz.

use std::collections::HashSet;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::WiFi::{WLAN_PROFILE_INFO_LIST, WlanCloseHandle, WlanFreeMemory, WlanGetProfileList, WlanOpenHandle};
use windows::core::GUID;

// WLAN_PROFILE_INFO_LIST: DWORD sayı, DWORD dizin, ardından WLAN_PROFILE_INFO dizisi.
// WLAN_PROFILE_INFO: WCHAR[256] ad + DWORD bayraklar.
const HEADER_SIZE: usize = 8;
const NAME_CHARS: usize = 256;
const ENTRY_SIZE: usize = NAME_CHARS * 2 + 4;
const MAX_ITEMS: i32 = 1024;

pub(crate) struct WlanProfiles;

impl WlanProfiles {
    pub(crate) fn get_names(interface_id: &GUID) -> HashSet<String> {
        let mut names = HashSet::new();
        // SAFETY: istemci tutamacı ve liste belleği her yolda serbest bırakılır; giriş sayısı sınırlıdır.
        unsafe {
            let mut negotiated = 0u32;
            let mut client = HANDLE::default();
            if WlanOpenHandle(2, None, &mut negotiated, &mut client) != 0 {
                // wlanapi kullanılamıyorsa (Wlan AutoConfig durdurulmuş) liste kayıtlı profil olmadan gösterilir.
                return names;
            }
            let mut list: *mut WLAN_PROFILE_INFO_LIST = std::ptr::null_mut();
            if WlanGetProfileList(client, interface_id, None, &mut list) == 0 && !list.is_null() {
                let bytes = list as *const u8;
                let count = (*(bytes as *const i32)).min(MAX_ITEMS).max(0) as usize;
                for i in 0..count {
                    let entry = bytes.add(HEADER_SIZE + i * ENTRY_SIZE) as *const u16;
                    let name = std::slice::from_raw_parts(entry, NAME_CHARS);
                    let length = name.iter().position(|&c| c == 0).unwrap_or(NAME_CHARS);
                    if length > 0 {
                        names.insert(String::from_utf16_lossy(&name[..length]));
                    }
                }
                WlanFreeMemory(list as *const _);
            }
            let _ = WlanCloseHandle(client, None);
        }
        names
    }
}
