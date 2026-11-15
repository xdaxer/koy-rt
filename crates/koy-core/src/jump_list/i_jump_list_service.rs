//! Karşılığı: `JumpList/IJumpListService.cs`.

use super::jump_list_models::{JumpListEntry, JumpListSection};

/// Uygulama bazlı atlama listesi (Windows görev çubuğundaki sağ tık menüsü). Yalnız menü açılınca çağrılır; izleyici,
/// zamanlayıcı ya da yoklama yoktur. Sonuçlar dosya zamanına göre sınırlı sayıda önbelleğe alınır.
///
/// C# `Task` döndüren metotlar burada eşzamanlı: çağıran, gerekiyorsa kendi iş parçacığında çağırır.
/// `CancellationToken` taşınmadı (uygulayan Platform katmanı kendi iptalini yönetir).
pub trait IJumpListService {
    /// AppUserModelID için bölümleri okur (menü sırasında, sınırlı). Hata, dosya yok ya da AppID boş: boş liste (istisna yok).
    fn get(&self, app_user_model_id: &str) -> Vec<JumpListSection>;

    /// Pencere ya da paket AUMID'i vermeyen paketsiz uygulamanın (Discord gibi süreç düzeyinde koyanlar) AppUserModelID'ini,
    /// exe'sini gösteren Başlat menüsü kısayolundan bulur. Yalnız sağ tıkta çağrılır; sınırlı tarama, sonuç (bulunamadı
    /// dahil) exe yoluna göre önbellekli. Bulunamazsa `None` (istisna yok).
    fn resolve_app_id(&self, exe_path: &str) -> Option<String>;

    /// Girdiyi çalıştırır. `app_exe_path` belge girdilerini uygulamanın kendisiyle açmak içindir.
    fn launch(&self, entry: &JumpListEntry, app_exe_path: Option<&str>);

    /// Önbelleği boşaltır (dock kapanırken).
    fn clear(&self);
}
