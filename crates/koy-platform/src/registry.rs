//! Kayıt defteri okuma yardımcıları (C# `Microsoft.Win32.Registry` karşılığı, yalnız okuma).

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, REG_VALUE_TYPE, RRF_RT_REG_DWORD, RRF_RT_REG_EXPAND_SZ,
    RRF_RT_REG_SZ, RRF_NOEXPAND, RegGetValueW, RRF_RT_REG_BINARY, RRF_RT_ANY, REG_SZ, RegSetKeyValueW, RegDeleteKeyValueW,
};
use windows::core::{HSTRING, PCWSTR};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hive {
    CurrentUser,
    LocalMachine,
    ClassesRoot,
}

impl Hive {
    fn key(self) -> HKEY {
        match self {
            Hive::CurrentUser => HKEY_CURRENT_USER,
            Hive::LocalMachine => HKEY_LOCAL_MACHINE,
            Hive::ClassesRoot => HKEY_CLASSES_ROOT,
        }
    }
}

/// Dize değeri (REG_SZ; REG_EXPAND_SZ genişletilmeden, C# `GetValue` varsayılanı genişletir: `expand` ile seçilir).
/// `name` `None` ise anahtarın varsayılan değeri.
pub fn get_string(hive: Hive, subkey: &str, name: Option<&str>, expand: bool) -> Option<String> {
    let subkey = HSTRING::from(subkey);
    let name_h = name.map(HSTRING::from);
    let name_p = name_h.as_ref().map_or(PCWSTR::null(), |h| PCWSTR(h.as_ptr()));
    let flags = if expand { RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ } else { RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND };
    let mut size: u32 = 0;
    let mut kind = REG_VALUE_TYPE(0);
    // SAFETY: önce boyut sorgusu (arabelleksiz).
    let status = unsafe { RegGetValueW(hive.key(), &subkey, name_p, flags, Some(&mut kind), None, Some(&mut size)) };
    if status != ERROR_SUCCESS || size == 0 {
        return None;
    }
    let mut buf = vec![0u16; size as usize / 2 + 1];
    let mut len = (buf.len() * 2) as u32;
    // SAFETY: arabellek `len` bayt yazılabilir.
    let status = unsafe {
        RegGetValueW(hive.key(), &subkey, name_p, flags, Some(&mut kind), Some(buf.as_mut_ptr() as *mut _), Some(&mut len))
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let chars = (len as usize / 2).min(buf.len());
    let text = String::from_utf16_lossy(&buf[..chars]);
    Some(text.trim_end_matches('\0').to_string())
}

/// DWORD değeri.
pub fn get_dword(hive: Hive, subkey: &str, name: &str) -> Option<u32> {
    let subkey = HSTRING::from(subkey);
    let name = HSTRING::from(name);
    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: değer u32'ye yazılır.
    let status = unsafe {
        RegGetValueW(hive.key(), &subkey, &name, RRF_RT_REG_DWORD, None, Some(&mut value as *mut u32 as *mut _), Some(&mut size))
    };
    (status == ERROR_SUCCESS).then_some(value)
}

/// İkili değer (REG_BINARY).
pub fn get_binary(hive: Hive, subkey: &str, name: &str) -> Option<Vec<u8>> {
    let subkey = HSTRING::from(subkey);
    let name = HSTRING::from(name);
    let mut size: u32 = 0;
    // SAFETY: önce boyut sorgusu, sonra o boyutta arabellek.
    unsafe {
        if RegGetValueW(hive.key(), &subkey, &name, RRF_RT_REG_BINARY, None, None, Some(&mut size)) != ERROR_SUCCESS {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        if RegGetValueW(hive.key(), &subkey, &name, RRF_RT_REG_BINARY, None, Some(buf.as_mut_ptr() as *mut _), Some(&mut size)) != ERROR_SUCCESS {
            return None;
        }
        buf.truncate(size as usize);
        Some(buf)
    }
}

/// Değer var mı (türü ne olursa olsun; C# `GetValue(name) is not null`).
pub fn value_exists(hive: Hive, subkey: &str, name: &str) -> bool {
    let subkey = HSTRING::from(subkey);
    let name = HSTRING::from(name);
    let mut size: u32 = 0;
    // SAFETY: yalnız boyut sorgusu.
    unsafe { RegGetValueW(hive.key(), &subkey, &name, RRF_RT_ANY, None, None, Some(&mut size)) == ERROR_SUCCESS }
}

/// REG_SZ yazar (anahtar yoksa oluşturur). Başarılıysa true.
pub fn set_string(hive: Hive, subkey: &str, name: &str, value: &str) -> bool {
    let data: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: veri arabelleği boyutuyla verilir.
    unsafe {
        RegSetKeyValueW(hive.key(), &HSTRING::from(subkey), &HSTRING::from(name), REG_SZ.0, Some(data.as_ptr() as *const _), (data.len() * 2) as u32)
            == ERROR_SUCCESS
    }
}

/// Değeri siler; yoksa sessizce geçer.
pub fn delete_value(hive: Hive, subkey: &str, name: &str) {
    // SAFETY: yalnız silme çağrısı.
    let _ = unsafe { RegDeleteKeyValueW(hive.key(), &HSTRING::from(subkey), &HSTRING::from(name)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_well_known_values() {
        let root = get_string(Hive::LocalMachine, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", Some("SystemRoot"), false);
        assert!(root.is_some_and(|r| !r.is_empty()));
        assert!(get_string(Hive::CurrentUser, r"Software\Koy\DoesNotExist", Some("x"), false).is_none());
        assert!(get_dword(Hive::LocalMachine, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", "CurrentMajorVersionNumber").is_some());
    }
}
