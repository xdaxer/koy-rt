//! QueryDisplayConfig + DisplayConfigGetDeviceInfo ile her etkin ekranın `monitorDevicePath` (kararlı kimlik) ve
//! `monitorFriendlyDeviceName` (dostça ad) bilgisini okur. Sonuç Windows aygıt adına (`\\.\DISPLAY2`) göre eşlenir;
//! çünkü EnumDisplayMonitors yalnız o adı verir. Yalnız okur, hiçbir ayarı değiştirmez.

use std::collections::HashMap;

use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes,
    QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};

/// Bir monitörün DisplayConfig'ten okunan kararlı kimliği ve dostça adı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DisplayConfigInfo {
    pub device_path: String,
    pub friendly_name: String,
    pub is_internal: bool,
}

fn wide(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

pub(crate) struct DisplayConfigReader;

impl DisplayConfigReader {
    /// GDI aygıt adı (anahtar büyük harfli: OrdinalIgnoreCase) → kimlik ve ad. API hata verirse boş sözlük döner.
    pub(crate) fn read() -> HashMap<String, DisplayConfigInfo> {
        let mut result = HashMap::new();
        // Ekran listesi sorgu ile okuma arasında değişebilir (ERROR_INSUFFICIENT_BUFFER); birkaç kez dene.
        for _ in 0..3 {
            let (mut path_count, mut mode_count) = (0u32, 0u32);
            // SAFETY: sayılar yerel değişkenlere yazılır; diziler bu boyutlarla ayrılır.
            unsafe {
                if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count) != ERROR_SUCCESS {
                    return result;
                }
                let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
                let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
                let status = QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut path_count, paths.as_mut_ptr(), &mut mode_count, modes.as_mut_ptr(), None);
                if status == ERROR_INSUFFICIENT_BUFFER {
                    continue;
                }
                if status != ERROR_SUCCESS {
                    return result;
                }

                for path in paths.iter().take(path_count as usize) {
                    let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
                    source.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
                    source.header.size = std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
                    source.header.adapterId = path.sourceInfo.adapterId;
                    source.header.id = path.sourceInfo.id;
                    if DisplayConfigGetDeviceInfo(&mut source.header) != 0 {
                        continue;
                    }

                    let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
                    target.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
                    target.header.size = std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
                    target.header.adapterId = path.targetInfo.adapterId;
                    target.header.id = path.targetInfo.id;
                    if DisplayConfigGetDeviceInfo(&mut target.header) != 0 {
                        continue;
                    }

                    let gdi_name = wide(&source.viewGdiDeviceName);
                    if gdi_name.is_empty() {
                        continue;
                    }
                    // Dahili (dizüstü paneli): INTERNAL (0x80000000), DISPLAYPORT_EMBEDDED (11), UDI_EMBEDDED (13).
                    let tech = target.outputTechnology.0;
                    let is_internal = tech == i32::MIN || tech == 11 || tech == 13;
                    result.insert(
                        koy_core::text::to_upper_invariant(&gdi_name),
                        DisplayConfigInfo { device_path: wide(&target.monitorDevicePath), friendly_name: wide(&target.monitorFriendlyDeviceName), is_internal },
                    );
                }
            }
            break;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_at_least_one_active_display() {
        let map = super::DisplayConfigReader::read();
        assert!(map.keys().all(|k| k.starts_with(r"\\.\DISPLAY")));
    }
}
