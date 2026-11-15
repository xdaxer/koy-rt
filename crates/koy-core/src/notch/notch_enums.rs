use serde::{Deserialize, Serialize};

// Bu dosyadaki enum adları settings.json'a yazılır: bir kez yayımlandıktan sonra ADLARI DEĞİŞTİRİLMEZ (yalnız yeni değer eklenir).

/// Çentik boştayken (kapalı, müzik çalmıyor) ne gösterilir. Eski dosyalarda sayıydı (0–3), göçte ada çevrilir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum IdleDisplayMode {
    /// Saat ve pil (varsayılan).
    #[default]
    ClockAndBattery,
    /// Yalnız saat.
    ClockOnly,
    /// Hiçbiri.
    Nothing,
    /// Solda dairesel pil halkası ve yüzde, sağda kısa gün adı ve saat ("Sal 19:21").
    RingClockDay,
}

/// Çentiğin açılması: fareyle üzerine gelince (varsayılan), tıklayınca ya da sürüklenen öğe üstüne gelince.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum NotchOpenBehavior {
    #[default]
    Hover,
    Click,
    DragOver,
}

/// Sekme çubuğunun görünümü: yalnız simge, simge ve ad, ya da hiç.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum NotchTabBarStyle {
    #[default]
    IconOnly,
    IconAndLabel,
    Hidden,
}

/// Müzik görselleştiricisinin çizim biçimi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum NotchVisualizerStyle {
    #[default]
    Bars,
    Wave,
    Dots,
}

/// Görselleştirici rengi: Windows vurgu rengi ya da albüm kapağından çıkarılan renk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum NotchVisualizerColorSource {
    #[default]
    Accent,
    Artwork,
}

/// Ana sayfadaki kartlar. Sıra ve görünürlük `NotchSettings.HomeCards` listesindedir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum HomeCard {
    #[default]
    Music,
    HeadphoneBattery,
    SystemNetwork,
    QuickLaunch,
    Weather,
    Timers,
}

/// Çentik üstünde fare tekerleği: panel değiştir (varsayılan) ya da sistem sesini ayarla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum WheelAction {
    #[default]
    SwitchPanel,
    Volume,
}

/// Çentik biçimi: ekrana yapışık (kulaklı) ya da yüzen ada (kulaksız kapsül, üstten boşluklu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum NotchStyleMode {
    #[default]
    Pinned,
    Floating,
}
