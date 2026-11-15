use crate::time::DateTimeOffset;

/// Çentik kartının içerik türü. Kart yüzeyi türden bağımsızdır; yeni tür eklemek yeni kart modeli gerektirmez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NotchCardKind {
    #[default]
    Update,
    Notification,
}

/// Çentikte HUD'dan (260×40) büyük, düğmeli kartın içeriği. Güncelleme kartı ve (sonraki pakette) Windows bildirimleri
/// aynı kartı kullanır. Metinler çağıran tarafından çevrilmiş olarak verilir (Core resx'e erişemez).
#[derive(Debug, Clone, PartialEq)]
pub struct NotchCard {
    pub kind: NotchCardKind,
    pub title: String,
    pub subtitle: String,
    /// Birincil düğmenin yazısı; boşsa düğme gösterilmez.
    pub primary_action_label: String,
    /// Bildirimi gönderen uygulama; güncellemede boş.
    pub app_name: String,
    pub timestamp: Option<DateTimeOffset>,
    /// Bildirim için uygulama simgesi (kodlanmış resim); güncellemede `None`.
    pub icon: Option<Vec<u8>>,
    /// Başlığın üstündeki küçük satır (bildirimde "UYGULAMA · şimdi"); boşsa satır gösterilmez.
    pub header: String,
    pub fallback_icon: Option<Vec<u8>>,
}

impl NotchCard {
    /// C# kurucusunun zorunlu parametreleri; öbürleri varsayılan (boş metin / `None`).
    pub fn new(kind: NotchCardKind, title: impl Into<String>, subtitle: impl Into<String>) -> Self {
        NotchCard {
            kind,
            title: title.into(),
            subtitle: subtitle.into(),
            primary_action_label: String::new(),
            app_name: String::new(),
            timestamp: None,
            icon: None,
            header: String::new(),
            fallback_icon: None,
        }
    }

    /// `PrimaryActionLabel` verilmiş kurucu çağrısı.
    pub fn with_primary_action_label(mut self, label: impl Into<String>) -> Self {
        self.primary_action_label = label.into();
        self
    }

    pub fn has_primary_action(&self) -> bool {
        !self.primary_action_label.is_empty()
    }

    pub fn has_header(&self) -> bool {
        !self.header.is_empty()
    }

    pub fn has_subtitle(&self) -> bool {
        !self.subtitle.is_empty()
    }
}
