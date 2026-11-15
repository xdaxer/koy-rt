//! Karşılığı: C#'taki `Process.Start(new ProcessStartInfo { UseShellExecute = true, ... })` (ortak yardımcı, dosya karşılığı yok).
//!
//! .NET kabuk çalıştırmayı (ShellExecuteEx) çağıran iş parçacığı STA değilse ayrı bir STA iş parçacığında yapar;
//! burada da aynı: kısa ömürlü STA iş parçacığı ve `SEE_MASK_FLAG_NO_UI` (hata penceresi yok).

use windows::Win32::Foundation::GetLastError;
use windows::Win32::UI::Shell::{SEE_MASK_FLAG_DDEWAIT, SEE_MASK_FLAG_NO_UI, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::PCWSTR;

use super::com_apartment::ComApartment;

pub(crate) const ERROR_FILE_NOT_FOUND: u32 = 2;
pub(crate) const ERROR_PATH_NOT_FOUND: u32 = 3;
pub(crate) const ERROR_NO_ASSOCIATION: u32 = 1155;
/// UAC istemi reddedildi.
pub(crate) const ERROR_CANCELLED: u32 = 1223;

/// `ProcessStartInfo` (UseShellExecute = true) alanları.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ShellExecuteRequest<'a> {
    pub file: &'a str,
    pub arguments: Option<&'a str>,
    pub working_directory: Option<&'a str>,
    pub verb: Option<&'a str>,
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Başlatır; başarısızlıkta Win32 hata kodu (`Win32Exception.NativeErrorCode`).
pub(crate) fn shell_execute(request: ShellExecuteRequest<'_>) -> Result<(), u32> {
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let _apartment = ComApartment::sta();
                let file = wide(request.file);
                let arguments = request.arguments.filter(|a| !a.is_empty()).map(wide);
                let directory = request.working_directory.filter(|d| !d.is_empty()).map(wide);
                let verb = request.verb.filter(|v| !v.is_empty()).map(wide);

                let mut info = SHELLEXECUTEINFOW {
                    cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                    fMask: SEE_MASK_FLAG_DDEWAIT | SEE_MASK_FLAG_NO_UI,
                    lpFile: PCWSTR(file.as_ptr()),
                    lpParameters: arguments.as_ref().map_or(PCWSTR::null(), |a| PCWSTR(a.as_ptr())),
                    lpDirectory: directory.as_ref().map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr())),
                    lpVerb: verb.as_ref().map_or(PCWSTR::null(), |v| PCWSTR(v.as_ptr())),
                    nShow: SW_SHOWNORMAL.0,
                    ..Default::default()
                };

                // SAFETY: info geçerli ve yukarıdaki tamponlar çağrı boyunca yaşıyor.
                match unsafe { ShellExecuteExW(&mut info) } {
                    Ok(()) => Ok(()),
                    Err(_) => {
                        // SAFETY: parametresiz okuma.
                        let error = unsafe { GetLastError() }.0;
                        if error != 0 {
                            return Err(error);
                        }
                        // .NET'teki gibi: hata kodu yoksa hInstApp'tan türet.
                        Err(match info.hInstApp.0 as usize {
                            2 => ERROR_FILE_NOT_FOUND,
                            3 => ERROR_PATH_NOT_FOUND,
                            31 => ERROR_NO_ASSOCIATION,
                            other => other as u32,
                        })
                    }
                }
            })
            .join()
            .unwrap_or(Err(0))
    })
}
