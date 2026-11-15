//! Karşılığı: `koy/src/Core/StartMenu/StartMenuModel.cs`.

use crate::search::SystemCommand;

/// Başlat düğmesi sağ tık menüsündeki (Win+X) öğeler. Ad, çeviri anahtarının sonudur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StartMenuItemId {
    #[default]
    InstalledApps,
    MobilityCenter,
    PowerOptions,
    EventViewer,
    System,
    DeviceManager,
    NetworkConnections,
    DiskManagement,
    ComputerManagement,
    Terminal,
    TerminalAdmin,
    TaskManager,
    Settings,
    FileExplorer,
    Search,
    Run,
    ShutDownOrSignOut,
    SignOut,
    Sleep,
    ShutDown,
    Restart,
    Desktop,
}

impl StartMenuItemId {
    /// `Enum.GetValues<StartMenuItemId>()`: tanım sırasıyla tüm değerler.
    pub fn all() -> &'static [StartMenuItemId] {
        use StartMenuItemId::*;
        &[
            InstalledApps, MobilityCenter, PowerOptions, EventViewer, System, DeviceManager, NetworkConnections, DiskManagement,
            ComputerManagement, Terminal, TerminalAdmin, TaskManager, Settings, FileExplorer, Search, Run, ShutDownOrSignOut, SignOut,
            Sleep, ShutDown, Restart, Desktop,
        ]
    }

    /// `ToString()`: çeviri anahtarının sonu.
    pub fn name(self) -> &'static str {
        match self {
            StartMenuItemId::InstalledApps => "InstalledApps",
            StartMenuItemId::MobilityCenter => "MobilityCenter",
            StartMenuItemId::PowerOptions => "PowerOptions",
            StartMenuItemId::EventViewer => "EventViewer",
            StartMenuItemId::System => "System",
            StartMenuItemId::DeviceManager => "DeviceManager",
            StartMenuItemId::NetworkConnections => "NetworkConnections",
            StartMenuItemId::DiskManagement => "DiskManagement",
            StartMenuItemId::ComputerManagement => "ComputerManagement",
            StartMenuItemId::Terminal => "Terminal",
            StartMenuItemId::TerminalAdmin => "TerminalAdmin",
            StartMenuItemId::TaskManager => "TaskManager",
            StartMenuItemId::Settings => "Settings",
            StartMenuItemId::FileExplorer => "FileExplorer",
            StartMenuItemId::Search => "Search",
            StartMenuItemId::Run => "Run",
            StartMenuItemId::ShutDownOrSignOut => "ShutDownOrSignOut",
            StartMenuItemId::SignOut => "SignOut",
            StartMenuItemId::Sleep => "Sleep",
            StartMenuItemId::ShutDown => "ShutDown",
            StartMenuItemId::Restart => "Restart",
            StartMenuItemId::Desktop => "Desktop",
        }
    }
}

/// Öğe tıklanınca ne tür bir iş yapılır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StartMenuActionKind {
    /// [`StartMenuItem::target`] kabuk üzerinden açılır (ms-settings:, .msc, .exe).
    #[default]
    OpenTarget,
    /// Windows Terminal; yoksa PowerShell.
    Terminal,
    /// Terminal, yönetici olarak (UAC istemi çıkar).
    TerminalAdmin,
    /// Oturum komutu ([`StartMenuItem::session`]).
    Session,
    /// Alt menü açar; kendi başına iş yapmaz.
    Submenu,
    ShowDesktop,
    Search,
    Run,
}

/// Menü öğesi. Hedefler saf metindir; Windows'ta çalıştırmak `IStartMenuHost`'un işi.
#[derive(Debug, Clone, PartialEq)]
pub struct StartMenuItem {
    pub id: StartMenuItemId,
    pub kind: StartMenuActionKind,
    pub target: Option<String>,
    pub session: Option<SystemCommand>,
    pub children: Option<Vec<StartMenuItem>>,
}

impl StartMenuItem {
    /// C# kayıt kurucusu: `Target`, `Session` ve `Children` varsayılan olarak null.
    fn new(id: StartMenuItemId, kind: StartMenuActionKind) -> Self {
        StartMenuItem { id, kind, target: None, session: None, children: None }
    }
}

/// Ayırıcıyla ayrılan öğe grubu.
#[derive(Debug, Clone, PartialEq)]
pub struct StartMenuGroup {
    pub items: Vec<StartMenuItem>,
}

/// Windows 11'in Başlat sağ tık menüsünün sırası ve grupları.
pub struct StartMenuModel;

impl StartMenuModel {
    /// Menüyü kurar. Mobilite Merkezi yalnız pili olan makinede çıkar (Windows da böyle yapar).
    pub fn build(has_battery: bool) -> Vec<StartMenuGroup> {
        let mut system = vec![open(StartMenuItemId::InstalledApps, "ms-settings:appsfeatures")];

        if has_battery {
            system.push(open(StartMenuItemId::MobilityCenter, "mblctr.exe"));
        }

        system.extend([
            // Windows 11'de Güç Seçenekleri, Sistem ve Ağ Bağlantıları Ayarlar'ın ilgili sayfasını açar.
            open(StartMenuItemId::PowerOptions, "ms-settings:powersleep"),
            open(StartMenuItemId::EventViewer, "eventvwr.msc"),
            open(StartMenuItemId::System, "ms-settings:about"),
            open(StartMenuItemId::DeviceManager, "devmgmt.msc"),
            open(StartMenuItemId::NetworkConnections, "ms-settings:network-status"),
            open(StartMenuItemId::DiskManagement, "diskmgmt.msc"),
            open(StartMenuItemId::ComputerManagement, "compmgmt.msc"),
        ]);

        let terminal = vec![
            StartMenuItem::new(StartMenuItemId::Terminal, StartMenuActionKind::Terminal),
            StartMenuItem::new(StartMenuItemId::TerminalAdmin, StartMenuActionKind::TerminalAdmin),
        ];

        let tools = vec![
            open(StartMenuItemId::TaskManager, "taskmgr.exe"),
            open(StartMenuItemId::Settings, "ms-settings:"),
            open(StartMenuItemId::FileExplorer, "explorer.exe"),
            StartMenuItem::new(StartMenuItemId::Search, StartMenuActionKind::Search),
            StartMenuItem::new(StartMenuItemId::Run, StartMenuActionKind::Run),
        ];

        let session = vec![StartMenuItem {
            children: Some(vec![
                session_item(StartMenuItemId::SignOut, SystemCommand::SignOut),
                session_item(StartMenuItemId::Sleep, SystemCommand::Sleep),
                session_item(StartMenuItemId::ShutDown, SystemCommand::Shutdown),
                session_item(StartMenuItemId::Restart, SystemCommand::Restart),
            ]),
            ..StartMenuItem::new(StartMenuItemId::ShutDownOrSignOut, StartMenuActionKind::Submenu)
        }];

        let desktop = vec![StartMenuItem::new(StartMenuItemId::Desktop, StartMenuActionKind::ShowDesktop)];

        vec![
            StartMenuGroup { items: system },
            StartMenuGroup { items: terminal },
            StartMenuGroup { items: tools },
            StartMenuGroup { items: session },
            StartMenuGroup { items: desktop },
        ]
    }
}

fn open(id: StartMenuItemId, target: &str) -> StartMenuItem {
    StartMenuItem { target: Some(target.to_string()), ..StartMenuItem::new(id, StartMenuActionKind::OpenTarget) }
}

fn session_item(id: StartMenuItemId, command: SystemCommand) -> StartMenuItem {
    StartMenuItem { session: Some(command), ..StartMenuItem::new(id, StartMenuActionKind::Session) }
}
