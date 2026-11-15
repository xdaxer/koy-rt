use super::i_harness_runner_service::{CancellationToken, HarnessError, IHarnessRunnerService};
use super::voice_models::{HarnessExecutionRequest, HarnessExecutionResult, VoiceFailure, VoiceSessionOptions, VoiceState};
use crate::event::Event;
use crate::text;
use std::sync::{Arc, Mutex, MutexGuard};

struct Inner {
    state: VoiceState,
    prompt: String,
    failure: VoiceFailure,
    failure_detail: String,
    last_result: Option<HarnessExecutionResult>,
    running_cts: Option<CancellationToken>,
    disposed: bool,
}

/// Kilit altında biriken `StateChanged` bildirimleri; kilit bırakıldıktan sonra sırayla yayınlanır
/// (PORTING.md kural 10: kilidi tutarken dış olay tetiklenmez).
type PendingStates = Vec<VoiceState>;

fn set_failure(inner: &mut Inner, failure: VoiceFailure, detail: &str) {
    inner.failure = failure;
    inner.failure_detail = detail.to_string();
}

fn set_state(inner: &mut Inner, pending: &mut PendingStates, new_state: VoiceState) {
    inner.state = new_state;
    pending.push(new_state);
}

/// Çentikteki sesli komut oturumu: Boşta -> Yazılıyor -> Çalışıyor -> Bitti / Hata.
/// Konuşmayı Koy tanımaz; Windows sesle yazma (Win+H) metni çentikteki kutuya yazar, kullanıcı isterse kendisi de yazar.
/// Böylece Windows'un desteklediği her dilde (Türkçe, İngilizce…) çalışır ve ayrıca konuşma dili paketi gerekmez.
/// Kullanıcıya metin üretmez: hata `failure()` koduyla bildirilir, metni App seçili dilde yazar.
///
/// C#'taki `Task` dönen metotlar (`TriggerShortcutAsync`, `ConfirmAsync`) burada bloklayan çağrıdır:
/// koşucu çalışırken `confirm` çağıranı bekletir; `cancel` başka bir iş parçacığından çağrılabilir.
pub struct VoiceSessionController {
    runner: Arc<dyn IHarnessRunnerService>,
    options_provider: Box<dyn Fn() -> VoiceSessionOptions + Send + Sync>,
    inner: Mutex<Inner>,

    /// Durum her değiştiğinde (aynı duruma geçiş dahil) yeni durumla tetiklenir.
    pub state_changed: Event<VoiceState>,
    pub execution_finished: Event<HarnessExecutionResult>,
}

impl VoiceSessionController {
    pub fn new(runner: Arc<dyn IHarnessRunnerService>, options_provider: impl Fn() -> VoiceSessionOptions + Send + Sync + 'static) -> Self {
        VoiceSessionController {
            runner,
            options_provider: Box::new(options_provider),
            inner: Mutex::new(Inner {
                state: VoiceState::Idle,
                prompt: String::new(),
                failure: VoiceFailure::None,
                failure_detail: String::new(),
                last_result: None,
                running_cts: None,
                disposed: false,
            }),
            state_changed: Event::new(),
            execution_finished: Event::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap()
    }

    fn notify(&self, pending: PendingStates) {
        for state in pending {
            self.state_changed.invoke(&state);
        }
    }

    pub fn state(&self) -> VoiceState {
        self.lock().state
    }

    /// Kutudaki komut metni (sesle yazılan ya da elle yazılan).
    pub fn prompt(&self) -> String {
        self.lock().prompt.clone()
    }

    /// Son başarısızlığın kodu; başarı ya da iptalde `VoiceFailure::None`.
    pub fn failure(&self) -> VoiceFailure {
        self.lock().failure
    }

    /// Başarısızlık mesajına eklenecek ham ayrıntı (yol, saniye); çevrilmez.
    pub fn failure_detail(&self) -> String {
        self.lock().failure_detail.clone()
    }

    pub fn last_result(&self) -> Option<HarnessExecutionResult> {
        self.lock().last_result.clone()
    }

    /// Kısayol: boştaysa kutuyu açar; kutu açıkken metin varsa çalıştırır, yoksa kapatır; çalışıyorsa iptal eder.
    pub fn trigger_shortcut(&self) {
        let (current, has_prompt) = {
            let inner = self.lock();
            if inner.disposed {
                return;
            }
            (inner.state, !text::is_null_or_white_space(Some(&inner.prompt)))
        };

        match current {
            VoiceState::Idle | VoiceState::Completed | VoiceState::Failed => self.start(),
            VoiceState::Editing if has_prompt => self.confirm(),
            VoiceState::Editing | VoiceState::Running => self.cancel(),
        }
    }

    /// Komut kutusunu boş açar.
    pub fn start(&self) {
        let mut pending = PendingStates::new();
        {
            let mut inner = self.lock();
            if inner.disposed || inner.state == VoiceState::Running {
                return;
            }

            inner.prompt = String::new();
            inner.last_result = None;
            set_failure(&mut inner, VoiceFailure::None, "");
            set_state(&mut inner, &mut pending, VoiceState::Editing);
        }
        self.notify(pending);
    }

    /// Kutudaki metin değişti (sesle yazma ya da klavye). Yalnız kutu açıkken kabul edilir.
    pub fn update_prompt(&self, text: &str) {
        let mut inner = self.lock();
        if !inner.disposed && inner.state == VoiceState::Editing {
            inner.prompt = text.to_string();
        }
    }

    /// Kutudaki metni çalıştırır. Metin boşsa bir şey yapmaz (kutu açık kalır).
    pub fn confirm(&self) {
        let options: VoiceSessionOptions;
        let prompt: String;
        let cts: CancellationToken;
        let mut pending = PendingStates::new();

        {
            let mut inner = self.lock();
            if inner.disposed || inner.state != VoiceState::Editing || text::is_null_or_white_space(Some(&inner.prompt)) {
                return;
            }

            options = (self.options_provider)();
            prompt = text::trim(&inner.prompt).to_string();

            if let Some(old) = inner.running_cts.take() {
                old.cancel();
            }
            cts = CancellationToken::new();
            inner.running_cts = Some(cts.clone());

            set_failure(&mut inner, VoiceFailure::None, "");
            set_state(&mut inner, &mut pending, VoiceState::Running);
        }
        self.notify(pending);

        let request = HarnessExecutionRequest {
            harness_type: options.harness_type,
            prompt,
            working_directory: options.working_directory,
            timeout_seconds: options.timeout_seconds,
            custom_executable: options.custom_executable,
            custom_arguments_template: options.custom_arguments_template,
            claude_permission: options.claude_permission,
            model: options.model,
        };

        match self.runner.execute(&request, &cts) {
            Ok(result) => {
                let mut pending = PendingStates::new();
                {
                    let mut inner = self.lock();
                    // İptal edildiyse (Cancel durumu zaten Idle yaptı) geç gelen sonuç durumu değiştirmez.
                    if inner.disposed || inner.state != VoiceState::Running {
                        return;
                    }

                    inner.last_result = Some(result.clone());
                    if result.success {
                        set_failure(&mut inner, VoiceFailure::None, "");
                        set_state(&mut inner, &mut pending, VoiceState::Completed);
                    } else {
                        let failure = if result.failure == VoiceFailure::None { VoiceFailure::ExitCode } else { result.failure };
                        set_failure(&mut inner, failure, &result.detail);
                        set_state(&mut inner, &mut pending, VoiceState::Failed);
                    }
                }
                self.notify(pending);

                self.execution_finished.invoke(&result);
            }
            Err(HarnessError::Canceled) => {
                let mut pending = PendingStates::new();
                {
                    let mut inner = self.lock();
                    if inner.disposed || inner.state != VoiceState::Running {
                        return;
                    }

                    set_failure(&mut inner, VoiceFailure::None, "");
                    set_state(&mut inner, &mut pending, VoiceState::Idle);
                }
                self.notify(pending);
            }
            Err(HarnessError::Command(ex)) => self.fail(ex.failure, &ex.detail),
            Err(HarnessError::Unexpected(message)) => self.fail(VoiceFailure::Unexpected, &message),
        }
    }

    pub fn cancel(&self) {
        let mut pending = PendingStates::new();
        {
            let mut inner = self.lock();
            if inner.disposed {
                return;
            }

            if inner.state == VoiceState::Running {
                if let Some(cts) = &inner.running_cts {
                    cts.cancel();
                }
            }

            inner.prompt = String::new();
            set_failure(&mut inner, VoiceFailure::None, "");
            set_state(&mut inner, &mut pending, VoiceState::Idle);
        }
        self.notify(pending);
    }

    fn fail(&self, failure: VoiceFailure, detail: &str) {
        let mut pending = PendingStates::new();
        {
            let mut inner = self.lock();
            if inner.disposed || inner.state != VoiceState::Running {
                return;
            }

            set_failure(&mut inner, failure, detail);
            set_state(&mut inner, &mut pending, VoiceState::Failed);
        }
        self.notify(pending);
    }

    /// `IDisposable.Dispose`: yinelenen çağrı zararsız. `Drop` da bunu çağırır.
    pub fn dispose(&self) {
        let mut inner = self.lock();
        if inner.disposed {
            return;
        }

        inner.disposed = true;
        if let Some(cts) = inner.running_cts.take() {
            cts.cancel();
        }
    }
}

impl Drop for VoiceSessionController {
    fn drop(&mut self) {
        self.dispose();
    }
}
