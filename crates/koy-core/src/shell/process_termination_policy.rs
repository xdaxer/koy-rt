use crate::text;

/// Dock'taki "Sonlandır" eyleminin hangi süreçleri öldürmeyi reddedeceğine karar verir (saf mantık).
/// Birden çok pencerenin ya da kabuğun ortak süreci öldürülürse ilgisiz uygulamalar da kapanır:
/// Mağaza (UWP) pencereleri ApplicationFrameHost.exe çatısında durur, Gezgin pencereleri explorer.exe içindedir.
pub struct ProcessTerminationPolicy;

const PROTECTED: [&str; 10] = [
    "ApplicationFrameHost",
    "explorer",
    "dwm",
    "csrss",
    "winlogon",
    "wininit",
    "services",
    "lsass",
    "svchost",
    "System",
];

impl ProcessTerminationPolicy {
    /// `process_name`: uzantısız süreç adı (ör. "chrome"); bilinmiyorsa `None`.
    pub fn can_terminate(process_id: i32, process_name: Option<&str>, own_process_id: i32) -> bool {
        if process_id <= 0 || process_id == own_process_id {
            return false;
        }

        let Some(process_name) = process_name else {
            return false;
        };
        if text::is_null_or_white_space(Some(process_name)) {
            return false;
        }

        let name = if text::ends_with_ignore_case(process_name, ".exe") {
            // `processName[..^4]`: ".exe" son 4 karakterdir.
            let cut = process_name.char_indices().rev().nth(3).map(|(i, _)| i).unwrap_or(0);
            &process_name[..cut]
        } else {
            process_name
        };

        !PROTECTED.iter().any(|p| text::eq_ignore_case(p, name))
    }
}
