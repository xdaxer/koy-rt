use crate::time::TimeSpan;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoiceState {
    Idle,

    /// Çentikte komut kutusu açık: Windows sesle yazma (Win+H) metni yazıyor ya da kullanıcı kendisi yazıyor.
    Editing,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoiceHarnessType {
    Claude,
    Codex,
    /// Antigravity CLI (`agy`).
    Agy,
    Custom,
}

/// Claude'un izin modu. Sesli komut `claude -p` ile başsız çalışır: izin sorusu çıkamaz,
/// bu yüzden `Default` kipinde Claude yalnız okur; dosya oluşturma ve düzenleme reddedilir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoiceClaudePermission {
    /// Ek bayrak yok: yalnız okuma.
    Default,

    /// Claude'un otomatik modu (`--permission-mode auto`): sınıflandırıcı güvenli işleri onaylar, riskliyi reddeder.
    Auto,

    /// Dosya düzenleme ve oluşturma (`--permission-mode acceptEdits`) onaysız yapılır.
    AcceptEdits,
}

/// Komutun neden başarısız olduğu. Kullanıcıya görünen metni App, seçili dile göre bu koddan üretir
/// (Core ve Platform dil bilmez).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoiceFailure {
    None,
    InvalidCommand,
    /// Ayardaki model adı geçersiz; ayrıntı modelin adı.
    InvalidModel,
    ScriptNotSupported,
    CommandNotFound,
    Timeout,
    ExitCode,
    StartFailed,
    Unexpected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCommand {
    pub executable: String,
    pub arguments: Vec<String>,
}

impl ResolvedCommand {
    pub fn new(executable: impl Into<String>, arguments: Vec<String>) -> Self {
        ResolvedCommand { executable: executable.into(), arguments }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessExecutionRequest {
    pub harness_type: VoiceHarnessType,
    pub prompt: String,
    pub working_directory: String,
    /// C# varsayılanı: 60.
    pub timeout_seconds: i32,
    pub custom_executable: Option<String>,
    pub custom_arguments_template: Option<String>,
    pub claude_permission: VoiceClaudePermission,
    pub model: Option<String>,
}

/// `error`: aracın kendi stderr çıktısı. `failure`: başarısızlık kodu; başarıda `VoiceFailure::None`.
/// `detail`: yerelleştirilmiş mesajdaki {0} yerine konacak değer (dosya yolu, saniye).
#[derive(Debug, Clone, PartialEq)]
pub struct HarnessExecutionResult {
    pub success: bool,
    pub exit_code: i32,
    pub output: String,
    pub error: String,
    pub duration: TimeSpan,
    pub failure: VoiceFailure,
    pub detail: String,
}

impl HarnessExecutionResult {
    /// C# varsayılanlarıyla (`Failure = None`, `Detail = ""`) sonuç.
    pub fn new(success: bool, exit_code: i32, output: impl Into<String>, error: impl Into<String>, duration: TimeSpan) -> Self {
        HarnessExecutionResult {
            success,
            exit_code,
            output: output.into(),
            error: error.into(),
            duration,
            failure: VoiceFailure::None,
            detail: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceSessionOptions {
    pub harness_type: VoiceHarnessType,
    pub working_directory: String,
    pub timeout_seconds: i32,
    pub custom_executable: Option<String>,
    pub custom_arguments_template: Option<String>,
    pub claude_permission: VoiceClaudePermission,
    pub model: Option<String>,
}

impl Default for VoiceSessionOptions {
    fn default() -> Self {
        VoiceSessionOptions {
            harness_type: VoiceHarnessType::Claude,
            working_directory: String::new(),
            timeout_seconds: 60,
            custom_executable: None,
            custom_arguments_template: None,
            claude_permission: VoiceClaudePermission::Default,
            model: None,
        }
    }
}

/// Komut çözülemedi: özel harness komutu geçersiz (boş, kabuk yorumlayıcısı ya da kabuk metakarakteri içeriyor).
/// `failure` ve `detail` kullanıcı metnini App'te seçer; ileti yalnız geliştirici içindir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceCommandException {
    pub message: String,
    pub failure: VoiceFailure,
    pub detail: String,
}

impl VoiceCommandException {
    /// C# varsayılanları: `Failure = InvalidCommand`, `Detail = ""`.
    pub fn new(message: impl Into<String>) -> Self {
        VoiceCommandException { message: message.into(), failure: VoiceFailure::InvalidCommand, detail: String::new() }
    }

    pub fn with(message: impl Into<String>, failure: VoiceFailure, detail: impl Into<String>) -> Self {
        VoiceCommandException { message: message.into(), failure, detail: detail.into() }
    }
}

impl fmt::Display for VoiceCommandException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for VoiceCommandException {}
