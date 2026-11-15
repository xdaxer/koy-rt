use super::{ClipboardFilter, ClipboardItem, Guid};
use crate::event::Event;

/// Pano geçmişi servisi sözleşmesi (yalnız metin). Ayar kapalıysa App servisi hiç kurmaz/Start etmez; kapalıyken
/// Win32 dinleyicisi yoktur, boşta CPU %0.
///
/// İPLİK: [`IClipboardHistoryService::changed`] ARKA PLAN iş parçacığından gelebilir (pano mesajı kendi iş
/// parçacığında işlenir). App tarafı Dispatcher'a geçirmeli. Diğer üyeler her iş parçacığından çağrılabilir.
/// Gizlilik: parola yöneticilerinin "geçmişe alma" işaretini taşıyan panolar kaydedilmez. Sabitlenen öğeler düz
/// metin olarak diske yazılır (clipboard-pinned.json); gerisi yalnız bellekte kalır.
/// (`IDisposable` → uygulayıcının `Drop`'u.)
pub trait IClipboardHistoryService: Send + Sync {
    /// Dinleyici kurulu mu.
    fn is_running(&self) -> bool;

    /// Dinlemeyi başlatır (idempotent). Sabit öğeler diskten ilk Start'ta yüklenir.
    fn start(&self);

    /// Dinleyiciyi kaldırır (idempotent). Bellekteki geçmiş durur; silmek için [`Self::clear`].
    fn stop(&self);

    /// Liste değişti (yeni öğe, taşıma, sabitleme, silme). Arka plan iş parçacığından gelebilir.
    fn changed(&self) -> &Event<()>;

    /// En yeni başta; süzgeç ve arama büyük/küçük harf duyarsız. C# varsayılanları: `filter = All`, `search = null`.
    fn query(&self, filter: ClipboardFilter, search: Option<&str>) -> Vec<ClipboardItem>;

    fn set_pinned(&self, id: Guid, pinned: bool) -> bool;

    fn remove(&self, id: Guid) -> bool;

    /// Sabitlenmeyenleri siler.
    fn clear(&self);

    /// Öğeyi panoya yazar; bu yazma geçmişe yeniden eklenmez (öğe başa taşınmaz). Başarılıysa true.
    fn copy_back(&self, id: Guid) -> bool;
}
