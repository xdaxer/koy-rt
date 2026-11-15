use crate::time::TimeSpan;
use crate::voice::*;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// `Resolve(type, prompt)` — C# varsayılan parametreleriyle.
fn resolve(harness: VoiceHarnessType, prompt: &str) -> ResolvedCommand {
    resolve_full(harness, prompt, None, None, VoiceClaudePermission::Default, None).unwrap()
}

fn resolve_full(
    harness: VoiceHarnessType,
    prompt: &str,
    exe: Option<&str>,
    template: Option<&str>,
    permission: VoiceClaudePermission,
    model: Option<&str>,
) -> Result<ResolvedCommand, VoiceCommandException> {
    VoiceCommandParser::resolve(harness, prompt, exe, template, permission, model)
}

#[test]
fn voice_command_parser_claude_preset_resolves_correctly() {
    let resolved = resolve(VoiceHarnessType::Claude, "unit test yaz");
    assert_eq!("claude", resolved.executable);
    // Varsayılan salt okunur: izin bayrağı yok, dikte yanlış duyulsa da onaysız değişiklik olmaz.
    assert_eq!(strs(&["-p", "--", "unit test yaz"]), resolved.arguments);
}

#[test]
fn voice_command_parser_claude_permission_adds_mode_before_double_dash() {
    let cases: [(VoiceClaudePermission, &[&str]); 3] = [
        (VoiceClaudePermission::Auto, &["-p", "--permission-mode", "auto", "--", "x"]),
        (VoiceClaudePermission::AcceptEdits, &["-p", "--permission-mode", "acceptEdits", "--", "x"]),
        (VoiceClaudePermission::Default, &["-p", "--", "x"]),
    ];
    for (mode, expected) in cases {
        let resolved = resolve_full(VoiceHarnessType::Claude, "x", None, None, mode, None).unwrap();
        assert_eq!(strs(expected), resolved.arguments, "{mode:?}");
    }
}

#[test]
fn voice_command_parser_claude_model_adds_model_flag_before_double_dash() {
    let resolved = resolve_full(VoiceHarnessType::Claude, "x", None, None, VoiceClaudePermission::Auto, Some("opus")).unwrap();
    assert_eq!(strs(&["-p", "--model", "opus", "--permission-mode", "auto", "--", "x"]), resolved.arguments);
}

#[test]
fn voice_command_parser_claude_model_default_or_empty_adds_no_model_flag() {
    for model in [None, Some(""), Some("  "), Some("default"), Some("DEFAULT")] {
        let resolved = resolve_full(VoiceHarnessType::Claude, "x", None, None, VoiceClaudePermission::Default, model).unwrap();
        assert_eq!(strs(&["-p", "--", "x"]), resolved.arguments, "{model:?}");
    }
}

#[test]
fn voice_command_parser_claude_model_invalid_name_throws() {
    for model in ["--dangerously-skip-permissions", "-x", "opus; calc", "a b"] {
        let ex = resolve_full(VoiceHarnessType::Claude, "x", None, None, VoiceClaudePermission::Default, Some(model)).unwrap_err();
        // Kullanıcı metnini App seçer: Core yalnız kodu ve modelin adını taşır.
        assert_eq!(VoiceFailure::InvalidModel, ex.failure, "{model}");
        assert_eq!(model, ex.detail, "{model}");
    }
}

#[test]
fn voice_command_parser_model_does_not_affect_other_harnesses() {
    let resolved = resolve_full(VoiceHarnessType::Codex, "x", None, None, VoiceClaudePermission::Default, Some("opus")).unwrap();
    assert_eq!(strs(&["exec", "--", "x"]), resolved.arguments);
}

#[test]
fn voice_command_parser_claude_permission_does_not_affect_other_harnesses() {
    let codex = resolve_full(VoiceHarnessType::Codex, "x", None, None, VoiceClaudePermission::Auto, None).unwrap();
    assert_eq!(strs(&["exec", "--", "x"]), codex.arguments);
    let agy = resolve_full(VoiceHarnessType::Agy, "x", None, None, VoiceClaudePermission::Auto, None).unwrap();
    assert_eq!(strs(&["-p", "x"]), agy.arguments);
}

#[test]
fn voice_command_parser_codex_preset_resolves_correctly() {
    let resolved = resolve(VoiceHarnessType::Codex, "refactor et");
    assert_eq!("codex", resolved.executable);
    assert_eq!(strs(&["exec", "--", "refactor et"]), resolved.arguments);
}

#[test]
fn voice_command_parser_agy_preset_uses_print_flag_with_prompt_as_value() {
    let resolved = resolve(VoiceHarnessType::Agy, "durumu raporla");
    assert_eq!("agy", resolved.executable);
    assert_eq!(strs(&["-p", "durumu raporla"]), resolved.arguments);
}

#[test]
fn voice_command_parser_dash_prompt_stays_a_prompt_for_every_preset() {
    // Claude ve Codex "--" ile, agy ise -p değeri olarak istemi seçenek sanmaz.
    assert_eq!(strs(&["-p", "--", "--version"]), resolve(VoiceHarnessType::Claude, "--version").arguments);
    assert_eq!(strs(&["exec", "--", "--version"]), resolve(VoiceHarnessType::Codex, "--version").arguments);
    assert_eq!(strs(&["-p", "--version"]), resolve(VoiceHarnessType::Agy, "--version").arguments);
}

#[test]
fn voice_command_parser_custom_with_placeholder_replaces_placeholder() {
    let resolved = resolve_full(
        VoiceHarnessType::Custom,
        "hızlı özet",
        Some("my-tool.exe"),
        Some("--prompt \"{prompt}\" --verbose"),
        VoiceClaudePermission::Default,
        None,
    )
    .unwrap();

    assert_eq!("my-tool.exe", resolved.executable);
    assert_eq!(strs(&["--prompt", "hızlı özet", "--verbose"]), resolved.arguments);
}

#[test]
fn voice_command_parser_custom_without_placeholder_appends_prompt_to_end() {
    let resolved = resolve_full(
        VoiceHarnessType::Custom,
        "testleri çalıştır",
        Some("runner"),
        Some("run --mode quick"),
        VoiceClaudePermission::Default,
        None,
    )
    .unwrap();

    assert_eq!("runner", resolved.executable);
    assert_eq!(strs(&["run", "--mode", "quick", "testleri çalıştır"]), resolved.arguments);
}

#[test]
fn voice_command_parser_blocks_shell_executables() {
    for shell_exe in ["cmd.exe", "cmd", "powershell.exe", "pwsh", "bash", "C:\\Windows\\System32\\cmd.exe"] {
        assert!(
            resolve_full(VoiceHarnessType::Custom, "test", Some(shell_exe), None, VoiceClaudePermission::Default, None).is_err(),
            "{shell_exe}"
        );
    }
}

#[test]
fn voice_command_parser_blocks_metacharacters_in_executable() {
    for dangerous_exe in ["tool & del /f *", "tool | calc", "tool > out.txt", "tool; malicious"] {
        assert!(
            resolve_full(VoiceHarnessType::Custom, "test", Some(dangerous_exe), None, VoiceClaudePermission::Default, None).is_err(),
            "{dangerous_exe}"
        );
    }
}

#[test]
fn voice_command_parser_dangerous_prompt_injection_treated_as_single_literal_argument() {
    // Komut enjeksiyonu denemesi: tırnaklar, borular, ampersand'lar
    const MALICIOUS_PROMPT: &str = "hello & rm -rf / ; dir | calc.exe && echo \"pwned\"";

    let resolved = resolve(VoiceHarnessType::Claude, MALICIOUS_PROMPT);

    assert_eq!("claude", resolved.executable);
    assert_eq!(3, resolved.arguments.len());
    assert_eq!("-p", resolved.arguments[0]);
    assert_eq!("--", resolved.arguments[1]);
    // Argüman parçalanmadan tam olarak tek bir öğe olarak kalmalı
    assert_eq!(MALICIOUS_PROMPT, resolved.arguments[2]);
}

#[test]
fn voice_command_parser_single_custom_command_line_parses_exe_and_arguments_with_placeholder() {
    let resolved = resolve_full(
        VoiceHarnessType::Custom,
        "kod yaz",
        Some("gh copilot suggest \"{prompt}\""),
        None,
        VoiceClaudePermission::Default,
        None,
    )
    .unwrap();

    assert_eq!("gh", resolved.executable);
    assert_eq!(strs(&["copilot", "suggest", "kod yaz"]), resolved.arguments);
}

#[test]
fn voice_command_parser_single_custom_command_line_with_quotes_in_exe_parses_correctly() {
    let resolved = resolve_full(
        VoiceHarnessType::Custom,
        "soru sor",
        Some("\"C:\\My Tools\\custom.exe\" --ask \"{prompt}\""),
        None,
        VoiceClaudePermission::Default,
        None,
    )
    .unwrap();

    assert_eq!(r"C:\My Tools\custom.exe", resolved.executable);
    assert_eq!(strs(&["--ask", "soru sor"]), resolved.arguments);
}

#[test]
fn voice_command_parser_single_custom_command_line_without_placeholder_appends_prompt() {
    let resolved =
        resolve_full(VoiceHarnessType::Custom, "fix bug", Some("git commit -m"), None, VoiceClaudePermission::Default, None).unwrap();

    assert_eq!("git", resolved.executable);
    assert_eq!(strs(&["commit", "-m", "fix bug"]), resolved.arguments);
}

// ---- VoiceSessionController ----

struct FakeHarnessRunnerService {
    result_to_return: Mutex<HarnessExecutionResult>,
    last_request: Mutex<Option<HarnessExecutionRequest>>,
    call_count: AtomicI32,
}

impl FakeHarnessRunnerService {
    fn new() -> Self {
        Self::returning(HarnessExecutionResult::new(true, 0, "OK", "", TimeSpan::from_secs(1)))
    }

    fn returning(result: HarnessExecutionResult) -> Self {
        FakeHarnessRunnerService {
            result_to_return: Mutex::new(result),
            last_request: Mutex::new(None),
            call_count: AtomicI32::new(0),
        }
    }

    fn call_count(&self) -> i32 {
        self.call_count.load(Ordering::SeqCst)
    }

    fn last_request(&self) -> Option<HarnessExecutionRequest> {
        self.last_request.lock().unwrap().clone()
    }
}

impl IHarnessRunnerService for FakeHarnessRunnerService {
    fn execute(&self, request: &HarnessExecutionRequest, ct: &CancellationToken) -> Result<HarnessExecutionResult, HarnessError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        *self.last_request.lock().unwrap() = Some(request.clone());
        if ct.is_cancellation_requested() {
            return Err(HarnessError::Canceled);
        }

        Ok(self.result_to_return.lock().unwrap().clone())
    }
}

/// İptal belirtecini bilerek yok sayar: süreç iptalden sonra yine de sonuç döndürebilir.
/// `TaskCompletionSource` karşılığı: sonuç verilene dek `execute` bekler.
struct PendingRunner {
    pending: Mutex<Option<HarnessExecutionResult>>,
    signal: Condvar,
}

impl PendingRunner {
    fn new() -> Self {
        PendingRunner { pending: Mutex::new(None), signal: Condvar::new() }
    }

    fn set_result(&self, result: HarnessExecutionResult) {
        *self.pending.lock().unwrap() = Some(result);
        self.signal.notify_all();
    }
}

impl IHarnessRunnerService for PendingRunner {
    fn execute(&self, _request: &HarnessExecutionRequest, _ct: &CancellationToken) -> Result<HarnessExecutionResult, HarnessError> {
        let mut guard = self.pending.lock().unwrap();
        while guard.is_none() {
            guard = self.signal.wait(guard).unwrap();
        }
        Ok(guard.clone().unwrap())
    }
}

struct ThrowingRunner(HarnessError);

impl IHarnessRunnerService for ThrowingRunner {
    fn execute(&self, _request: &HarnessExecutionRequest, _ct: &CancellationToken) -> Result<HarnessExecutionResult, HarnessError> {
        Err(self.0.clone())
    }
}

fn controller(runner: Arc<dyn IHarnessRunnerService>) -> VoiceSessionController {
    controller_with(runner, VoiceSessionOptions::default())
}

fn controller_with(runner: Arc<dyn IHarnessRunnerService>, options: VoiceSessionOptions) -> VoiceSessionController {
    VoiceSessionController::new(runner, move || options.clone())
}

#[test]
fn shortcut_opens_box_then_runs_typed_prompt() {
    let runner = Arc::new(FakeHarnessRunnerService::new());
    let controller =
        controller_with(runner.clone(), VoiceSessionOptions { harness_type: VoiceHarnessType::Claude, ..VoiceSessionOptions::default() });

    controller.trigger_shortcut();
    assert_eq!(VoiceState::Editing, controller.state());

    // Windows sesle yazma kutuya parça parça yazar.
    controller.update_prompt("testleri");
    controller.update_prompt("testleri çalıştır ");

    controller.trigger_shortcut();

    assert_eq!(VoiceState::Completed, controller.state());
    let request = runner.last_request();
    assert_eq!(Some("testleri çalıştır"), request.as_ref().map(|r| r.prompt.as_str()));
    assert_eq!(Some(VoiceHarnessType::Claude), request.as_ref().map(|r| r.harness_type));
}

#[test]
fn enter_on_empty_box_does_nothing_and_keeps_box_open() {
    let runner = Arc::new(FakeHarnessRunnerService::new());
    let controller = controller(runner.clone());

    controller.start();
    controller.update_prompt("   ");
    controller.confirm();

    assert_eq!(VoiceState::Editing, controller.state());
    assert_eq!(0, runner.call_count());
}

#[test]
fn shortcut_on_empty_box_closes_it() {
    let controller = controller(Arc::new(FakeHarnessRunnerService::new()));

    controller.start();
    controller.trigger_shortcut();

    assert_eq!(VoiceState::Idle, controller.state());
}

#[test]
fn text_outside_the_box_is_ignored() {
    let controller = controller(Arc::new(FakeHarnessRunnerService::new()));

    controller.update_prompt("geç gelen yazı");

    assert_eq!(VoiceState::Idle, controller.state());
    assert_eq!("", controller.prompt());
}

#[test]
fn reopening_box_clears_previous_prompt_and_result() {
    let controller = controller(Arc::new(FakeHarnessRunnerService::new()));

    controller.start();
    controller.update_prompt("ilk komut");
    controller.confirm();
    assert_eq!(VoiceState::Completed, controller.state());

    controller.trigger_shortcut();

    assert_eq!(VoiceState::Editing, controller.state());
    assert_eq!("", controller.prompt());
    assert!(controller.last_result().is_none());
}

#[test]
fn runner_failure_sets_failed_with_exit_code() {
    let runner = Arc::new(FakeHarnessRunnerService::returning(HarnessExecutionResult::new(
        false,
        1,
        "",
        "Command failed",
        TimeSpan::from_secs(1),
    )));
    let controller = controller(runner);

    controller.start();
    controller.update_prompt("hatalı komut");
    controller.confirm();

    assert_eq!(VoiceState::Failed, controller.state());
    assert_eq!(VoiceFailure::ExitCode, controller.failure());
    assert_eq!(Some("Command failed".to_string()), controller.last_result().map(|r| r.error));
}

#[test]
fn runner_failure_code_and_detail_are_passed_on() {
    let runner = Arc::new(FakeHarnessRunnerService::returning(HarnessExecutionResult {
        failure: VoiceFailure::CommandNotFound,
        detail: "agy".to_string(),
        ..HarnessExecutionResult::new(false, -1, "", "", TimeSpan::ZERO)
    }));
    let controller = controller(runner);

    controller.start();
    controller.update_prompt("durum");
    controller.confirm();

    assert_eq!(VoiceFailure::CommandNotFound, controller.failure());
    assert_eq!("agy", controller.failure_detail());
}

#[test]
fn invalid_custom_command_reports_invalid_command() {
    let controller = controller(Arc::new(ThrowingRunner(HarnessError::Command(VoiceCommandException::new("boş")))));

    controller.start();
    controller.update_prompt("bir şey yap");
    controller.confirm();

    assert_eq!(VoiceState::Failed, controller.state());
    assert_eq!(VoiceFailure::InvalidCommand, controller.failure());
}

fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "bekleme zaman aşımı");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn result_arriving_after_cancel_does_not_override_idle() {
    let runner = Arc::new(PendingRunner::new());
    let controller = Arc::new(controller(runner.clone()));

    controller.start();
    controller.update_prompt("uzun iş");
    let running = {
        let controller = Arc::clone(&controller);
        thread::spawn(move || controller.confirm())
    };
    wait_for(|| controller.state() == VoiceState::Running);
    assert_eq!(VoiceState::Running, controller.state());

    controller.trigger_shortcut();
    assert_eq!(VoiceState::Idle, controller.state());

    runner.set_result(HarnessExecutionResult::new(true, 0, "", "", TimeSpan::ZERO));
    running.join().unwrap();

    assert_eq!(VoiceState::Idle, controller.state());
    assert!(controller.last_result().is_none());
}

#[test]
fn cancel_closes_box_and_clears_prompt() {
    let controller = controller(Arc::new(FakeHarnessRunnerService::new()));

    controller.start();
    controller.update_prompt("vazgeçtim");
    controller.cancel();

    assert_eq!(VoiceState::Idle, controller.state());
    assert_eq!("", controller.prompt());
}
