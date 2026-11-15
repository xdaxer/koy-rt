//! Karşılığı: `koy/src/Core/StartMenu/StartMenuActions.cs`.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use super::{StartMenuActionKind, StartMenuGroup, StartMenuItem, StartMenuModel};
use crate::search::ISystemCommandService;

/// Bir aracı başlatma denemesinin sonucu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ToolLaunchResult {
    #[default]
    Started,
    /// Dosya yok ya da kabukta tanımlı değil (ör. Windows Terminal kurulu değil).
    NotFound,
    /// Kullanıcı UAC istemini reddetti; hata değildir.
    Declined,
    Failed,
}

/// Başlat sağ tık menüsünün Windows tarafı. Uygulaması Platform'da; WinRT/Win32 tipi sızmaz.
pub trait IStartMenuHost: Send + Sync {
    /// Makinede pil var mı (Mobilite Merkezi yalnız o zaman gösterilir).
    fn has_battery(&self) -> bool;

    /// Hedefi kabuk üzerinden açar. Hata fırlatmaz, sonucu döner.
    fn launch(&self, target: &str, arguments: Option<&str>, elevated: bool) -> ToolLaunchResult;

    fn show_desktop(&self);

    fn open_search(&self);

    fn open_run_dialog(&self);
}

/// C#'taki `Action<string, Exception>`: kaynak adı ve hata (panik) iletisi.
pub type StartMenuErrorHandler = Arc<dyn Fn(&str, &str) + Send + Sync>;

const WINDOWS_TERMINAL: &str = "wt.exe";
const POWER_SHELL: &str = "powershell.exe";

/// Menü öğesine tıklanınca yapılacak iş. Hiçbir hata dışarı çıkmaz: dock bir başlatma hatasıyla çökmemeli.
/// (C#'taki istisnalar Rust'ta panik olarak yakalanır.)
pub struct StartMenuActions {
    host: Arc<dyn IStartMenuHost>,
    session: Arc<dyn ISystemCommandService>,
    on_error: Option<StartMenuErrorHandler>,
}

impl StartMenuActions {
    pub fn new(host: Arc<dyn IStartMenuHost>, session: Arc<dyn ISystemCommandService>, on_error: Option<StartMenuErrorHandler>) -> Self {
        StartMenuActions { host, session, on_error }
    }

    pub fn build_menu(&self) -> Vec<StartMenuGroup> {
        match catch_unwind(AssertUnwindSafe(|| StartMenuModel::build(self.host.has_battery()))) {
            Ok(menu) => menu,
            Err(payload) => {
                // Pil sorgusu patlarsa menü yine açılsın: pilsiz varsayılır.
                self.report("Başlat menüsü kurulumu", payload.as_ref());
                StartMenuModel::build(false)
            }
        }
    }

    pub fn execute(&self, item: &StartMenuItem) {
        let outcome = catch_unwind(AssertUnwindSafe(|| match item.kind {
            StartMenuActionKind::OpenTarget => {
                if let Some(target) = item.target.as_deref().filter(|t| !t.is_empty()) {
                    self.host.launch(target, None, false);
                }
            }
            StartMenuActionKind::Terminal => self.launch_terminal(false),
            StartMenuActionKind::TerminalAdmin => self.launch_terminal(true),
            StartMenuActionKind::Session => {
                if let Some(command) = item.session {
                    self.session.execute(command);
                }
            }
            StartMenuActionKind::ShowDesktop => self.host.show_desktop(),
            StartMenuActionKind::Search => self.host.open_search(),
            StartMenuActionKind::Run => self.host.open_run_dialog(),
            StartMenuActionKind::Submenu => {}
        }));

        if let Err(payload) = outcome {
            self.report(&format!("Başlat menüsü: {}", item.id.name()), payload.as_ref());
        }
    }

    // Reddedilen UAC istemi sessizce biter; başka her başarısızlıkta PowerShell'e düşülür.
    fn launch_terminal(&self, elevated: bool) {
        let result = self.host.launch(WINDOWS_TERMINAL, None, elevated);
        if matches!(result, ToolLaunchResult::Started | ToolLaunchResult::Declined) {
            return;
        }

        self.host.launch(POWER_SHELL, None, elevated);
    }

    fn report(&self, source: &str, payload: &(dyn std::any::Any + Send)) {
        if let Some(on_error) = &self.on_error {
            let message = if let Some(s) = payload.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "(panik iletisi yok)".to_string()
            };
            on_error(source, &message);
        }
    }
}
