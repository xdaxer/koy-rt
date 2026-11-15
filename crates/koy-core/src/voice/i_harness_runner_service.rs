use super::voice_models::{HarnessExecutionRequest, HarnessExecutionResult, VoiceCommandException};

pub use crate::cancel::CancellationToken;

/// `ExecuteAsync` görevinin istisnayla bitme biçimleri (`await` sırasında fırlayan istisnaların karşılığı).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessError {
    /// `OperationCanceledException`.
    Canceled,
    /// `VoiceCommandException`: komut çözülemedi.
    Command(VoiceCommandException),
    /// Başka her istisna; ileti `Exception.Message`.
    Unexpected(String),
}

impl From<VoiceCommandException> for HarnessError {
    fn from(e: VoiceCommandException) -> Self {
        HarnessError::Command(e)
    }
}

/// CLI kodlama aracı (harness) yürütme servisi sözleşmesi.
///
/// C#'taki `Task<...> ExecuteAsync` burada bloklayan çağrıdır (Core'da zaman uyumsuz çalışma zamanı yok);
/// çağıran bunu bir çalışan iş parçacığında çalıştırır. İptal `ct` ile bildirilir.
pub trait IHarnessRunnerService: Send + Sync {
    fn execute(&self, request: &HarnessExecutionRequest, ct: &CancellationToken) -> Result<HarnessExecutionResult, HarnessError>;
}
