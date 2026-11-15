//! PKEY_AppUserModel_ID okuyucu: pencere (`SHGetPropertyStoreForWindow`) ve kısayol (IShellLink'in özellik deposu) için.

use windows::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HWND, PROPERTYKEY};
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};
use windows::core::{GUID, Interface, PWSTR};

const KEY: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3), pid: 5 };

pub struct AppUserModelIdReader;

impl AppUserModelIdReader {
    pub fn for_window(hwnd: isize) -> Option<String> {
        // SAFETY: pencere tutamacı yalnız sorguya verilir; arayüz Drop ile bırakılır.
        let store: IPropertyStore = unsafe { SHGetPropertyStoreForWindow(HWND(hwnd as *mut _)) }.ok()?;
        read(&store)
    }

    /// Paketli (MSIX / Mağaza) sürecin uygulama kimliği. Bu uygulamalar pencereye değil pakete AUMID verir
    /// (`Aile_yayıncı!Uygulama`). Paketsiz süreçte APPMODEL_ERROR_NO_APPLICATION döner: `None`.
    pub fn for_process(pid: u32) -> Option<String> {
        if pid == 0 {
            return None;
        }
        // SAFETY: sınırlı sorgu hakkıyla açılır ve kapatılır; arabellek 130 karakter (APPLICATION_USER_MODEL_ID_MAX_LENGTH).
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buffer = [0u16; 130];
            let mut length = buffer.len() as u32;
            let status = GetApplicationUserModelId(process, &mut length, Some(PWSTR(buffer.as_mut_ptr())));
            let _ = CloseHandle(process);
            if status != ERROR_SUCCESS || length < 2 {
                return None;
            }
            Some(String::from_utf16_lossy(&buffer[..length as usize - 1]))
        }
    }

    /// Kabuk kısayolu nesnesinin (ShellLink) AUMID'i; nesne IPropertyStore'u destekler.
    pub fn for_shell_link(shell_link: &windows::core::IUnknown) -> Option<String> {
        shell_link.cast::<IPropertyStore>().ok().and_then(|s| read(&s))
    }
}

/// `PKEY_*` dize değeri (yalnız VT_LPWSTR; boşsa `None`).
pub(crate) fn read_string_property(store: &IPropertyStore, key: &PROPERTYKEY, allow_bstr: bool) -> Option<String> {
    // SAFETY: PROPVARIANT her yolda temizlenir; dönüştürülen dize serbest bırakılır.
    unsafe {
        let mut value = store.GetValue(key).ok()?;
        let vt = value.Anonymous.Anonymous.vt;
        // Boş (VT_EMPTY) değerlerde dönüştürmeye çalışmak her seferinde hata verirdi.
        let result = if vt == VT_LPWSTR || (allow_bstr && vt == windows::Win32::System::Variant::VT_BSTR) {
            PropVariantToStringAlloc(&value).ok().and_then(|p| {
                let s = p.to_string().ok();
                CoTaskMemFree(Some(p.0 as *const _));
                s.filter(|s| !s.trim().is_empty())
            })
        } else {
            None
        };
        let _ = PropVariantClear(&mut value);
        result
    }
}

fn read(store: &IPropertyStore) -> Option<String> {
    read_string_property(store, &KEY, false)
}
