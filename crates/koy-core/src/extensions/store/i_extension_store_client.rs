//! Karşılığı: `Extensions/Store/IExtensionStoreClient.cs`.

use std::path::Path;

use super::{InstallResult, StoreIndexResult, StoreItem};
use crate::cancel::CancellationToken;

/// Mağaza istemcisinin sözleşmesi (uygulaması Platform'da). Hiçbir yöntem beklenen hatada fırlatmaz:
/// sonuçta hata kodu döner. İptal de hata kodudur (`ExtensionInstallError::Cancelled`).
///
/// C# `Task<T>` dönüşleri burada bloklayan çağrıdır (Core'da zaman uyumsuz çalışma zamanı yok); çağıran
/// (App) bunları arka plan iş parçacığında çağırır.
pub trait IExtensionStoreClient: Send + Sync {
    /// İndeksi çeker ve ayrıştırır. Yalnız mağaza sayfası açıkken çağrılır (AGENTS #5).
    fn fetch_index(&self, cancellation_token: &CancellationToken) -> StoreIndexResult;

    /// Paketi indirir, SHA256'yı doğrular, denetler, kurar ve kaydı günceller. İlerleme 0..1 (`IProgress<double>`).
    fn install_from_store(
        &self,
        item: &StoreItem,
        progress: Option<&(dyn Fn(f64) + Send + Sync)>,
        cancellation_token: &CancellationToken,
    ) -> InstallResult;

    /// Yerel bir .koyext (zip) dosyasından kurar; aynı denetimler, SHA256 yok.
    fn install_from_file(&self, path: &Path, cancellation_token: &CancellationToken) -> InstallResult;

    /// Yalnız kayıttaki eklentinin klasörünü siler ve kayıttan çıkarır. Ayarların geri alınması çağıranındır
    /// (önce `ExtensionApplyPlanner::revert_on_uninstall`). Kayıtta yoksa ya da silinemezse false.
    fn uninstall(&self, id: &str) -> bool;
}
