//! Karşılığı: `koy/src/Core/Settings/AppSettings.cs`.

use super::setting_value::{FieldView, SectionModel, section};
use super::{
    AppearanceSettings, ClipboardSettings, DisplaysSettings, DockItemsSettings, DockSettings, FocusSettings,
    GeneralSettings, HotCornersSettings, MusicSettings, NotchSettings, QuickLaunchSettings, SearchSettings,
    ShortcutsSettings, VoiceSettings, WeatherSettings,
};
use crate::dock::{DockItemRules, DockLayoutLimits};

// settings.json içeriği (şema v4): bölümlere ayrılmış kayıtlar. Yeni ayar = ilgili bölüm kaydına varsayılanlı alan;
// eşitlik kayıtlardan kendiliğinden gelir (bkz. docs/MAP.md › Yeni ayar eklemek).
// Alan yeniden adlandırılır ya da silinirse `version` artırılır ve `SettingsMigration` yazılır.
// C# notu: özellikler bilerek `set`: kaynak üreticili JSON, `init` özelliklerde dosyada olmayan alanı varsayılan yerine
// sıfırlıyordu. Rust'ta alanlar `pub` ve eksik alan varsayılanında kalır.
section! {
    /// Kök ayar kaydı (settings.json).
    AppSettings {
        /// Şema sürümü. 3: monitör başına ayar, eski MonitorTarget göçle taşınır. 4: bölümlere ayrılmış kayıtlar,
        /// sayı ve dile bağlı metin değerleri enum adı olarak yazılır (v3 düz dosyası `SettingsMigration` ile taşınır).
        version: i32 = AppSettings::CURRENT_VERSION => "Version",
        general: GeneralSettings = GeneralSettings::default() => "General",
        appearance: AppearanceSettings = AppearanceSettings::default() => "Appearance",
        dock: DockSettings = DockSettings::default() => "Dock",
        dock_items: DockItemsSettings = DockItemsSettings::default() => "DockItems",
        notch: NotchSettings = NotchSettings::default() => "Notch",
        music: MusicSettings = MusicSettings::default() => "Music",
        displays: DisplaysSettings = DisplaysSettings::default() => "Displays",
        shortcuts: ShortcutsSettings = ShortcutsSettings::default() => "Shortcuts",
        hot_corners: HotCornersSettings = HotCornersSettings::default() => "HotCorners",
        voice: VoiceSettings = VoiceSettings::default() => "Voice",
        search: SearchSettings = SearchSettings::default() => "Search",
        focus: FocusSettings = FocusSettings::default() => "Focus",
        weather: WeatherSettings = WeatherSettings::default() => "Weather",
        clipboard: ClipboardSettings = ClipboardSettings::default() => "Clipboard",
        quick_launch: QuickLaunchSettings = QuickLaunchSettings::default() => "QuickLaunch",
    }
}

/// Bir ayar bölümünün alan görünümü (C# `AppSettings` özelliklerinin yansıması).
#[derive(Debug, Clone)]
pub struct SectionView {
    pub name: &'static str,
    pub fields: Vec<FieldView>,
}

impl AppSettings {
    pub const CURRENT_VERSION: i32 = 4;

    /// Dosyada `"Dock": null` gibi boş bırakılmış bölümler (ve bölümlerdeki boş listeler) okuma sırasında zaten varsayılanla
    /// dolar (bkz. `setting_value`); burada geri kalan düzeltmeler yapılır: dock geometri sınırları ve öğe sayısı üst sınırı.
    /// Yeni bölüm/liste eklenince `setting_value` kuralı kendiliğinden kapsar.
    pub fn fill_missing_sections(mut self) -> AppSettings {
        DockLayoutLimits::sanitize(&mut self.dock);

        // "Items": [null, ...] boş kayıt bırakır; null girdiler okuma sırasında (sınırdan önce) atılır ki boş yuvalar
        // geçerli öğenin yerini yemesin.

        // Üst sınır yükleme anında uygulanır: Normalize çağırmayan tüketici de 32'den fazla öğe görmesin.
        // Sıra korunur, fazlası sondan atılır (Normalize'ın "ilk gelenler kalır" kuralıyla aynı).
        let max = DockItemRules::MAX_ITEM_COUNT as usize;
        if self.dock_items.items.len() > max {
            self.dock_items.items.truncate(max);
        }

        self
    }

    /// Bölümler bildirim sırasıyla (`Version` hariç) ve her bölümün alanları; `defaults` ile karşılaştırmalı.
    pub fn section_views(&self, defaults: &AppSettings) -> Vec<SectionView> {
        vec![
            SectionView { name: "General", fields: self.general.describe(&defaults.general) },
            SectionView { name: "Appearance", fields: self.appearance.describe(&defaults.appearance) },
            SectionView { name: "Dock", fields: self.dock.describe(&defaults.dock) },
            SectionView { name: "DockItems", fields: self.dock_items.describe(&defaults.dock_items) },
            SectionView { name: "Notch", fields: self.notch.describe(&defaults.notch) },
            SectionView { name: "Music", fields: self.music.describe(&defaults.music) },
            SectionView { name: "Displays", fields: self.displays.describe(&defaults.displays) },
            SectionView { name: "Shortcuts", fields: self.shortcuts.describe(&defaults.shortcuts) },
            SectionView { name: "HotCorners", fields: self.hot_corners.describe(&defaults.hot_corners) },
            SectionView { name: "Voice", fields: self.voice.describe(&defaults.voice) },
            SectionView { name: "Search", fields: self.search.describe(&defaults.search) },
            SectionView { name: "Focus", fields: self.focus.describe(&defaults.focus) },
            SectionView { name: "Weather", fields: self.weather.describe(&defaults.weather) },
            SectionView { name: "Clipboard", fields: self.clipboard.describe(&defaults.clipboard) },
            SectionView { name: "QuickLaunch", fields: self.quick_launch.describe(&defaults.quick_launch) },
        ]
    }

    /// `section.field` skaler alanını farklı değere çevirmiş kopya (eşitlik testleri için).
    pub fn perturb(&self, section: &str, field: &str) -> Option<AppSettings> {
        let mut copy = self.clone();
        match section {
            "General" => copy.general = self.general.perturb(field)?,
            "Appearance" => copy.appearance = self.appearance.perturb(field)?,
            "Dock" => copy.dock = self.dock.perturb(field)?,
            "DockItems" => copy.dock_items = self.dock_items.perturb(field)?,
            "Notch" => copy.notch = self.notch.perturb(field)?,
            "Music" => copy.music = self.music.perturb(field)?,
            "Displays" => copy.displays = self.displays.perturb(field)?,
            "Shortcuts" => copy.shortcuts = self.shortcuts.perturb(field)?,
            "HotCorners" => copy.hot_corners = self.hot_corners.perturb(field)?,
            "Voice" => copy.voice = self.voice.perturb(field)?,
            "Search" => copy.search = self.search.perturb(field)?,
            "Focus" => copy.focus = self.focus.perturb(field)?,
            "Weather" => copy.weather = self.weather.perturb(field)?,
            "Clipboard" => copy.clipboard = self.clipboard.perturb(field)?,
            "QuickLaunch" => copy.quick_launch = self.quick_launch.perturb(field)?,
            _ => return None,
        }
        Some(copy)
    }
}
