use crate::shell::{TaskbarWindowFacts, TaskbarWindowRule};

fn plain() -> TaskbarWindowFacts {
    TaskbarWindowFacts {
        is_visible: true,
        is_cloaked: false,
        is_shell_window: false,
        has_owner: false,
        is_app_window: false,
        is_tool_window: false,
        is_no_activate: false,
    }
}

#[test]
fn plain_visible_unowned_window_is_shown() {
    assert!(TaskbarWindowRule::should_show(&plain()));
}

#[test]
fn invisible_window_is_hidden_even_with_appwindow() {
    assert!(!TaskbarWindowRule::should_show(&TaskbarWindowFacts { is_visible: false, is_app_window: true, ..plain() }));
}

#[test]
fn cloaked_window_is_hidden_even_with_appwindow() {
    assert!(!TaskbarWindowRule::should_show(&TaskbarWindowFacts { is_cloaked: true, is_app_window: true, ..plain() }));
}

#[test]
fn shell_window_is_hidden() {
    assert!(!TaskbarWindowRule::should_show(&TaskbarWindowFacts { is_shell_window: true, is_app_window: true, ..plain() }));
}

#[test]
fn appwindow_style_wins_over_owner_toolwindow_and_noactivate() {
    let facts = TaskbarWindowFacts { is_app_window: true, has_owner: true, is_tool_window: true, is_no_activate: true, ..plain() };
    assert!(TaskbarWindowRule::should_show(&facts));
}

#[test]
fn owned_window_without_appwindow_is_hidden() {
    assert!(!TaskbarWindowRule::should_show(&TaskbarWindowFacts { has_owner: true, ..plain() }));
}

#[test]
fn toolwindow_without_appwindow_is_hidden() {
    assert!(!TaskbarWindowRule::should_show(&TaskbarWindowFacts { is_tool_window: true, ..plain() }));
}

#[test]
fn noactivate_without_appwindow_is_hidden() {
    assert!(!TaskbarWindowRule::should_show(&TaskbarWindowFacts { is_no_activate: true, ..plain() }));
}
