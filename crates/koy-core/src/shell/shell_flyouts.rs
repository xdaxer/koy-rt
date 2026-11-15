use crate::text;

/// Windows kabuğunun görev çubuğuna bağlı açılır pencereleri (Başlat, Arama, bildirim merkezi / hızlı ayarlar).
/// Bunlar ön plana gelince dock "üstüne binen pencere" gibi çekilmez; Windows görev çubuğu gibi o ekranda görünür kalır.
pub struct ShellFlyouts;

const FLYOUT_PROCESSES: [&str; 4] = [
    "StartMenuExperienceHost", // Başlat menüsü
    "SearchHost",              // Windows 11 arama
    "SearchApp",               // eski arama süreci
    "ShellExperienceHost",     // bildirim merkezi, hızlı ayarlar, takvim açılır penceresi
];

impl ShellFlyouts {
    /// Süreç adı (uzantısız, `Process.ProcessName`) kabuk açılır penceresine mi ait.
    pub fn is_flyout_process(process_name: Option<&str>) -> bool {
        if text::is_null_or_white_space(process_name) {
            return false;
        }
        let process_name = process_name.unwrap_or_default();

        FLYOUT_PROCESSES.iter().any(|name| text::eq_ignore_case(name, process_name))
    }
}
