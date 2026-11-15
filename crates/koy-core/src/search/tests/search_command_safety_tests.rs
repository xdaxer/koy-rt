use super::support::{q, translate, FakeShell};
use crate::search::*;
use crate::search_source_base_methods;
use std::sync::Arc;

fn t() -> Translate {
    translate(|key| format!("{key}|{{0}}"))
}

/// C#'taki `FakeShell`: yalnız `C:\Tools\build.CMD` var; PATH ve PATHEXT sabit.
fn build_shell() -> Arc<FakeShell> {
    let shell = FakeShell::new();
    shell.add_file(r"C:\Tools\build.CMD");
    shell.set_env("PATH", r"C:\Tools");
    shell.set_env("PATHEXT", ".CMD");
    shell
}

#[test]
fn control_characters_are_detected() {
    let cases = [
        ("build -x", false),
        ("build\t-x", false),
        ("build -x\ncalc", true),
        ("build\r\n", false), // sondaki satır sonu kırpılır
        ("build -x\u{7}", true),
    ];
    for (text, expected) in cases {
        assert_eq!(expected, SearchQuery::from_text(text).needs_command_confirmation(), "{text:?}");
    }
}

#[test]
fn clipboard_query_always_needs_confirmation() {
    assert!(SearchQuery::new(Some("build -x"), true).needs_command_confirmation());
}

#[test]
fn display_marks_newlines() {
    assert_eq!("a⏎b⏎c d", SearchCommandSafety::for_display("a\r\nb\rc\u{1}d"));
}

#[test]
fn single_line_has_no_line_breaks() {
    let line = SearchCommandSafety::to_single_line("build -x\r\ncalc\n\nformat");
    assert!(!line.contains('\n'));
    assert!(!line.contains('\r'));
}

#[test]
fn typed_single_line_command_runs_without_confirmation() {
    let results = ShellCommandSearchSource::new(build_shell(), t()).search(&q("build -x"));
    assert_eq!(1, results.len());
    let result = &results[0];

    assert!(!result.requires_confirmation);
    assert!(!result.actions[0].requires_confirmation);
}

#[test]
fn multiline_command_needs_confirmation_shows_all_text_and_sends_no_newline() {
    let shell = build_shell();
    let results = ShellCommandSearchSource::new(shell.clone(), t()).search(&q("build -x\ncalc"));
    assert_eq!(1, results.len());
    let result = &results[0];

    assert!(result.requires_confirmation);
    assert!(result.actions[0].requires_confirmation);
    assert_eq!(Some("build -x⏎calc"), result.confirmation_detail.as_deref());
    assert!(result.title.contains('⏎'));

    (result.execute)();
    (result.actions[0].execute)();

    assert!(!shell.log().is_empty());
    for entry in shell.log() {
        assert!(!entry.contains('\n'), "{entry:?}");
    }
}

#[test]
fn clipboard_command_needs_confirmation_for_both_enter_and_terminal() {
    let results = ShellCommandSearchSource::new(build_shell(), t()).search(&SearchQuery::new(Some("build -x"), true));
    assert_eq!(1, results.len());
    let result = &results[0];

    assert!(result.requires_confirmation);
    assert!(result.actions[0].requires_confirmation);
    assert_eq!(Some("build -x"), result.confirmation_detail.as_deref());
}

// --- Denetleyici: pano kaynağı ve eylem onayı ---

/// C# testindeki `ShellSource`: iç kaynağa yönlendiren, kimliği "shell" olan kaynak.
struct ShellSource {
    base: SearchSourceBase,
    inner: ShellCommandSearchSource,
}

impl ShellSource {
    fn new(shell: Arc<FakeShell>) -> Arc<dyn ISearchSource> {
        Arc::new(ShellSource { base: SearchSourceBase::new("shell", "Shell"), inner: ShellCommandSearchSource::new(shell, t()) })
    }
}

impl ISearchSource for ShellSource {
    search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {}

    fn release(&self) {}

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        self.inner.search(query)
    }
}

fn panel(shell: &Arc<FakeShell>) -> SearchPanelController {
    SearchPanelController::new(vec![ShellSource::new(shell.clone())], None, None, None, "", "")
}

#[test]
fn clipboard_text_does_not_run_on_first_enter() {
    let shell = build_shell();
    let panel = panel(&shell);
    panel.open();
    panel.replace_query("build -x", true);

    assert_eq!(SearchActivation::NeedsConfirmation, panel.activate(false).outcome);
    assert!(shell.log().is_empty());
    assert_eq!(SearchActivation::Executed, panel.activate(false).outcome);
    assert_eq!(1, shell.log().len());
}

#[test]
fn terminal_action_confirms_separately_from_the_row() {
    let shell = build_shell();
    let panel = panel(&shell);
    panel.open();
    panel.replace_query("build -x", true);

    panel.activate(false); // satır için onay bekler
    assert_eq!(SearchActivation::NeedsConfirmation, panel.activate(true).outcome);
    assert!(shell.log().is_empty());
    assert_eq!(SearchActivation::Executed, panel.activate(true).outcome);
    assert_eq!(vec!["term build -x"], shell.log());
}

#[test]
fn clearing_the_box_ends_clipboard_origin_and_typed_text_runs_directly() {
    let shell = build_shell();
    let panel = panel(&shell);
    panel.open();
    panel.replace_query("build -x", true);
    panel.set_query(Some(""));
    panel.set_query(Some("build -y"));

    assert!(!panel.query_from_clipboard());
    assert_eq!(SearchActivation::Executed, panel.activate(false).outcome);
}

#[test]
fn clipboard_text_is_not_kept_for_next_open() {
    let shell = build_shell();
    let panel = panel(&shell);
    panel.open();
    panel.replace_query("build -x", true);
    panel.close();
    panel.open();

    assert_eq!("", panel.query());
}

// --- PATH çözümleme önbelleği ---

/// C# testindeki `CountingShell`: ağ klasörlü PATH; dosya yoklamaları sayılır.
fn counting_shell() -> Arc<FakeShell> {
    let shell = FakeShell::new();
    shell.add_file(r"C:\Tools\build.CMD");
    shell.set_env("PATH", r"\\nas\share;C:\Tools");
    shell.set_env("PATHEXT", ".CMD");
    shell
}

#[test]
fn path_lookup_is_cached_until_release_and_skips_network_directories() {
    let shell = counting_shell();
    let source = ShellCommandSearchSource::new(shell.clone(), t());

    assert_eq!(1, source.search(&q("build a")).len());
    let after_first = shell.file_checks();
    assert_eq!(1, source.search(&q("build a b")).len());
    assert_eq!(after_first, shell.file_checks());

    source.search(&q("nope"));
    let after_miss = shell.file_checks();
    assert!(after_miss > after_first);
    source.search(&q("nope x"));
    assert_eq!(after_miss, shell.file_checks());
    assert!(!shell.checked.lock().unwrap().iter().any(|c| c.starts_with(r"\\")));

    source.release();
    assert_eq!(1, source.search(&q("build a")).len());
    assert!(shell.file_checks() > after_miss);
}

#[test]
fn network_paths_are_recognised() {
    let cases = [(r"\\nas\share\x.exe", true), ("//nas/share", true), (r"C:\Tools", false)];
    for (path, expected) in cases {
        assert_eq!(expected, ShellCommandSearchSource::is_network_path(path), "{path}");
    }
}
