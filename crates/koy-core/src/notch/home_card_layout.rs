use std::collections::HashSet;

use super::HomeCard;
use crate::settings::ValueList;

/// Ana sayfa kart düzeni. Kullanıcı tüm kartları gizleyebilir, bu yüzden boş liste geçerlidir; yalnız tekrarlar
/// ve bilinmeyen değerler elenir (elle düzenlenmiş dosyaya karşı).
pub struct HomeCardLayout;

impl HomeCardLayout {
    /// `HomeCardLayout.Default`.
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> &'static [HomeCard] {
        &[HomeCard::Music, HomeCard::HeadphoneBattery, HomeCard::SystemNetwork, HomeCard::QuickLaunch, HomeCard::Weather, HomeCard::Timers]
    }

    /// Sıra korunur, ilk görülen kazanır. `None` ise varsayılan düzen döner.
    /// (`Enum.IsDefined` denetimi Rust'ta gereksiz: geçersiz sayı değeri enum'a hiç girmez.)
    pub fn normalize(cards: Option<&[HomeCard]>) -> ValueList<HomeCard> {
        let Some(cards) = cards else {
            return ValueList::from(Self::default().to_vec());
        };

        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for card in cards {
            if seen.insert(*card) {
                result.push(*card);
            }
        }

        ValueList::from(result)
    }
}
