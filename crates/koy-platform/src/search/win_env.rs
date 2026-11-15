//! Karşılığı: C#'taki `Environment.GetFolderPath`, `Environment.ExpandEnvironmentVariables`,
//! `FileInfo.LastWriteTimeUtc.Ticks`, `Guid.NewGuid().ToString("N")` (ortak yardımcılar, dosya karşılığı yok).

use std::os::windows::fs::MetadataExt;

use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows::Win32::UI::Shell::{KF_FLAG_DEFAULT, SHGetKnownFolderPath};
use windows::core::{GUID, PCWSTR};

/// `DateTime.Ticks` cinsinden 1601-01-01 (FILETIME başlangıcı) ile 0001-01-01 arası tik sayısı.
const FILETIME_EPOCH_TICKS: i64 = 504_911_232_000_000_000;

/// `DateTime.FromFileTimeUtc(t).Ticks`.
pub(crate) fn filetime_to_ticks(filetime: u64) -> i64 {
    filetime as i64 + FILETIME_EPOCH_TICKS
}

/// `FileInfo.LastWriteTimeUtc.Ticks`.
pub(crate) fn last_write_utc_ticks(metadata: &std::fs::Metadata) -> i64 {
    filetime_to_ticks(metadata.last_write_time())
}

/// `Environment.GetFolderPath(...)`: bilinen klasör yolu; alınamazsa C#'taki gibi boş dize.
pub(crate) fn known_folder(id: &GUID) -> String {
    // SAFETY: id geçerli bir GUID; dönen bellek aşağıda CoTaskMemFree ile bırakılır.
    unsafe {
        match SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) {
            Ok(path) => {
                let text = path.to_string().unwrap_or_default();
                CoTaskMemFree(Some(path.0 as *const _));
                text
            }
            Err(_) => String::new(),
        }
    }
}

/// `Environment.ExpandEnvironmentVariables`: tanımsız değişkenler olduğu gibi kalır.
pub(crate) fn expand_environment_variables(text: &str) -> String {
    if !text.contains('%') {
        return text.to_string();
    }

    let source: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: source NUL sonlandırılmış; tampon boyutu çağrıdan dönen uzunlukla ayarlanır.
    unsafe {
        let needed = ExpandEnvironmentStringsW(PCWSTR(source.as_ptr()), None);
        if needed == 0 {
            return text.to_string();
        }
        let mut buffer = vec![0u16; needed as usize];
        let written = ExpandEnvironmentStringsW(PCWSTR(source.as_ptr()), Some(&mut buffer));
        if written == 0 || written as usize > buffer.len() {
            return text.to_string();
        }
        // `written` sondaki NUL'u içerir.
        String::from_utf16_lossy(&buffer[..(written as usize).saturating_sub(1)])
    }
}

/// `Guid.NewGuid().ToString("N")`: 32 küçük harfli onaltılık hane.
pub(crate) fn new_guid_n() -> String {
    let guid = GUID::new().unwrap_or_default();
    let tail: String = guid.data4.iter().map(|b| format!("{b:02x}")).collect();
    format!("{:08x}{:04x}{:04x}{}", guid.data1, guid.data2, guid.data3, tail)
}

/// `Environment.GetFolderPath(ApplicationData)`: `%APPDATA%` (PORTING.md kural 15).
pub(crate) fn app_data() -> String {
    std::env::var("APPDATA").unwrap_or_default()
}

/// `Environment.GetFolderPath(LocalApplicationData)`: `%LOCALAPPDATA%`.
pub(crate) fn local_app_data() -> String {
    std::env::var("LOCALAPPDATA").unwrap_or_default()
}

/// `CrashLog.Write(source, ex)`: Rust'ta tür adı/yığın izi olmadığından tür adı çağıranca verilir.
pub(crate) fn crash_log(source: &str, type_name: &str, message: &str) {
    use koy_core::diagnostics::{CrashError, CrashLog};
    CrashLog::write(source, &CrashError::exception(type_name, message, ""));
}
