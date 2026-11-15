use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::text;

/// Açık çentikte alttaki sekmelerden seçilebilen paneller. Sekme çubuğu bu sıraya göre dizilir
/// ([`NotchPanels::order`]). Pano yalnız pano geçmişi ayarı açıkken görünür.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum NotchPanel {
    /// Ana: müzik ya da selamlama, hava durumu, sistem kartları, hızlı başlat, çalışan zamanlayıcı.
    #[default]
    Home,
    /// Odak: Pomodoro, mola, sayaç ve kronometre; görev listesi.
    Focus,
    /// Takvim: ay, hafta ve gün görünümü, yerel etkinlikler.
    Calendar,
    Shelf,
    /// Pano geçmişi (yalnız `ClipboardHistoryEnabled` açıkken).
    Clipboard,
    /// Sistem: CPU, bellek, ağ hızı, Koy'un kendi belleği.
    System,
    ControlCenter,
}

/// Panellerin sekme sırası ve ayarlardaki anahtar adları.
pub struct NotchPanels;

impl NotchPanels {
    /// Sekme çubuğundaki sıra. Ayarlar yalnız hangilerinin açık olduğunu söyler, sırayı değiştirmez.
    pub fn order() -> &'static [NotchPanel] {
        &[
            NotchPanel::Home,
            NotchPanel::Focus,
            NotchPanel::Calendar,
            NotchPanel::Shelf,
            NotchPanel::Clipboard,
            NotchPanel::System,
            NotchPanel::ControlCenter,
        ]
    }

    /// Kullanıcının sekme sırası (`NotchSettings.PanelOrder`). Geçerli, tekrarsız anahtarlar
    /// verilen sırayla başa gelir; eksik paneller varsayılan sıranın sonuna eklenir. Boş ya da `None` girdi
    /// varsayılan sırayı verir. Görünürlük bu listeyi etkilemez.
    pub fn normalize_order(keys: Option<&[String]>) -> Vec<NotchPanel> {
        let mut result: Vec<NotchPanel> = Vec::new();
        if let Some(keys) = keys {
            for key in keys {
                if let Some(panel) = Self::try_parse(Some(key)) {
                    if !result.contains(&panel) {
                        result.push(panel);
                    }
                }
            }
        }

        for panel in Self::order() {
            if !result.contains(panel) {
                result.push(*panel);
            }
        }

        result
    }

    /// `AppSettings.EnabledPanels` anahtarlarından açık paneller, sekme sırasıyla.
    /// Ana her zaman açıktır. Eski anahtarlar taşınır: "music" Ana, "calendar" Takvim olur (H öncesi dosyalarda Odak da açılır); "status" artık ayrı
    /// bir panel değil (saat, hava ve pil üstteki durum satırında), yok sayılır. Eski ayar dosyalarında "clipboard"
    /// ve "system" yoktur: bu sekmeler Koy penceresinden açılana kadar kapalı kalır (dosya bozulmaz).
    /// `clipboard_history_enabled` kapalıysa Pano, anahtar listede olsa bile elenir.
    /// C# varsayılanı `clipboardHistoryEnabled = true`.
    pub fn parse_enabled(keys: Option<&[String]>, clipboard_history_enabled: bool) -> Vec<NotchPanel> {
        let mut enabled: HashSet<NotchPanel> = HashSet::new();
        enabled.insert(NotchPanel::Home);
        if let Some(list) = keys {
            for key in list {
                if let Some(panel) = Self::try_parse(Some(key)) {
                    enabled.insert(panel);
                }
            }

            // H öncesi dosyalarda (yalnız onlarda "music"/"status" anahtarı vardır) "calendar" paneli Pomodoro ve
            // zamanlayıcıları da taşırdı; Odak o zamandan beri ayrı sekme, bu yüzden kaybolmasın.
            let legacy = list.iter().any(|k| text::eq_ignore_case(text::trim(k), "music") || text::eq_ignore_case(text::trim(k), "status"));
            if legacy && enabled.contains(&NotchPanel::Calendar) {
                enabled.insert(NotchPanel::Focus);
            }

            // Pano ve Sistem H'den önce yoktu: H öncesi dosyada kullanıcı onları kapatmış olamaz, açık gelirler
            // (Pano yine de pano geçmişi ayarına bağlı, aşağıda).
            if legacy {
                enabled.insert(NotchPanel::Clipboard);
                enabled.insert(NotchPanel::System);
            }
        }

        if !clipboard_history_enabled {
            enabled.remove(&NotchPanel::Clipboard);
        }

        Self::order().iter().copied().filter(|p| enabled.contains(p)).collect()
    }

    /// Açık paneller, kullanıcının sekme sırasıyla (issue #78): görünürlük [`NotchPanels::parse_enabled`]'dan, sıra
    /// [`NotchPanels::normalize_order`]'dan gelir. Sıra boşsa sonuç `parse_enabled` ile aynıdır.
    pub fn parse_enabled_with_order(keys: Option<&[String]>, clipboard_history_enabled: bool, order: Option<&[String]>) -> Vec<NotchPanel> {
        let enabled = Self::parse_enabled(keys, clipboard_history_enabled);
        Self::normalize_order(order).into_iter().filter(|p| enabled.contains(p)).collect()
    }

    /// Sekme sırasında bir paneli bir adım yukarı (-1) ya da aşağı (+1) taşır; sonuç anahtar listesi.
    /// (C# adı `Move`; Rust'ta anahtar sözcük.)
    pub fn r#move(order: Option<&[String]>, panel: NotchPanel, step: i32) -> Vec<String> {
        let mut list = Self::normalize_order(order);
        let index = list.iter().position(|p| *p == panel).map_or(-1, |i| i as i32);
        let target = text::clamp(index + step.signum(), 0, list.len() as i32 - 1);
        if index >= 0 && target != index {
            list.remove(index as usize);
            list.insert(target as usize, panel);
        }

        list.into_iter().map(|p| Self::to_key(p).to_string()).collect()
    }

    /// Sekme sırasındaki yeri (listede olmayan sona gider).
    pub fn sort_index(panel: NotchPanel) -> i32 {
        for (i, p) in Self::order().iter().enumerate() {
            if *p == panel {
                return i as i32;
            }
        }

        i32::MAX
    }

    /// Ayarlara yazılan anahtar.
    pub fn to_key(panel: NotchPanel) -> &'static str {
        match panel {
            NotchPanel::Home => "home",
            NotchPanel::Focus => "focus",
            NotchPanel::Calendar => "calendar",
            NotchPanel::Shelf => "shelf",
            NotchPanel::Clipboard => "clipboard",
            NotchPanel::System => "system",
            NotchPanel::ControlCenter => "controlcenter",
        }
    }

    /// `TryParse(key, out panel)`: başarısızsa `None`.
    pub fn try_parse(key: Option<&str>) -> Option<NotchPanel> {
        let key = key.map(|k| text::to_lower_invariant(text::trim(k)))?;
        match key.as_str() {
            "home" | "music" => Some(NotchPanel::Home),
            "focus" => Some(NotchPanel::Focus),
            "calendar" => Some(NotchPanel::Calendar),
            "shelf" => Some(NotchPanel::Shelf),
            "clipboard" => Some(NotchPanel::Clipboard),
            "system" => Some(NotchPanel::System),
            "controlcenter" => Some(NotchPanel::ControlCenter),
            _ => None,
        }
    }
}
