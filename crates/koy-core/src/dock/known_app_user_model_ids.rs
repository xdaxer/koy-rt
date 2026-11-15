use super::pinned_app_matcher::PinnedAppMatcher;
use crate::win_path;

/// Pencerelerinde açık (pencere başına) AUMID bulunmayan, Windows'un süreçten türettiği iyi bilinen sistem uygulamaları.
/// Gezgin pencereleri (CabinetWClass) AUMID özelliği taşımaz ama görev çubuğunda "Microsoft.Windows.Explorer" altında
/// gruplanır; sabitli File Explorer kısayolunun AUMID'i de budur.
pub struct KnownAppUserModelIds;

impl KnownAppUserModelIds {
    pub const EXPLORER: &'static str = "Microsoft.Windows.Explorer";

    /// Gezgin'in klasör pencereleri; kabuğun öteki pencereleri (masaüstü, görev çubuğu, açılır pencereler) bu AUMID'e girmez.
    pub fn is_explorer_folder_window_class(class_name: Option<&str>) -> bool {
        class_name == Some("CabinetWClass") || class_name == Some("ExploreWClass")
    }

    /// Süreç yolu ve pencere sınıfından bilinen AUMID; tanınmıyorsa `None`. Sınıf bilinmiyorsa (`None`) eşleme yapılmaz.
    pub fn for_window(process_path: Option<&str>, class_name: Option<&str>) -> Option<String> {
        let process_path = process_path?;
        if process_path.chars().all(char::is_whitespace) || !Self::is_explorer_folder_window_class(class_name) {
            return None;
        }

        let system_root = std::env::var("SystemRoot").unwrap_or_default();
        if !system_root.is_empty() && PinnedAppMatcher::same_path(process_path, &win_path::combine(&system_root, "explorer.exe")) {
            return Some(Self::EXPLORER.to_string());
        }

        None
    }
}
