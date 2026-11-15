//! Karşılığı: `koy/tests/Core.Tests/StartMenu/StartMenuTests.cs`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::search::{ISystemCommandService, SystemCommand};
use crate::start_menu::{
    IStartMenuHost, StartMenuActionKind, StartMenuActions, StartMenuErrorHandler, StartMenuGroup, StartMenuItem, StartMenuItemId,
    StartMenuModel, ToolLaunchResult,
};

// ---- StartMenuModelTests ----

fn flat(groups: &[StartMenuGroup]) -> Vec<StartMenuItemId> {
    groups.iter().flat_map(|g| g.items.iter()).map(|i| i.id).collect()
}

fn ids(items: &[StartMenuItem]) -> Vec<StartMenuItemId> {
    items.iter().map(|i| i.id).collect()
}

/// `SelectMany(i => i.Children is null ? [i] : new[] { i }.Concat(i.Children))`.
fn with_children(groups: &[StartMenuGroup]) -> Vec<StartMenuItem> {
    let mut all = Vec::new();
    for item in groups.iter().flat_map(|g| g.items.iter()) {
        all.push(item.clone());
        if let Some(children) = &item.children {
            all.extend(children.iter().cloned());
        }
    }
    all
}

#[test]
fn layout_follows_windows11_order_and_grouping() {
    use StartMenuItemId::*;
    let groups = StartMenuModel::build(true);

    assert_eq!(5, groups.len());
    assert_eq!(
        vec![InstalledApps, MobilityCenter, PowerOptions, EventViewer, System, DeviceManager, NetworkConnections, DiskManagement, ComputerManagement],
        ids(&groups[0].items)
    );
    assert_eq!(vec![Terminal, TerminalAdmin], ids(&groups[1].items));
    assert_eq!(vec![TaskManager, Settings, FileExplorer, Search, Run], ids(&groups[2].items));
    assert_eq!(vec![ShutDownOrSignOut], ids(&groups[3].items));
    assert_eq!(vec![Desktop], ids(&groups[4].items));
}

#[test]
fn mobility_center_hidden_without_battery_rest_unchanged() {
    let with_battery = flat(&StartMenuModel::build(true));
    let without = flat(&StartMenuModel::build(false));

    assert!(!without.contains(&StartMenuItemId::MobilityCenter));
    let expected: Vec<_> = with_battery.into_iter().filter(|id| *id != StartMenuItemId::MobilityCenter).collect();
    assert_eq!(expected, without);
}

#[test]
fn shut_down_or_sign_out_is_submenu_with_session_commands_in_order() {
    let groups = StartMenuModel::build(true);
    let parents: Vec<_> = groups.iter().flat_map(|g| g.items.iter()).filter(|i| i.id == StartMenuItemId::ShutDownOrSignOut).collect();
    assert_eq!(1, parents.len());
    let parent = parents[0];

    assert_eq!(StartMenuActionKind::Submenu, parent.kind);
    let children = parent.children.as_ref().expect("alt öğeler yok");
    assert_eq!(
        vec![StartMenuItemId::SignOut, StartMenuItemId::Sleep, StartMenuItemId::ShutDown, StartMenuItemId::Restart],
        ids(children)
    );
    assert_eq!(
        vec![SystemCommand::SignOut, SystemCommand::Sleep, SystemCommand::Shutdown, SystemCommand::Restart],
        children.iter().map(|c| c.session.unwrap()).collect::<Vec<_>>()
    );
    assert!(children.iter().all(|c| c.kind == StartMenuActionKind::Session));
}

#[test]
fn action_kinds_and_targets() {
    use StartMenuItemId::*;
    let items: HashMap<StartMenuItemId, StartMenuItem> =
        StartMenuModel::build(true).into_iter().flat_map(|g| g.items).map(|i| (i.id, i)).collect();

    assert_eq!(StartMenuActionKind::Terminal, items[&Terminal].kind);
    assert_eq!(StartMenuActionKind::TerminalAdmin, items[&TerminalAdmin].kind);
    assert_eq!(StartMenuActionKind::Search, items[&Search].kind);
    assert_eq!(StartMenuActionKind::Run, items[&Run].kind);
    assert_eq!(StartMenuActionKind::ShowDesktop, items[&Desktop].kind);

    let targets = [
        (InstalledApps, "ms-settings:appsfeatures"),
        (MobilityCenter, "mblctr.exe"),
        (PowerOptions, "ms-settings:powersleep"),
        (EventViewer, "eventvwr.msc"),
        (System, "ms-settings:about"),
        (DeviceManager, "devmgmt.msc"),
        (NetworkConnections, "ms-settings:network-status"),
        (DiskManagement, "diskmgmt.msc"),
        (ComputerManagement, "compmgmt.msc"),
        (TaskManager, "taskmgr.exe"),
        (Settings, "ms-settings:"),
        (FileExplorer, "explorer.exe"),
    ];
    for (id, target) in targets {
        assert_eq!(Some(target), items[&id].target.as_deref(), "{id:?}");
    }
}

#[test]
fn every_open_target_item_has_target_and_ids_are_unique() {
    let all = with_children(&StartMenuModel::build(true));

    for item in all.iter().filter(|i| i.kind == StartMenuActionKind::OpenTarget) {
        assert!(!item.target.as_deref().unwrap_or("").is_empty(), "{:?}", item.id);
    }
    let mut distinct = ids(&all);
    distinct.sort_by_key(|id| id.name());
    distinct.dedup();
    assert_eq!(all.len(), distinct.len());
    // Pilli makinede her kimlik menüde tam bir kez geçer.
    assert_eq!(StartMenuItemId::all().len(), all.len());
}

// ---- StartMenuActionsTests ----

#[derive(Default)]
struct FakeHost {
    has_battery: AtomicBool,
    throw_on_battery: AtomicBool,
    throw_on_launch: AtomicBool,
    results: Mutex<HashMap<String, ToolLaunchResult>>,
    launches: Mutex<Vec<(String, bool)>>,
    calls: Mutex<Vec<String>>,
}

impl FakeHost {
    fn new() -> Arc<FakeHost> {
        let host = FakeHost::default();
        host.has_battery.store(true, Ordering::SeqCst);
        Arc::new(host)
    }

    fn launches(&self) -> Vec<(String, bool)> {
        self.launches.lock().unwrap().clone()
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl IStartMenuHost for FakeHost {
    fn has_battery(&self) -> bool {
        if self.throw_on_battery.load(Ordering::SeqCst) {
            panic!("pil");
        }
        self.has_battery.load(Ordering::SeqCst)
    }

    fn launch(&self, target: &str, _arguments: Option<&str>, elevated: bool) -> ToolLaunchResult {
        if self.throw_on_launch.load(Ordering::SeqCst) {
            panic!("boom");
        }

        self.launches.lock().unwrap().push((target.to_string(), elevated));
        self.results.lock().unwrap().get(target).copied().unwrap_or(ToolLaunchResult::Started)
    }

    fn show_desktop(&self) {
        self.calls.lock().unwrap().push("desktop".to_string());
    }

    fn open_search(&self) {
        self.calls.lock().unwrap().push("search".to_string());
    }

    fn open_run_dialog(&self) {
        self.calls.lock().unwrap().push("run".to_string());
    }
}

#[derive(Default)]
struct FakeSession {
    executed: Mutex<Vec<SystemCommand>>,
}

impl ISystemCommandService for FakeSession {
    fn execute(&self, command: SystemCommand) {
        self.executed.lock().unwrap().push(command);
    }
}

fn item(id: StartMenuItemId) -> StartMenuItem {
    with_children(&StartMenuModel::build(true)).into_iter().find(|i| i.id == id).expect("öğe yok")
}

fn collecting_errors() -> (Arc<Mutex<Vec<String>>>, StartMenuErrorHandler) {
    let errors = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&errors);
    (errors, Arc::new(move |source: &str, _message: &str| sink.lock().unwrap().push(source.to_string())))
}

fn actions(host: &Arc<FakeHost>, session: &Arc<FakeSession>, on_error: Option<StartMenuErrorHandler>) -> StartMenuActions {
    StartMenuActions::new(host.clone(), session.clone(), on_error)
}

fn pairs(expected: &[(&str, bool)]) -> Vec<(String, bool)> {
    expected.iter().map(|(t, e)| (t.to_string(), *e)).collect()
}

#[test]
fn build_menu_uses_host_battery_state() {
    let host = FakeHost::new();
    host.has_battery.store(false, Ordering::SeqCst);
    let actions = actions(&host, &Arc::new(FakeSession::default()), None);

    assert!(!flat(&actions.build_menu()).contains(&StartMenuItemId::MobilityCenter));

    host.has_battery.store(true, Ordering::SeqCst);
    assert!(flat(&actions.build_menu()).contains(&StartMenuItemId::MobilityCenter));
}

#[test]
fn build_menu_battery_query_failure_still_builds_menu_without_mobility_center() {
    let host = FakeHost::new();
    host.throw_on_battery.store(true, Ordering::SeqCst);
    let (errors, handler) = collecting_errors();
    let actions = actions(&host, &Arc::new(FakeSession::default()), Some(handler));

    let ids = flat(&actions.build_menu());

    assert!(!ids.contains(&StartMenuItemId::MobilityCenter));
    assert!(ids.contains(&StartMenuItemId::Desktop));
    assert_eq!(1, errors.lock().unwrap().len());
}

#[test]
fn open_target_launches_target_not_elevated() {
    let host = FakeHost::new();
    actions(&host, &Arc::new(FakeSession::default()), None).execute(&item(StartMenuItemId::DeviceManager));

    assert_eq!(pairs(&[("devmgmt.msc", false)]), host.launches());
}

#[test]
fn terminal_prefers_windows_terminal() {
    let host = FakeHost::new();
    actions(&host, &Arc::new(FakeSession::default()), None).execute(&item(StartMenuItemId::Terminal));

    assert_eq!(pairs(&[("wt.exe", false)]), host.launches());
}

#[test]
fn terminal_falls_back_to_power_shell_when_windows_terminal_unavailable() {
    for result in [ToolLaunchResult::NotFound, ToolLaunchResult::Failed] {
        let host = FakeHost::new();
        host.results.lock().unwrap().insert("wt.exe".to_string(), result);
        actions(&host, &Arc::new(FakeSession::default()), None).execute(&item(StartMenuItemId::Terminal));

        assert_eq!(pairs(&[("wt.exe", false), ("powershell.exe", false)]), host.launches(), "{result:?}");
    }
}

#[test]
fn terminal_admin_runs_elevated_and_falls_back_elevated() {
    let host = FakeHost::new();
    host.results.lock().unwrap().insert("wt.exe".to_string(), ToolLaunchResult::NotFound);
    actions(&host, &Arc::new(FakeSession::default()), None).execute(&item(StartMenuItemId::TerminalAdmin));

    assert_eq!(pairs(&[("wt.exe", true), ("powershell.exe", true)]), host.launches());
}

#[test]
fn terminal_admin_declined_uac_is_silent_no_fallback_no_error() {
    let host = FakeHost::new();
    host.results.lock().unwrap().insert("wt.exe".to_string(), ToolLaunchResult::Declined);
    let (errors, handler) = collecting_errors();

    actions(&host, &Arc::new(FakeSession::default()), Some(handler)).execute(&item(StartMenuItemId::TerminalAdmin));

    assert_eq!(pairs(&[("wt.exe", true)]), host.launches());
    assert!(errors.lock().unwrap().is_empty());
}

#[test]
fn session_commands_go_to_session_service_not_to_host() {
    let host = FakeHost::new();
    let session = Arc::new(FakeSession::default());
    let actions = actions(&host, &session, None);

    actions.execute(&item(StartMenuItemId::SignOut));
    actions.execute(&item(StartMenuItemId::Sleep));
    actions.execute(&item(StartMenuItemId::ShutDown));
    actions.execute(&item(StartMenuItemId::Restart));

    assert_eq!(
        vec![SystemCommand::SignOut, SystemCommand::Sleep, SystemCommand::Shutdown, SystemCommand::Restart],
        *session.executed.lock().unwrap()
    );
    assert!(host.launches().is_empty());
    assert!(host.calls().is_empty());
}

#[test]
fn shell_shortcuts_desktop_search_run() {
    let host = FakeHost::new();
    let actions = actions(&host, &Arc::new(FakeSession::default()), None);

    actions.execute(&item(StartMenuItemId::Desktop));
    actions.execute(&item(StartMenuItemId::Search));
    actions.execute(&item(StartMenuItemId::Run));

    assert_eq!(vec!["desktop", "search", "run"], host.calls());
}

#[test]
fn submenu_parent_does_nothing() {
    let host = FakeHost::new();
    let session = Arc::new(FakeSession::default());
    actions(&host, &session, None).execute(&item(StartMenuItemId::ShutDownOrSignOut));

    assert!(host.launches().is_empty());
    assert!(host.calls().is_empty());
    assert!(session.executed.lock().unwrap().is_empty());
}

#[test]
fn launch_exception_never_escapes_and_is_reported() {
    let host = FakeHost::new();
    host.throw_on_launch.store(true, Ordering::SeqCst);
    let (errors, handler) = collecting_errors();
    let actions = actions(&host, &Arc::new(FakeSession::default()), Some(handler));

    actions.execute(&item(StartMenuItemId::TaskManager));
    actions.execute(&item(StartMenuItemId::Terminal));

    assert_eq!(2, errors.lock().unwrap().len());
}
