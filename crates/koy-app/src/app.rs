//! Karşılığı: `koy/src/App/App.xaml.cs` (`Program` ve `App`).
//!
//! Uygulama kabuğu: tema, ilk açılış sihirbazı, çentik ve dock özellikleri, bildirim alanı simgesi, sağ alt menü,
//! genel kısayollar, ekran köşeleri, bağlam kuralları, otomatik güncelleme denetimi ve "1 saat duraklat". Koy penceresi
//! (ayarlar) ilk istendiğinde kurulur. Hepsi arayüz iş parçacığında çalışır; dışarıdan (tek örnek etkinleştirmesi,
//! dock kancaları) gelen istekler `with_app` ile buraya ulaşır.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use koy_core::diagnostics::{CrashError, CrashLog};
use koy_core::input::InputAction;
use koy_core::media::IMediaService;
use koy_core::settings::{AppSettings, FirstRunPolicy, SettingsStore};
use koy_core::shortcuts::ShortcutAction;
use koy_core::tray::TrayHostPolicy;
use koy_core::updates::UpdateFlowPolicy;

use crate::features::Feature;
use crate::features::actions::hot_corner_controller::HotCornerController;
use crate::features::dock::dock_feature::DockFeature;
use crate::features::menu::sag_alt_menu_window::{MenuActions, SagAltMenuWindow};
use crate::features::menu::tray_icon_manager::{TrayActions, TrayIconManager};
use crate::features::notch::notch_feature::NotchFeature;
use crate::features::rules::context_rule_service::ContextRuleService;
use crate::features::settings::koy_window::KoyWindowHost;
use crate::features::settings::profile_service::ProfileService;
use crate::features::settings::settings_view_model::{SettingsHooks, SettingsViewModel};
use crate::features::shortcuts::shortcut_controller::ShortcutController;
use crate::features::wizard::setup_wizard::SetupWizard;
use crate::infra::observable::SharedSettings;
use crate::resources::strings::Strings;
use crate::resources::theme_manager::ThemeManager;
use crate::shared_services::SharedServices;
use crate::update_status::AppInfo;

const PAUSE_DURATION: Duration = Duration::from_secs(60 * 60);

pub struct App {
    services: Rc<SharedServices>,
    notch: RefCell<Option<NotchFeature>>,
    dock: RefCell<Option<Rc<DockFeature>>>,
    tray: RefCell<Option<TrayIconManager>>,
    shortcuts: ShortcutController,
    hot_corners: RefCell<Option<HotCornerController>>,
    koy_window: RefCell<Option<Rc<KoyWindowHost>>>,
    wizard: RefCell<Option<Rc<SetupWizard>>>,
    /// "1 saat duraklat" etkin mi: çentik ve dock kapalı, Windows görev çubuğu geri gelmiş durumda.
    is_paused: Cell<bool>,
    pause_timer: slint::Timer,
    disposed: Cell<bool>,
}

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

/// Çalışan uygulamaya erişim (arayüz iş parçacığında).
pub fn with_app(f: impl FnOnce(&Rc<App>)) {
    let app = APP.with(|a| a.borrow().clone());
    if let Some(app) = app {
        f(&app);
    }
}

fn store() -> SettingsStore {
    SettingsStore::new(SettingsStore::default_path())
}

fn shared(settings: AppSettings) -> SharedSettings {
    Rc::new(RefCell::new(settings))
}

/// Görev çubuğunu geri getirir; hata verse bile çağıranın akışını bozmaz.
pub fn safe_restore_taskbar() {
    koy_platform::shell::taskbar_service::restore(0, false);
}

impl App {
    /// Uygulamayı kurar ve başlatır (olay döngüsü çağıranda döner).
    pub fn start() -> Rc<App> {
        // Görev çubuğu güvenliği: önceki oturum çöktüyse görev çubuğunu geri getir.
        safe_restore_taskbar();
        install_panic_hook();
        let app = Rc::new(App {
            services: Rc::new(SharedServices::default()),
            notch: RefCell::new(None),
            dock: RefCell::new(None),
            tray: RefCell::new(None),
            shortcuts: ShortcutController::default(),
            hot_corners: RefCell::new(None),
            koy_window: RefCell::new(None),
            wizard: RefCell::new(None),
            is_paused: Cell::new(false),
            pause_timer: slint::Timer::default(),
            disposed: Cell::new(false),
        });
        APP.with(|a| *a.borrow_mut() = Some(Rc::clone(&app)));

        let settings = store().load();
        // Tema ve ortak kaynaklar, ilk pencere (sihirbaz dahil) oluşmadan önce yüklenir.
        ThemeManager::apply(&settings);

        // İlk açılış sihirbazı: ayar henüz tamamlanmadıysa sihirbaz başlar; kapanınca (tamamlansa da X ile kapansa da) devam.
        if !settings.general.first_run_completed {
            let done = |first: bool| {
                move || {
                    with_app(|app| {
                        app.wizard.borrow_mut().take();
                        app.continue_startup(first);
                    })
                }
            };
            match SetupWizard::new(store(), done(true), done(true)) {
                Ok(w) => {
                    w.show();
                    *app.wizard.borrow_mut() = Some(w);
                }
                Err(_) => app.continue_startup(true),
            }
        } else {
            app.continue_startup(false);
        }
        app
    }

    fn continue_startup(self: &Rc<Self>, is_first_run: bool) {
        // Sihirbaz X ile kapatıldıysa (tamamlanmadıysa) dock açılıp görev çubuğu gizlenmesin.
        let settings = FirstRunPolicy::apply_safe_defaults_if_incomplete(&store().load());
        Strings::set_ui_culture(&settings.general.language);
        ThemeManager::apply(&settings);

        // Dock kancaları: Koy logosu sağ alt menüyü açar, ayarlar bağlantıları Koy penceresini açar.
        crate::features::dock::hooks::set_open_koy_menu(SagAltMenuWindow::toggle);
        crate::features::dock::hooks::set_open_settings(|page| with_app(|a| a.show_koy_window(Some(page))));
        crate::features::dock::hooks::set_reload_dock(|| with_app(|a| a.reload_dock()));

        SagAltMenuWindow::set_actions(MenuActions {
            notch_toggled: Box::new(|v| with_app(|a| a.update_notch_state(v))),
            dock_toggled: Box::new(|v| with_app(|a| a.update_dock_state(v))),
            pause_toggled: Box::new(|v| with_app(|a| a.set_paused(v))),
            is_paused: Box::new(|| APP.with(|a| a.borrow().as_ref().is_some_and(|a| a.is_paused.get()))),
            open_koy_window: Box::new(|| with_app(|a| a.show_koy_window(None))),
            open_update: Box::new(|| {
                with_app(|a| {
                    a.show_koy_window(None);
                    if let Some(h) = a.koy_window.borrow().as_ref() {
                        h.view_model().open_update_overlay();
                    }
                })
            }),
            exit: Box::new(|| with_app(|a| a.exit())),
        });

        // Sistem tepsisi simgesi: dock kapalıyken de Koy menüsüne erişilmesini sağlar. Dock açılacaksa Koy simgesi
        // dock'un sağ ucunda olacağı için bildirim alanı simgesi eklenmez.
        let tray = TrayIconManager::new(TrayActions {
            toggle_menu: Box::new(SagAltMenuWindow::toggle),
            hide_menu: Box::new(SagAltMenuWindow::hide_if_open),
            show_koy_window: Box::new(|| with_app(|a| a.show_koy_window(None))),
        });
        tray.on_explorer_restarted(|| {
            with_app(|a| {
                if let Some(d) = a.dock.borrow().as_ref() {
                    d.reapply_taskbar_hiding();
                }
            })
        });
        tray.start(!settings.dock.enabled);
        *self.tray.borrow_mut() = Some(tray);

        // Profil uygulandı (bu pencereden ya da bir kuraldan): özellikler ve anahtarlar yeniden kurulur.
        ProfileService::instance().set_reload_running(|saved| {
            let (notch, dock) = (saved.notch.enabled, saved.dock.enabled);
            with_app(|a| {
                a.reload_all_features();
                a.sync_feature_states(Some(notch), Some(dock));
            })
        });

        // Yalnız açık özellikler başlatılır. Dock kapalıyken görev çubuğuna hiç dokunulmaz.
        if settings.notch.enabled {
            self.start_notch(&settings);
        }
        if settings.dock.enabled {
            self.start_dock(&settings);
        }

        // Bağlam kuralları: etkin kural yoksa hiçbir şeye abone olmaz.
        ContextRuleService::instance().start(Rc::clone(&self.services));

        // Güncellemeleri otomatik denetle: açılıştan sonra arka planda tek seferlik denetim.
        crate::auto_updater::run_on_startup(&settings);

        // Genel kısayollar: çentik/dock açık olmasa da çalışır.
        self.shortcuts.on_triggered(|action| with_app(|a| a.on_shortcut_triggered(action)));
        self.shortcuts.start(&settings);

        // Ekran köşeleri: hiçbir köşeye eylem bağlı değilse zamanlayıcı hiç kurulmaz.
        let corners = HotCornerController::new(|action| with_app(|a| a.run_input_action(action)));
        corners.apply(&settings.hot_corners);
        *self.hot_corners.borrow_mut() = Some(corners);

        // Geliştirici: sağ alt menüyü açılıştan sonra koddan açar (canlı doğrulama; C# karşılığı yok).
        if std::env::var("KOY_DEV_OPEN_MENU").is_ok_and(|v| v == "1") {
            slint::Timer::single_shot(Duration::from_secs(3), SagAltMenuWindow::toggle);
        }

        // İlk kurulumdan hemen sonra ana uygulama doğrudan ekrana çıksın; yeni sürümde "Yenilikler".
        if is_first_run {
            self.show_koy_window(None);
        } else if UpdateFlowPolicy::should_show_whats_new(
            AppInfo::version(),
            Some(&settings.general.last_seen_version).filter(|v| !v.is_empty()).map(String::as_str),
            settings.general.first_run_completed,
        ) {
            self.show_koy_window(None);
            if let Some(h) = self.koy_window.borrow().as_ref() {
                h.view_model().open_whats_new_overlay();
            }
        }
    }

    // ------------------------------------------------------------------ özellikler

    fn start_notch(self: &Rc<Self>, settings: &AppSettings) {
        let notch = NotchFeature::new(shared(settings.clone()), Rc::clone(&self.services));
        // Güncelleme kartı "Ayrıntılar": Koy penceresinde güncelleme penceresi açılır.
        notch.set_open_update_handler(|| {
            with_app(|a| {
                a.show_koy_window(None);
                if let Some(h) = a.koy_window.borrow().as_ref() {
                    h.view_model().open_update_overlay();
                }
            })
        });
        notch.start();
        *self.notch.borrow_mut() = Some(notch);
    }

    fn start_dock(self: &Rc<Self>, settings: &AppSettings) {
        let dock = Rc::new(DockFeature::new(shared(settings.clone()), Rc::clone(&self.services)));
        dock.window_count_changed().subscribe(|_| with_app(|a| a.refresh_notification_area_icon()));
        dock.start();
        *self.dock.borrow_mut() = Some(dock);
        self.refresh_notification_area_icon();
    }

    /// Koy'un bildirim alanı simgesi yalnız dock penceresi yokken görünür (dock'ta Koy düğmesi var; iki simge olmasın).
    fn refresh_notification_area_icon(&self) {
        let dock_windows = self.dock.borrow().as_ref().map_or(0, |d| d.window_count());
        if let Some(t) = self.tray.borrow().as_ref() {
            t.set_icon_visible(TrayHostPolicy::should_show_notification_area_icon(dock_windows as i32));
        }
    }

    /// Kısayol ayarı değişti: diskteki ayarlarla yeniden kaydeder.
    pub fn apply_shortcuts(&self) {
        self.shortcuts.apply(&store().load());
    }

    /// Köşe ayarı değişti: diskteki ayarlarla köşe takibini açar, kapatır ya da günceller.
    pub fn apply_hot_corners(&self) {
        if let Some(c) = self.hot_corners.borrow().as_ref() {
            c.apply(&store().load().hot_corners);
        }
    }

    /// Kısayol dışı tetikleyicilerin (ekran köşeleri) ortak eylem dağıtıcısı; karşılığı olmayan eylem yok sayılır.
    fn run_input_action(self: &Rc<Self>, action: InputAction) {
        match action {
            InputAction::OpenNotch => self.on_shortcut_triggered(ShortcutAction::ToggleNotch),
            InputAction::OpenSearch => self.on_shortcut_triggered(ShortcutAction::OpenSearch),
            InputAction::OpenKoyWindow => self.on_shortcut_triggered(ShortcutAction::OpenKoyWindow),
            InputAction::ToggleDock => self.on_shortcut_triggered(ShortcutAction::ToggleDock),
            _ => {}
        }
    }

    fn on_shortcut_triggered(self: &Rc<Self>, action: ShortcutAction) {
        match action {
            ShortcutAction::OpenKoyWindow => self.show_koy_window(None),
            ShortcutAction::ToggleNotch => {
                if let Some(n) = self.notch.borrow().as_ref() {
                    n.toggle_expanded();
                }
            }
            ShortcutAction::OpenShelf => {
                if let Some(n) = self.notch.borrow().as_ref() {
                    n.open_shelf();
                }
            }
            ShortcutAction::ToggleDock => {
                let current = store().load();
                self.update_dock_state(!current.dock.enabled);
            }
            ShortcutAction::OpenMenu => SagAltMenuWindow::toggle(),
            ShortcutAction::PlayPause => self.play_pause(),
            ShortcutAction::OpenSearch => {
                if let Some(n) = self.notch.borrow().as_ref() {
                    n.toggle_search(false);
                }
            }
            ShortcutAction::OpenSearchClipboard => {
                if let Some(n) = self.notch.borrow().as_ref() {
                    n.toggle_search(true);
                }
            }
            ShortcutAction::VoiceCommand => {
                if let Some(n) = self.notch.borrow().as_ref() {
                    n.toggle_voice_command();
                }
            }
        }
    }

    fn play_pause(&self) {
        if let Some(running) = self.services.running_media_service() {
            running.play_pause();
            return;
        }
        // Çentik ve dock kapalı: servis yalnız bu iş için kurulur, sonra bırakılır.
        std::thread::spawn(|| {
            let temporary = koy_platform::media::MediaService::default();
            temporary.start();
            temporary.play_pause();
            temporary.stop();
        });
    }

    pub fn update_notch_state(self: &Rc<Self>, enabled: bool) {
        if self.is_paused.get() {
            return;
        }
        let store = store();
        let mut current = store.load();
        if current.notch.enabled != enabled {
            current.notch.enabled = enabled;
            let _ = store.save(&current);
        }
        self.sync_feature_states(Some(enabled), None);
        let existing = self.notch.borrow().is_some();
        if enabled && !existing {
            self.start_notch(&store.load());
        } else if !enabled && existing {
            if let Some(n) = self.notch.borrow_mut().take() {
                n.stop();
            }
        }
    }

    pub fn update_dock_state(self: &Rc<Self>, enabled: bool) {
        if self.is_paused.get() {
            return;
        }
        let store = store();
        let mut current = store.load();
        if current.dock.enabled != enabled {
            current.dock.enabled = enabled;
            let _ = store.save(&current);
        }
        self.sync_feature_states(None, Some(enabled));
        let existing = self.dock.borrow().is_some();
        if enabled && !existing {
            self.start_dock(&store.load());
        } else if !enabled && existing {
            if let Some(d) = self.dock.borrow_mut().take() {
                d.stop();
            }
            koy_platform::shell::taskbar_service::restore(0, false);
            self.refresh_notification_area_icon();
        }
    }

    /// Dışarıdan değişen çentik / dock açık durumu Koy penceresine ve sağ alt menüye yansır.
    fn sync_feature_states(&self, notch: Option<bool>, dock: Option<bool>) {
        if let Some(h) = self.koy_window.borrow().as_ref() {
            h.view_model().sync_external_state();
        }
        if SagAltMenuWindow::is_created() {
            SagAltMenuWindow::sync_feature_states(notch, dock);
        }
    }

    /// Görev çubuğu elle geri getirildi: çalışan dock Koy'un tepsi penceresini bırakır.
    pub fn notify_taskbar_restored_manually(&self) {
        if let Some(d) = self.dock.borrow().as_ref() {
            d.on_taskbar_restored_manually();
        }
    }

    /// Ayarlar değişince dock'u günceller: çalışan dock yerinde güncellenir; yalnız dock yoksa kurulur.
    pub fn reload_dock(self: &Rc<Self>) {
        if self.is_paused.get() {
            return;
        }
        let settings = store().load();
        let existing = self.dock.borrow().clone();
        if let Some(d) = &existing {
            if settings.dock.enabled {
                d.apply_settings(shared(settings));
                return;
            }
            d.stop();
            *self.dock.borrow_mut() = None;
            self.refresh_notification_area_icon();
        }
        if settings.dock.enabled {
            self.start_dock(&settings);
        }
    }

    /// Çalışan çentiği durdurur, ayarlar açıksa yeni ayarlarla yeniden başlatır.
    pub fn reload_notch(self: &Rc<Self>) {
        if self.is_paused.get() {
            return;
        }
        if let Some(n) = self.notch.borrow_mut().take() {
            n.stop();
        }
        let settings = store().load();
        if settings.notch.enabled {
            self.start_notch(&settings);
        }
    }

    pub fn reload_all_features(self: &Rc<Self>) {
        self.reload_notch();
        self.reload_dock();
    }

    /// Duraklat: çentik ve dock durdurulur (dock görev çubuğunu geri getirir), tepsi simgesi kalır. Bir saat sonra ya da
    /// tekrar basılınca özellikler diskteki ayarlara göre yeniden başlar. Ayar dosyasına dokunulmaz.
    pub fn set_paused(self: &Rc<Self>, paused: bool) {
        if self.is_paused.get() == paused {
            return;
        }
        if paused {
            if let Some(n) = self.notch.borrow_mut().take() {
                n.stop();
            }
            if let Some(d) = self.dock.borrow_mut().take() {
                d.stop();
            }
            safe_restore_taskbar();
            self.is_paused.set(true);
            self.refresh_notification_area_icon();
            self.pause_timer.start(slint::TimerMode::SingleShot, PAUSE_DURATION, || with_app(|a| a.set_paused(false)));
        } else {
            self.is_paused.set(false);
            self.pause_timer.stop();
            self.reload_all_features();
        }
    }

    // ------------------------------------------------------------------ Koy penceresi

    fn settings_view_model(self: &Rc<Self>) -> Option<Rc<SettingsViewModel>> {
        if let Some(h) = self.koy_window.borrow().as_ref() {
            return Some(Rc::clone(h.view_model()));
        }
        let vm = SettingsViewModel::new(store());
        vm.set_hooks(SettingsHooks {
            update_notch_state: Some(Box::new(|v| with_app(|a| a.update_notch_state(v)))),
            update_dock_state: Some(Box::new(|v| with_app(|a| a.update_dock_state(v)))),
            reload_dock: Some(Box::new(|| with_app(|a| a.reload_dock()))),
            reload_notch: Some(Box::new(|| with_app(|a| a.reload_notch()))),
            reload_all_features: Some(Box::new(|| with_app(|a| a.reload_all_features()))),
            notify_taskbar_restored_manually: Some(Box::new(|| with_app(|a| a.notify_taskbar_restored_manually()))),
            exit_app: Some(Box::new(|| with_app(|a| a.exit()))),
            apply_shortcuts: Some(Box::new(|| with_app(|a| a.apply_shortcuts()))),
            apply_hot_corners: Some(Box::new(|| with_app(|a| a.apply_hot_corners()))),
            ..Default::default()
        });
        let host = KoyWindowHost::new(vm).ok()?;
        host.shortcuts().set_controller(Rc::new(self.shortcuts.clone()));
        let vm = Rc::clone(host.view_model());
        *self.koy_window.borrow_mut() = Some(host);
        Some(vm)
    }

    /// Koy penceresini gösterir (ilk istekte kurulur); verilen sayfa seçilir.
    pub fn show_koy_window(self: &Rc<Self>, page: Option<i32>) {
        if self.settings_view_model().is_none() {
            return;
        }
        if let Some(h) = self.koy_window.borrow().as_ref() {
            h.show(page);
        }
    }

    // ------------------------------------------------------------------ çıkış

    /// Koy'dan çık: olay döngüsü biter, `dispose` çağıranda yapılır.
    pub fn exit(&self) {
        let _ = slint::quit_event_loop();
    }

    /// Sıra önemli: özellikler servisleri hâlâ kullanırken servisler atılmamalı.
    pub fn dispose(&self) {
        if self.disposed.replace(true) {
            return;
        }
        self.shortcuts.dispose();
        if let Some(c) = self.hot_corners.borrow_mut().take() {
            c.dispose();
        }
        if let Some(t) = self.tray.borrow_mut().take() {
            t.stop();
        }
        ContextRuleService::instance().stop();
        if let Some(n) = self.notch.borrow_mut().take() {
            n.stop();
        }
        if let Some(d) = self.dock.borrow_mut().take() {
            d.stop();
        }
        if let Some(h) = self.koy_window.borrow_mut().take() {
            h.hide();
        }
        self.services.dispose();
    }
}

/// Önce log, sonra görev çubuğu: geri getirme bir hata verse bile kayıt yazılmış olur.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        CrashLog::write("Panic", &CrashError::object(info.to_string()));
        safe_restore_taskbar();
        // Tepsi hemen asıl sahibine dönsün (arayüz iş parçacığındaysa; değilse atlanır).
        if let Ok(Some(app)) = APP.try_with(|a| a.try_borrow().ok().and_then(|a| a.clone())) {
            app.services.request_tray_host_close();
        }
        previous(info);
    }));
}
