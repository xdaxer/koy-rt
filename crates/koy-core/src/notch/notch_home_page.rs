/// Ana sekmesinin iki yüzü: tam sayfa müzik ya da Ana (gün, hava, etkinlikler, sistem, hızlı başlat).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NotchHomePage {
    #[default]
    Music,
    Home,
}

/// Ana sekmesinde hangi yüzün gösterileceği. Kullanıcının seçtiği yüz (Ana veya Müzik) çentik kapansa da korunur;
/// ancak parça yoksa ya da müzik kartı ayarlarda kapalıysa Ana yüzüne düşer.
pub struct NotchHomePages;

impl NotchHomePages {
    pub fn on_open(current: NotchHomePage, has_track: bool, music_enabled: bool) -> NotchHomePage {
        if has_track && music_enabled { current } else { NotchHomePage::Home }
    }
}
