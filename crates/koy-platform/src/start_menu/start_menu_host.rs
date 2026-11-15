//! Başlat sağ tık menüsünün Windows tarafı: araç başlatma, Win+D / Win+S / Win+R ve pil sorgusu. Durumsuzdur, arka
//! plan işi yok.

use koy_core::diagnostics::{CrashError, CrashLog};
use koy_core::start_menu::{IStartMenuHost, ToolLaunchResult};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_D, VK_R, VK_S};
use windows::Win32::UI::Shell::FOLDERID_Profile;

use crate::search::process_start::{ERROR_CANCELLED, ERROR_FILE_NOT_FOUND, ERROR_NO_ASSOCIATION, ERROR_PATH_NOT_FOUND, ShellExecuteRequest, shell_execute};
use crate::search::win_env::known_folder;
use crate::shell::send_win_combo;

const NO_SYSTEM_BATTERY: u8 = 128;

#[derive(Default)]
pub struct StartMenuHost;

impl IStartMenuHost for StartMenuHost {
    fn has_battery(&self) -> bool {
        let mut status = SYSTEM_POWER_STATUS::default();
        // Sorgu başarısızsa pil yok sayılır: Mobilite Merkezi gereksiz yere çıkmasın.
        // SAFETY: yapı bizim.
        unsafe { GetSystemPowerStatus(&mut status) }.is_ok() && (status.BatteryFlag & NO_SYSTEM_BATTERY) == 0
    }

    fn launch(&self, target: &str, arguments: Option<&str>, elevated: bool) -> ToolLaunchResult {
        // Çalışma klasörü Koy'un kurulum klasörü olmasın; Windows'un kendi menüsü gibi kullanıcı klasöründe açılır.
        let home = known_folder(&FOLDERID_Profile);
        let request = ShellExecuteRequest {
            file: target,
            arguments: arguments.filter(|a| !a.is_empty()),
            working_directory: Some(&home),
            verb: elevated.then_some("runas"),
        };
        match shell_execute(request) {
            Ok(()) => ToolLaunchResult::Started,
            // UAC istemi reddedildi.
            Err(ERROR_CANCELLED) => ToolLaunchResult::Declined,
            Err(ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND | ERROR_NO_ASSOCIATION) => ToolLaunchResult::NotFound,
            Err(code) => {
                CrashLog::write(
                    &format!("Başlat menüsü aracı: {target}"),
                    &CrashError::exception("System.ComponentModel.Win32Exception", &format!("Win32 error {code}"), ""),
                );
                ToolLaunchResult::Failed
            }
        }
    }

    // Masaüstünü göster mevcut bir servis değil; Windows'un kendi kısayolu (Win+D) kullanılır.
    fn show_desktop(&self) {
        send_win_combo(VK_D);
    }

    fn open_search(&self) {
        send_win_combo(VK_S);
    }

    fn open_run_dialog(&self) {
        send_win_combo(VK_R);
    }
}
