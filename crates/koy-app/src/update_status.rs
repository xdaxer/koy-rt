//! Bu oturumdaki gerçek güncelleme denetimi sonucu (otomatik ve elle denetim aynı yere yazar). Uydurma zaman
//! gösterilmez. Karşılığı: `koy/src/App/UpdateStatus.cs`.
//!
//! Sapma: C# Velopack `UpdateInfo` tutar; burada yalnız hedef sürüm metni tutulur (güncelleme kaynağı App'te).

use std::cell::RefCell;
use std::rc::Rc;

use chrono::{DateTime, Local};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpdateCheckResult {
    #[default]
    None,
    UpToDate,
    /// Yeni sürüm bulundu, kullanıcı onayı bekleniyor.
    UpdateAvailable,
    /// Yeni sürüm indirildi; Koy kapanınca kurulacak.
    UpdateReady,
    Failed,
    NotInstalled,
}

/// Bulunan güncelleme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
}

#[derive(Default)]
struct State {
    last_check: Option<DateTime<Local>>,
    last_result: UpdateCheckResult,
    available: Option<UpdateInfo>,
    changed: Vec<Rc<dyn Fn()>>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub struct UpdateStatus;

impl UpdateStatus {
    pub fn last_check() -> Option<DateTime<Local>> {
        STATE.with(|s| s.borrow().last_check)
    }

    pub fn last_result() -> UpdateCheckResult {
        STATE.with(|s| s.borrow().last_result)
    }

    pub fn available_update() -> Option<UpdateInfo> {
        STATE.with(|s| s.borrow().available.clone())
    }

    pub fn available_version() -> Option<String> {
        Self::available_update().map(|u| u.version)
    }

    pub fn has_available_update() -> bool {
        STATE.with(|s| {
            let s = s.borrow();
            s.available.is_some() && s.last_result == UpdateCheckResult::UpdateAvailable
        })
    }

    /// Arayüz iş parçacığından abone olunur ve çağrılır.
    pub fn on_changed(f: impl Fn() + 'static) {
        STATE.with(|s| s.borrow_mut().changed.push(Rc::new(f)));
    }

    fn raise() {
        let list = STATE.with(|s| s.borrow().changed.clone());
        for f in list {
            f();
        }
    }

    pub fn record(result: UpdateCheckResult) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.last_result = result;
            // Kurulu olmayan sürümde denetim yapılmadığı için zaman kaydedilmez.
            if result != UpdateCheckResult::NotInstalled {
                s.last_check = Some(Local::now());
            }
            if result != UpdateCheckResult::UpdateAvailable {
                s.available = None;
            }
        });
        Self::raise();
    }

    pub fn record_available(update: UpdateInfo) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.available = Some(update);
            s.last_result = UpdateCheckResult::UpdateAvailable;
            s.last_check = Some(Local::now());
        });
        Self::raise();
    }
}

/// Çalışan uygulamanın sürümü (C# `AppInfo.Version`: derleme bilgisi, `+` sonrası atılır).
pub struct AppInfo;

impl AppInfo {
    pub fn version() -> &'static str {
        let v = env!("CARGO_PKG_VERSION");
        v.split('+').next().unwrap_or(v)
    }
}
