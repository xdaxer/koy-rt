//! Karşılığı: `koy/src/App/Features/Wizard/SetupWizardViewModel.cs` ve `SetupWizardWindow.xaml.cs`.
//!
//! İlk açılış sihirbazı. Konum adımından çıkarken (kullanıcı "evet" dediyse) tek seferlik, düşük doğruluklu konum
//! sorgusu yapılır; reddedilse ya da zaman aşımına uğrasa da kurulum devam eder. Bitince ayarlar yazılır ve `completed`
//! çağrılır; pencere kapatılırsa (tamamlanmadan) `cancelled`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use koy_core::cancel::CancellationToken;
use koy_core::settings::{AppLanguage, SettingsStore};
use koy_core::weather::{ILocationService, LocationResult, LocationStatus, WeatherLocationPolicy};
use koy_platform::shell::StartupService;
use koy_platform::weather::LocationService;
use slint::ComponentHandle;

use crate::infra::{dispatch, native_window, ui_ref::UiRef};
use crate::resources::strings::Strings;
use crate::resources::theme_manager::ThemeManager;
use crate::update_status::AppInfo;
use crate::{SetupWizardWindow, Tr};

pub const LAST_STEP: i32 = 4;

pub struct SetupWizard {
    window: SetupWizardWindow,
    store: SettingsStore,
    current_step: Cell<i32>,
    // 0: Çentik ve dock, 1: Yalnız çentik, 2: Yalnız dock
    selected_layout_index: Cell<i32>,
    selected_language: RefCell<String>,
    run_at_startup: Cell<bool>,
    /// Hava durumu için konum kullanılsın mı. Varsayılan hayır: izin yalnız kullanıcı isteyince sorulur.
    use_location: Cell<bool>,
    is_locating: Cell<bool>,
    location_result: RefCell<Option<LocationResult>>,
    completed: RefCell<Option<Box<dyn FnOnce()>>>,
    cancelled: RefCell<Option<Box<dyn FnOnce()>>>,
}

impl SetupWizard {
    /// `on_completed`: ayarlar yazıldı, uygulama özellikleri başlatabilir. `on_cancelled`: pencere tamamlanmadan kapandı.
    pub fn new(
        store: SettingsStore,
        on_completed: impl FnOnce() + 'static,
        on_cancelled: impl FnOnce() + 'static,
    ) -> Result<Rc<Self>, slint::PlatformError> {
        let window = SetupWizardWindow::new()?;
        ThemeManager::install(&window);
        let tr = window.global::<Tr>();
        tr.on_lookup(|k, _| slint::SharedString::from(Strings::get(&k)));
        tr.on_format1(|k, a, _| slint::SharedString::from(Strings::format(&k, &[&a])));
        let language = AppLanguage::system_default();
        let me = Rc::new(SetupWizard {
            window,
            store,
            current_step: Cell::new(1),
            selected_layout_index: Cell::new(0),
            selected_language: RefCell::new(language),
            run_at_startup: Cell::new(true),
            use_location: Cell::new(false),
            is_locating: Cell::new(false),
            location_result: RefCell::new(None),
            completed: RefCell::new(Some(Box::new(on_completed))),
            cancelled: RefCell::new(Some(Box::new(on_cancelled))),
        });
        me.bind();
        me.sync();
        Ok(me)
    }

    pub fn show(&self) {
        let _ = self.window.show();
        let weak = self.window.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(30), move || {
            if let Some(w) = weak.upgrade() {
                let hwnd = native_window::hwnd_of(w.window());
                if hwnd != 0 {
                    native_window::center_on_cursor_monitor(hwnd);
                    native_window::activate(hwnd);
                }
            }
        });
    }

    fn bind(self: &Rc<Self>) {
        let w = &self.window;
        let me = Rc::downgrade(self);
        w.on_next(move || {
            if let Some(m) = me.upgrade() {
                m.next();
            }
        });
        let me = Rc::downgrade(self);
        w.on_back(move || {
            if let Some(m) = me.upgrade() {
                if m.current_step.get() > 1 && !m.is_locating.get() {
                    m.current_step.set(m.current_step.get() - 1);
                    m.sync();
                }
            }
        });
        let me = Rc::downgrade(self);
        w.on_layout_selected(move |i| {
            if let Some(m) = me.upgrade() {
                m.selected_layout_index.set(i);
                m.sync();
            }
        });
        let me = Rc::downgrade(self);
        w.on_language_selected(move |l| {
            if let Some(m) = me.upgrade() {
                *m.selected_language.borrow_mut() = l.to_string();
                m.sync();
            }
        });
        let me = Rc::downgrade(self);
        w.on_startup_toggled(move |v| {
            if let Some(m) = me.upgrade() {
                m.run_at_startup.set(v);
            }
        });
        let me = Rc::downgrade(self);
        w.on_location_selected(move |v| {
            if let Some(m) = me.upgrade() {
                // Seçim değişirse eski sorgu sonucu geçersiz olur (Evet → Hayır → Evet yeniden sorar).
                if m.use_location.get() != v {
                    *m.location_result.borrow_mut() = None;
                }
                m.use_location.set(v);
                m.sync();
            }
        });
        let me = Rc::downgrade(self);
        w.on_close_window(move || {
            if let Some(m) = me.upgrade() {
                m.close(false);
            }
        });
        let me = Rc::downgrade(self);
        w.on_drag_title(move || {
            if let Some(m) = me.upgrade() {
                begin_drag(native_window::hwnd_of(m.window.window()));
            }
        });
        let me = Rc::downgrade(self);
        w.window().on_close_requested(move || {
            if let Some(m) = me.upgrade() {
                m.close(false);
            }
            slint::CloseRequestResponse::KeepWindowShown
        });
    }

    fn sync(&self) {
        let w = &self.window;
        w.set_step(self.current_step.get());
        w.set_layout_index(self.selected_layout_index.get());
        w.set_language(self.selected_language.borrow().as_str().into());
        w.set_run_at_startup(self.run_at_startup.get());
        w.set_use_location(self.use_location.get());
        w.set_locating(self.is_locating.get());
    }

    fn next(self: &Rc<Self>) {
        if self.is_locating.get() {
            return;
        }
        if self.current_step.get() == 3 && self.use_location.get() && self.location_result.borrow().is_none() {
            self.is_locating.set(true);
            self.sync();
            let ui = UiRef::new(self);
            std::thread::spawn(move || {
                let result = LocationService
                    .get_location(&CancellationToken::none())
                    .unwrap_or_else(|_| LocationResult::of(LocationStatus::Error));
                dispatch::post(move || {
                    if let Some(me) = ui.upgrade() {
                        *me.location_result.borrow_mut() = Some(result);
                        me.is_locating.set(false);
                        me.advance();
                    }
                });
            });
            return;
        }
        self.advance();
    }

    fn advance(&self) {
        if self.current_step.get() < LAST_STEP {
            self.current_step.set(self.current_step.get() + 1);
            self.sync();
        } else {
            self.complete();
        }
    }

    fn complete(&self) {
        let mut updated = self.store.load();
        let layout = self.selected_layout_index.get();
        let notch_enabled = layout == 0 || layout == 1;
        let dock_enabled = layout == 0 || layout == 2;
        updated.general.first_run_completed = true;
        updated.general.last_seen_version = AppInfo::version().to_string();
        updated.general.language = self.selected_language.borrow().clone();
        updated.notch.enabled = notch_enabled;
        updated.dock.enabled = dock_enabled;
        updated.dock.hide_windows_taskbar = dock_enabled;
        updated.weather.use_location = self.use_location.get();
        // Konum alındıysa koordinat ayara yazılır; alınamadıysa hava durumu gizli kalır.
        if self.use_location.get() {
            if let Some(result) = self.location_result.borrow().as_ref() {
                updated = WeatherLocationPolicy::apply_device_location(&updated, result, chrono::Utc::now().fixed_offset());
            }
        }
        let _ = self.store.save(&updated);
        StartupService::set_enabled(self.run_at_startup.get());
        self.close(true);
    }

    fn close(&self, completed: bool) {
        let _ = self.window.hide();
        let f = if completed { self.completed.borrow_mut().take() } else { self.cancelled.borrow_mut().take() };
        // İkisi de tek seferlik: hangisi çalışırsa diğeri bırakılır.
        self.completed.borrow_mut().take();
        self.cancelled.borrow_mut().take();
        if let Some(f) = f {
            f();
        }
    }
}

fn begin_drag(hwnd: isize) {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
    use windows::Win32::UI::WindowsAndMessaging::{HTCAPTION, PostMessageW, WM_NCLBUTTONDOWN};
    if hwnd == 0 {
        return;
    }
    // SAFETY: kendi penceremize ileti gönderilir.
    unsafe {
        let _ = ReleaseCapture();
        let _ = PostMessageW(Some(HWND(hwnd as *mut _)), WM_NCLBUTTONDOWN, WPARAM(HTCAPTION as usize), LPARAM(0));
    }
}
