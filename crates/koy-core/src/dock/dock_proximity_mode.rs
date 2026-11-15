use serde::{Deserialize, Serialize};

/// Pencere dock alanına yaklaştığında uygulanacak davranış modu (M6.3 / DESIGN.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DockProximityMode {
    /// Aşağı yapış: Normal bir pencere dock alanına yaklaştığında dock ekranın en altına iner,
    /// yüksekliği 60 px'den 46 px'e düşer ve yalnızca üst köşeleri yuvarlatılır (14,14,0,0).
    /// Pencere uzaklaşınca eski yüzen haline (60 px, alttan 10 px, 18 px yarıçap) geri döner.
    #[default]
    DockDown,

    /// Yer ayır (Varsayılan): Dock çalışma alanından (AppBar / SHAppBarMessage) 46 px ayırır.
    /// Büyütülmüş pencereler tam olarak dock'un üstünde biter, arada boşluk kalmaz.
    ReserveSpace,

    /// Çekil: Büyütülmüş veya çakışan bir pencere dock alanını kapladığında dock ekran dışına kayar
    /// ve altta ince bir tutamak (120×4 px) bırakır. Fare tutamağa gelince (150 ms) dock geri çıkar.
    Dodge,

    /// Hep gizli: Fare ekranın alt kenarına inene kadar dock tamamen gizli kalır.
    AlwaysHidden,

    /// Hep havada: Yüzen kapsül pencere yaklaşsa ya da büyütülse bile asla alta yapışmaz, gizlenmez, yer ayırmaz;
    /// her zaman alttan 10 px boşlukla 60 px yükseklikte yüzer. Yalnız tam ekranda (DockHideInFullscreen) gizlenir.
    /// Ayar dosyasında ADIYLA yazıldığı için (UseStringEnumConverter) sıra önemsizdir; yine de en sona eklendi.
    AlwaysFloating,
}

/// Dock'un görsel yerleşim stili (Yüzen Kapsül veya Tam Genişlik).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DockStyleMode {
    /// Yüzen kapsül (varsayılan): 60 px yükseklik, alttan 10 px boşluk, 18 px köşe yuvarlama.
    #[default]
    FloatingCapsule,

    /// Tam Genişlik Modu: Ekranın en altına yapışık 48 px (`DockPositionResolver::FULL_WIDTH_HEIGHT_DIP`) yükseklik.
    /// Solda müzik, ortada uygulamalar, sağda Koy butonu.
    FullWidth,
}
