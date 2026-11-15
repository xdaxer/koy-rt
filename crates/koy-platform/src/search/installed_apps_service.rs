//! Klasik ve Store uygulamalarını Windows'un Uygulamalar klasöründen okur. Kabuk nesneleri apartman ister; iş bitince
//! biten kısa ömürlü bir STA iş parçacığı kullanılır, boşta kalıcı iş parçacığı yoktur.

use koy_core::cancel::{CancellationToken, OperationCanceled};
use koy_core::search::{IInstalledAppsService, InstalledApp, ServiceError};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{BHID_EnumItems, IEnumShellItems, IShellItem, SHCreateItemFromParsingName, SIGDN, SIGDN_NORMALDISPLAY, SIGDN_PARENTRELATIVEPARSING};
use windows::core::w;

use super::com_apartment::ComApartment;

#[derive(Default)]
pub struct InstalledAppsService;

impl InstalledAppsService {
    pub const APPS_FOLDER: &'static str = "shell:AppsFolder";

    /// Uygulamanın simgesi ve başlatma yolu için kabuk adı.
    pub fn shell_path(app: &InstalledApp) -> String {
        format!("{}\\{}", Self::APPS_FOLDER, app.id)
    }

    fn enumerate(ct: &CancellationToken) -> Result<Vec<InstalledApp>, ServiceError> {
        let mut apps = Vec::new();
        // SAFETY: STA bu iş parçacığında kurulu; COM arayüzleri Drop ile bırakılır.
        unsafe {
            let folder: IShellItem = SHCreateItemFromParsingName(w!("shell:AppsFolder"), None)?;
            let Ok(enumerator) = folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) else { return Ok(apps) };
            loop {
                if ct.is_cancellation_requested() {
                    return Err(Box::new(OperationCanceled));
                }
                let mut item: [Option<IShellItem>; 1] = [None];
                let mut fetched = 0u32;
                let _ = enumerator.Next(&mut item, Some(&mut fetched));
                if fetched == 0 {
                    break;
                }
                let Some(item) = item[0].take() else { break };
                let name = display_name(&item, SIGDN_NORMALDISPLAY);
                let id = display_name(&item, SIGDN_PARENTRELATIVEPARSING);
                if let (Some(name), Some(id)) = (name, id)
                    && !name.is_empty()
                    && !id.is_empty()
                {
                    apps.push(InstalledApp::new(id, name));
                }
            }
        }
        Ok(apps)
    }
}

unsafe fn display_name(item: &IShellItem, kind: SIGDN) -> Option<String> {
    // SAFETY: dönen dize CoTaskMemAlloc ile ayrılır, burada serbest bırakılır.
    unsafe {
        let value = item.GetDisplayName(kind).ok()?;
        let text = value.to_string().ok();
        CoTaskMemFree(Some(value.0 as *const _));
        text
    }
}

impl IInstalledAppsService for InstalledAppsService {
    fn load(&self, ct: &CancellationToken) -> Result<Vec<InstalledApp>, ServiceError> {
        let ct = ct.clone();
        std::thread::Builder::new()
            .name("Koy.InstalledApps".into())
            .spawn(move || {
                let _sta = ComApartment::sta();
                Self::enumerate(&ct).map_err(|e| e.to_string())
            })
            .map_err(|e| Box::new(e) as ServiceError)?
            .join()
            .map_err(|_| Box::<dyn std::error::Error + Send + Sync>::from("installed apps thread panicked"))?
            .map_err(|e| {
                if e == OperationCanceled.to_string() { Box::new(OperationCanceled) as ServiceError } else { e.into() }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerates_apps_folder() {
        let apps = InstalledAppsService.load(&CancellationToken::none()).unwrap();
        assert!(!apps.is_empty(), "Uygulamalar klasörü boş olmamalı");
        assert_eq!(InstalledAppsService::shell_path(&apps[0]), format!("shell:AppsFolder\\{}", apps[0].id));
    }
}
