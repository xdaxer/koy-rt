use crate::voice::path_util;
use crate::voice::{HarnessExecutableKind, HarnessExecutableResolver};
use std::collections::HashSet;

fn npm_dir() -> String {
    path_util::combine("C:", "npm")
}

fn tools_dir() -> String {
    path_util::combine("C:", "tools")
}

/// `HashSet<string>(StringComparer.OrdinalIgnoreCase)` karşılığı: yollar büyük harfe çevrilip saklanır.
fn files(paths: &[String]) -> impl Fn(&str) -> bool + use<> {
    let set: HashSet<String> = paths.iter().map(|p| p.to_uppercase()).collect();
    move |p: &str| set.contains(&p.to_uppercase())
}

#[test]
fn resolve_prefers_native_exe_over_script_shim_in_same_directory() {
    let exists = files(&[path_util::combine(&npm_dir(), "codex.cmd"), path_util::combine(&npm_dir(), "codex.exe")]);

    let result = HarnessExecutableResolver::resolve("codex", [npm_dir()], exists);

    assert_eq!(HarnessExecutableKind::Found, result.kind);
    assert_eq!(path_util::combine(&npm_dir(), "codex.exe"), result.path);
}

#[test]
fn resolve_only_script_shim_reports_script_not_supported() {
    let exists = files(&[path_util::combine(&npm_dir(), "codex.cmd")]);

    let result = HarnessExecutableResolver::resolve("codex", [npm_dir()], exists);

    assert_eq!(HarnessExecutableKind::ScriptNotSupported, result.kind);
    assert_eq!(path_util::combine(&npm_dir(), "codex.cmd"), result.path);
}

#[test]
fn resolve_first_path_directory_wins() {
    let exists = files(&[path_util::combine(&npm_dir(), "claude.cmd"), path_util::combine(&tools_dir(), "claude.exe")]);

    let result = HarnessExecutableResolver::resolve("claude", [npm_dir(), tools_dir()], exists);

    assert_eq!(HarnessExecutableKind::ScriptNotSupported, result.kind);
}

#[test]
fn resolve_explicit_script_extension_reports_script_not_supported() {
    let path = path_util::combine(&tools_dir(), "run.bat");

    let result = HarnessExecutableResolver::resolve(&path, Vec::<String>::new(), |p| p == path);

    assert_eq!(HarnessExecutableKind::ScriptNotSupported, result.kind);
    assert_eq!(path, result.path);
}

#[test]
fn resolve_rooted_exe_path_is_used_directly() {
    let path = path_util::combine(&tools_dir(), "agy.exe");

    let result = HarnessExecutableResolver::resolve(&path, [npm_dir()], |p| p == path);

    assert_eq!(HarnessExecutableKind::Found, result.kind);
    assert_eq!(path, result.path);
}

#[test]
fn resolve_extensionless_shell_script_is_not_treated_as_runnable() {
    // npm, aynı klasöre uzantısız sh betiği de koyar; Windows bunu çalıştıramaz.
    let exists = files(&[path_util::combine(&npm_dir(), "codex")]);

    let result = HarnessExecutableResolver::resolve("codex", [npm_dir()], exists);

    assert_eq!(HarnessExecutableKind::NotFound, result.kind);
}

#[test]
fn resolve_missing_returns_not_found() {
    let result = HarnessExecutableResolver::resolve("yok", [npm_dir(), tools_dir()], |_| false);

    assert_eq!(HarnessExecutableKind::NotFound, result.kind);
    assert_eq!("yok", result.path);
}
