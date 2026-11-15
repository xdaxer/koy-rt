/// Dock'un sağ bölümünün hangi parçalarının görüneceği (yüzen kapsül ve tam genişlik aynı kuralı kullanır).
/// Kapalı düğmelerin ayırıcısı da gizlenir; küçük tepsi ok olarak Koy menüsü düğmesinin sağına geçer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DockRightSection {
    pub show_first_separator: bool,
    pub show_full_tray: bool,
    pub show_small_tray: bool,
}

impl DockRightSection {
    /// Ok düğmesinin genişliği (DIP); tam boy düğmenin yerine yalnız bu kadar yer tutar.
    pub const SMALL_TRAY_WIDTH_DIP: f64 = 18.0;

    /// * `show_folders`: İndirilenler düğmesi açık mı.
    /// * `show_recents`: Son kullanılanlar düğmesi açık mı (ayar ve servis).
    /// * `has_tray`: Tepsi penceresi gerçekten çalışıyor mu.
    /// * `small_tray`: Küçük tepsi ayarı.
    pub fn resolve(show_folders: bool, show_recents: bool, has_tray: bool, small_tray: bool) -> DockRightSection {
        let full_tray = has_tray && !small_tray;
        DockRightSection {
            show_first_separator: show_folders || show_recents || full_tray,
            show_full_tray: full_tray,
            show_small_tray: has_tray && small_tray,
        }
    }
}
