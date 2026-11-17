//! Karşılığı: `koy/src/App/Features/Settings/KoyWindowViewModel.cs`.
//!
//! WPF'te her ayar bir `[ObservableProperty]` idi; değişince `OnPropertyChanged` bütün görünüm alanlarını diskteki
//! ayarın üstüne yazıp kaydediyor, sonra özelliğin adına göre tema / çentik / dock'u yeniden kuruyordu. Burada satır
//! ayarın anahtarını taşır ("Dock.ShowTray"): değer diskteki ayardan metin olarak okunur, yazma `AdvancedSettings::
//! try_set` ile yapılır (aynı biçim ve doğrulama), ardından `SaveSettings`'in sınırlama ve normalleştirmeleri uygulanır.
//! Hangi özelliğin neyi yeniden kurduğu (`AffectsDock`, `AffectsNotch`, tema anahtarları) anahtar üzerinden aynıdır.
//! Görünüm modelinin kendi durumları (başlangıçta çalış, sekme anahtarları, hareket anahtarları) "Ui." ve "Motion."
//! önekli sanal anahtarlardır.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use koy_core::appearance::{CornerSetting, FontScalePolicy};
use koy_core::clock::{ClockFormatPolicy, TimeZoneInfo};
use koy_core::dock::DockProximityMapping;
use koy_core::notch::{HomeCard, HomeCardLayout, LiveBackgroundLimits, LiveBackgroundPresets, NotchEdgeLine, NotchOpenPolicy,
    NotchPanel, NotchPanels, NotchVisualizer};
use koy_core::settings::{
    AdvancedSettingError, AdvancedSettings, AppSettings, ApplyRevertChoice, ApplyRevertDecision, ApplyRevertPolicy,
    FirstRunPolicy, SettingsChanges, SettingsPageReset, SettingsStore, ValueDictionary, ValueList,
};
use koy_core::time::DateTimeOffset;
use koy_core::tray::{TrayIconVisibility, TrayIconVisibilityPolicy};
use koy_platform::shell::StartupService;

use crate::infra::observable::Notifier;
use crate::resources::strings::Strings;
use crate::resources::theme_manager::ThemeManager;

use super::settings_index::{self, IndexRow};

/// Sayfa dizinleri (C# `SelectedPageIndex`): 0 Genel, 1 Çentik, 2 Dock, 3 Ekranlar, 4 Görünüm, 5 Kısayollar,
/// 6 Dikte, 7 Performans, 8 Hakkında, 9 Gelişmiş, 10 Eklentiler.
pub const ADVANCED_PAGE_INDEX: i32 = 9;
pub const EXTENSIONS_PAGE_INDEX: i32 = 10;
const PERFORMANCE_PAGE_INDEX: i32 = 7;

pub const NOTCH_DELAY_OPTIONS: &[i32] = &[0, 150, 300, 500, 800, 1200, 2000];
const LIVE_BACKGROUND_SCRIM_OPTIONS: &[i32] = &[40, 50, 60, 65, 70, 80, 90];
const BASE_FONT_SIZE_OPTIONS: &[i32] = &[10, 11, 12, 13, 14, 15, 16, 17, 18];
const UI_SCALE_OPTIONS: &[i32] = &[80, 90, 100, 110, 120, 130, 140, 150];
const POMODORO_WORK_OPTIONS: &[i32] = &[15, 20, 25, 30, 45, 50, 60];
const POMODORO_SHORT_BREAK_OPTIONS: &[i32] = &[3, 5, 10, 15];
const POMODORO_LONG_BREAK_OPTIONS: &[i32] = &[10, 15, 20, 30];

const MEMORY_TARGET_MB: f64 = 150.0;
const MEMORY_LIMIT_MB: f64 = 200.0;

/// Pencere dışındaki işler (App.xaml.cs karşılıkları). Kurulmamış kanca sessizce atlanır.
#[derive(Default)]
pub struct SettingsHooks {
    pub update_notch_state: Option<Box<dyn Fn(bool)>>,
    pub update_dock_state: Option<Box<dyn Fn(bool)>>,
    pub reload_dock: Option<Box<dyn Fn()>>,
    pub reload_notch: Option<Box<dyn Fn()>>,
    pub reload_all_features: Option<Box<dyn Fn()>>,
    pub notify_taskbar_restored_manually: Option<Box<dyn Fn()>>,
    pub check_updates: Option<Box<dyn Fn()>>,
    pub open_update_overlay: Option<Box<dyn Fn()>>,
    pub exit_app: Option<Box<dyn Fn()>>,
    pub open_store: Option<Box<dyn Fn(&str)>>,
    /// Kısayollar yeniden kaydedilir (App.ApplyShortcuts).
    pub apply_shortcuts: Option<Box<dyn Fn()>>,
    /// Ekran köşeleri takibi yeniden değerlendirilir (App.ApplyHotCorners).
    pub apply_hot_corners: Option<Box<dyn Fn()>>,
}

/// Tepsi uygulaması satırı (`TrayAppVisibilityRow`): açık = dock tepsi penceresinde görünür. Kullanıcı satırı bir kez
/// değiştirince kaydı olur; kaydı olmayan satır yeni simge varsayılanını izler.
#[derive(Clone, Debug)]
pub struct TrayAppRow {
    pub exe_name: String,
    pub is_shown: bool,
    pub is_explicit: bool,
}

#[derive(Clone, Debug)]
pub struct SearchSourceToggle {
    pub id: String,
    pub name: String,
    pub is_enabled: bool,
}

#[derive(Clone, Debug)]
pub struct HomeCardRow {
    pub card: HomeCard,
    pub label: String,
    pub is_enabled: bool,
}

#[derive(Clone, Debug)]
pub struct TabOrderRow {
    pub panel: NotchPanel,
    pub label: String,
}

#[derive(Clone, Debug)]
pub struct SearchResultRow {
    pub title: String,
    pub page: String,
    pub row: usize,
}

#[derive(Default)]
struct UiState {
    selected_page: i32,
    sub_tabs: [i32; 5],
    search_query: String,
    search_results: Vec<SearchResultRow>,
    show_changed_only: bool,
    highlight: String,
    is_page_reset_confirming: bool,
    page_reset_done: bool,
    is_reset_confirming: bool,
    is_uninstall_confirming: bool,
    keep_settings_on_uninstall: bool,
    uninstall_error: String,
    settings_file_invalid: bool,
    clock_custom_date_format_invalid: bool,
    // Uygula / geri al (yalnız görev çubuğunu gizleme).
    revert_hide_taskbar_to: Option<bool>,
    applied_at: Option<DateTimeOffset>,
    apply_revert_seconds_left: i32,
    tray_rows: Vec<TrayAppRow>,
    saved_tray_apps: ValueDictionary<TrayIconVisibility>,
    search_sources: Vec<SearchSourceToggle>,
    home_cards: Vec<HomeCardRow>,
    tab_order: Vec<TabOrderRow>,
    memory_mb: f64,
    memory_level: i32,
}

pub struct SettingsViewModel {
    store: SettingsStore,
    current: RefCell<AppSettings>,
    changed_keys: RefCell<HashSet<String>>,
    st: RefCell<UiState>,
    hooks: RefCell<SettingsHooks>,
    revert_timer: slint::Timer,
    /// Değer okuyan bağlamaların yeniden hesaplanması için artar.
    rev: Cell<i32>,
    /// Dil değişince artar (metinler).
    lang_rev: Cell<i32>,
    pub changed: Notifier,
    /// Kayıt sonrası (C# `SettingsSaved`).
    pub settings_saved: Notifier,
}

fn is_true(text: &str) -> bool {
    text.eq_ignore_ascii_case("true")
}

fn bool_text(b: bool) -> String {
    if b { "true" } else { "false" }.to_string()
}

impl SettingsViewModel {
    pub fn new(store: SettingsStore) -> Rc<Self> {
        let vm = Rc::new(SettingsViewModel {
            store,
            current: RefCell::new(AppSettings::default()),
            changed_keys: RefCell::new(HashSet::new()),
            st: RefCell::new(UiState { keep_settings_on_uninstall: true, ..Default::default() }),
            hooks: RefCell::new(SettingsHooks::default()),
            revert_timer: slint::Timer::default(),
            rev: Cell::new(0),
            lang_rev: Cell::new(0),
            changed: Notifier::default(),
            settings_saved: Notifier::default(),
        });
        vm.load_from_settings();
        vm
    }

    pub fn set_hooks(&self, hooks: SettingsHooks) {
        *self.hooks.borrow_mut() = hooks;
    }

    pub fn store(&self) -> &SettingsStore {
        &self.store
    }

    pub fn settings(&self) -> AppSettings {
        self.current.borrow().clone()
    }

    pub fn rev(&self) -> i32 {
        self.rev.get()
    }

    pub fn lang_rev(&self) -> i32 {
        self.lang_rev.get()
    }

    fn bump(&self) {
        self.rev.set(self.rev.get().wrapping_add(1));
        self.changed.notify();
    }

    fn hook(&self, f: impl FnOnce(&SettingsHooks)) {
        f(&self.hooks.borrow());
    }

    // ------------------------------------------------------------------ yükleme

    /// `LoadFromSettings`: diskteki ayar okunur, görünüm listeleri (tepsi, arama kaynakları, kartlar, sekmeler) kurulur.
    pub fn load_from_settings(&self) {
        let s = self.store.load();
        self.set_current(s);
        self.rebuild_lists();
        self.refresh_memory();
        self.bump();
    }

    fn set_current(&self, s: AppSettings) {
        let changed: HashSet<String> = SettingsChanges::changed_keys(&s, &AppSettings::default()).into_iter().collect();
        *self.changed_keys.borrow_mut() = changed;
        let invalid = !s.notch.clock_custom_date_format.trim().is_empty()
            && !ClockFormatPolicy::is_valid_custom_date_format(Some(&s.notch.clock_custom_date_format));
        self.st.borrow_mut().clock_custom_date_format_invalid = invalid;
        *self.current.borrow_mut() = s;
    }

    fn rebuild_lists(&self) {
        let s = self.current.borrow().clone();
        let mut st = self.st.borrow_mut();

        // Tepsi uygulamaları: görülenler ve kayıtlılar.
        st.saved_tray_apps = s.dock.tray_app_visibility.clone();
        let seen = crate::features::dock::tray_icons::seen_app_names();
        let saved_keys: Vec<String> = st.saved_tray_apps.keys().cloned().collect();
        let apps = TrayIconVisibilityPolicy::settings_apps(seen.iter().map(|s| Some(s.as_str())), saved_keys.iter().map(String::as_str));
        st.tray_rows = apps
            .into_iter()
            .map(|exe| {
                let saved = st.saved_tray_apps.get(&exe).copied();
                let visibility = saved.unwrap_or(s.dock.new_tray_icon_visibility);
                TrayAppRow { exe_name: exe, is_shown: visibility != TrayIconVisibility::Hidden, is_explicit: saved.is_some() }
            })
            .collect();

        st.search_sources = crate::features::search::search_factory::SOURCE_CATALOG
            .iter()
            .map(|(id, name_key)| SearchSourceToggle {
                id: (*id).to_string(),
                name: Strings::get(name_key),
                is_enabled: !s.search.disabled_sources.iter().any(|d| d == id),
            })
            .collect();

        st.home_cards = Self::home_card_rows(&s.notch.home_cards);
        st.tab_order = NotchPanels::normalize_order(Some(&s.notch.panel_order))
            .into_iter()
            .map(|panel| TabOrderRow { panel, label: crate::features::notch::notch_view_model::create_tab(panel, false).label })
            .collect();
    }

    fn home_card_rows(cards: &[HomeCard]) -> Vec<HomeCardRow> {
        let normalized = HomeCardLayout::normalize(Some(cards));
        let mut rows: Vec<HomeCardRow> =
            normalized.iter().map(|c| HomeCardRow { card: *c, label: home_card_label(*c), is_enabled: true }).collect();
        for card in [HomeCard::Music, HomeCard::HeadphoneBattery, HomeCard::SystemNetwork, HomeCard::QuickLaunch, HomeCard::Weather, HomeCard::Timers] {
            if !normalized.contains(&card) {
                rows.push(HomeCardRow { card, label: home_card_label(card), is_enabled: false });
            }
        }
        rows
    }

    /// Dışarıdan (sağ alt menü, kısayol) değişen çentik / dock açık durumu: ayar yeniden okunur, kaydedilmez.
    pub fn sync_external_state(&self) {
        self.load_from_settings();
    }

    /// `ReloadFromDiskIfChanged`: settings.json dışarıdan değişti. Koy'un kendi kaydı yok sayılır; dosya okunamıyorsa
    /// yüklenmez, kısa uyarı gösterilir.
    pub fn reload_from_disk_if_changed(&self) {
        if self.store.is_own_write() {
            return;
        }
        let Some(disk) = self.store.try_read_strict() else {
            self.st.borrow_mut().settings_file_invalid = true;
            self.bump();
            return;
        };
        self.st.borrow_mut().settings_file_invalid = false;
        if disk == *self.current.borrow() {
            self.bump();
            return;
        }
        self.apply_loaded_settings();
    }

    /// Gelişmiş tablodan yazılan ayar: kaydedilir, pencere ve özellikler yeni değerle kurulur (alan her şey olabilir).
    pub fn commit_loaded(&self, updated: AppSettings) {
        if super::app_uninstaller::in_progress() {
            return;
        }
        let _ = self.store.save(&updated);
        self.apply_loaded_settings();
    }

    /// Ayar dışarıdan (profil, eklenti, gelişmiş tablo, sıfırlama) değişti: yeniden okunur, tema ve özellikler kurulur.
    fn apply_loaded_settings(&self) {
        self.load_from_settings();
        self.settings_saved.notify();
        ThemeManager::apply(&self.current.borrow());
        self.hook(|h| {
            if let Some(f) = &h.reload_all_features {
                f();
            }
        });
    }

    // ------------------------------------------------------------------ okuma

    /// Satırın gösterdiği değer (bool: true/false, enum: ad, sayı: değişmez kültür).
    pub fn value(&self, key: &str) -> String {
        let s = self.current.borrow();
        if key == "Ui.RunAtStartup" {
            return bool_text(StartupService::is_enabled());
        }
        if let Some(panel) = key.strip_prefix("Ui.Panel") {
            let panels = NotchPanels::parse_enabled(Some(&s.notch.enabled_panels), true);
            return bool_text(panel_of(panel).is_some_and(|p| panels.contains(&p)));
        }
        if let Some(name) = key.strip_prefix("Motion.") {
            let m = &s.appearance.motion;
            return bool_text(match name {
                "NotchOpenClose" => m.notch_open_close,
                "TabTransitions" => m.tab_transitions,
                "Osd" => m.osd,
                "DockMagnify" => m.dock_magnify,
                "FolderPanel" => m.folder_panel,
                _ => false,
            });
        }
        let Some((section, field)) = key.split_once('.') else { return String::new() };
        let defaults = AppSettings::default();
        let views = s.section_views(&defaults);
        let raw = views
            .iter()
            .find(|v| v.name == section)
            .and_then(|v| v.fields.iter().find(|f| f.name == field))
            .map(|f| f.value.clone())
            .unwrap_or_default();
        // Görünümün yükleme düzeltmeleri (LoadFromSettings): tam genişlikte geçerli yakınlık, köşe seçenek değeri,
        // gecikmenin en yakın seçeneği, canlı arka plan önayarı.
        match key {
            "Dock.Proximity" => format!("{:?}", DockProximityMapping::effective(s.dock.style, s.dock.proximity)),
            "Appearance.CornerRadius" => format_number(CornerSetting::to_stored(CornerSetting::resolve(s.appearance.corner_radius))),
            "Notch.OpenDelayMs" => nearest_delay(s.notch.open_delay_ms).to_string(),
            "Notch.CloseDelayMs" => nearest_delay(s.notch.close_delay_ms).to_string(),
            "Notch.LiveBackgroundPreset" => LiveBackgroundPresets::find(Some(&s.notch.live_background_preset)).id.to_string(),
            "Dock.NewTrayIconVisibility" => {
                if s.dock.new_tray_icon_visibility == TrayIconVisibility::Hidden { "Hidden" } else { "Overflow" }.to_string()
            }
            "Dock.IconSize" if s.dock.icon_size <= 0.0 => "28".to_string(),
            "HotCorners.DwellMs" => {
                [200, 400, 700, 1000].iter().min_by_key(|o| (*o - s.hot_corners.dwell_ms).abs()).copied().unwrap_or(400).to_string()
            }
            _ => raw,
        }
    }

    /// Satır varsayılandan farklı mı ("yalnız değişenler" ve değişmiş noktası).
    pub fn is_changed(&self, key: &str) -> bool {
        let real = if key.starts_with("Ui.Panel") {
            "Notch.EnabledPanels"
        } else if key.starts_with("Motion.") {
            "Appearance.Motion"
        } else if key.starts_with("Ui.") {
            return false;
        } else {
            key
        };
        self.changed_keys.borrow().contains(real)
    }

    /// Seçim satırında değerin dizini; metin büyük/küçük harf duyarsız, sayılar sayı olarak karşılaştırılır.
    pub fn choice_index(&self, key: &str, values: &[String]) -> i32 {
        let current = self.value(key);
        let num = current.parse::<f64>().ok();
        values
            .iter()
            .position(|v| v.eq_ignore_ascii_case(&current) || (num.is_some() && v.parse::<f64>().ok() == num))
            .map_or(-1, |i| i as i32)
    }

    /// Görünüm modelinden gelen seçenek listeleri. "Ad#labels:ek" görünen metinleri ister.
    pub fn list(&self, name: &str) -> Vec<String> {
        let (base, labels) = match name.split_once("#labels:") {
            Some((b, suffix)) => (b, Some(suffix)),
            None => (name, None),
        };
        let numbers = |opts: &[i32]| -> Vec<String> {
            opts.iter()
                .map(|v| match labels {
                    Some(sfx) if !sfx.is_empty() => {
                        if sfx == "%" { format!("{v} %") } else { format!("{v} {sfx}") }
                    }
                    _ => v.to_string(),
                })
                .collect()
        };
        match base {
            "NotchDelayOptions" => numbers(NOTCH_DELAY_OPTIONS),
            "LiveBackgroundScrimOptions" => numbers(LIVE_BACKGROUND_SCRIM_OPTIONS),
            "BaseFontSizeOptions" => numbers(BASE_FONT_SIZE_OPTIONS),
            "UiScaleOptions" => numbers(UI_SCALE_OPTIONS),
            "PomodoroWorkOptions" => numbers(POMODORO_WORK_OPTIONS),
            "PomodoroShortBreakOptions" => numbers(POMODORO_SHORT_BREAK_OPTIONS),
            "PomodoroLongBreakOptions" => numbers(POMODORO_LONG_BREAK_OPTIONS),
            "FontOptions" => {
                // Önce "Varsayılan" (boş değer), sonra yüklü yazı tipleri.
                let mut out = vec![if labels.is_some() { Strings::get("FontDefault") } else { String::new() }];
                out.extend(ThemeManager::installed_fonts());
                out
            }
            "SecondTimeZoneOptions" => {
                let mut out = vec![if labels.is_some() { Strings::get("ClockSecondTimeZoneNone") } else { String::new() }];
                out.extend(
                    TimeZoneInfo::system_time_zones()
                        .into_iter()
                        .map(|(id, display)| if labels.is_some() { display.to_string() } else { id.to_string() }),
                );
                out
            }
            "LiveBackgroundPresetOptions" => LiveBackgroundPresets::all()
                .iter()
                .map(|p| if labels.is_some() { Strings::get(&p.name_key) } else { p.id.to_string() })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Satır dışı koşullar ("Ad=Değer").
    pub fn flag(&self, spec: &str) -> bool {
        let (name, want) = spec.split_once('=').unwrap_or((spec, "True"));
        let want = !want.eq_ignore_ascii_case("False");
        let s = self.current.borrow();
        let st = self.st.borrow();
        let v = match name {
            "HasTrayAppRows" => !st.tray_rows.is_empty(),
            "MotionTogglesActive" => motion_toggles_active(&s),
            "MotionTogglesInactive" => !motion_toggles_active(&s),
            "IsLiveBackgroundEnabled" => s.notch.live_background_target != koy_core::notch::LiveBackgroundTarget::None,
            "IsLiveBackgroundFileSource" => s.notch.live_background_source == koy_core::notch::LiveBackgroundSourceKind::LocalFile,
            "IsLiveBackgroundAmbientSource" => {
                s.notch.live_background_source == koy_core::notch::LiveBackgroundSourceKind::BuiltInAmbient
            }
            "ClockCustomDateFormatInvalid" => st.clock_custom_date_format_invalid,
            "NotificationsPermissionMissing" => {
                s.notch.show_notifications
                    && matches!(
                        koy_platform::notifications::WinRtNotificationListener::get_access_status(),
                        koy_core::notifications::NotificationAccessStatus::Denied
                            | koy_core::notifications::NotificationAccessStatus::Unavailable
                    )
            }
            "IsApplyRevertPending" => st.revert_hide_taskbar_to.is_some(),
            "SettingsFileInvalid" => st.settings_file_invalid,
            "IsResetConfirming" => st.is_reset_confirming,
            "IsPageResetConfirming" => st.is_page_reset_confirming,
            "PageResetDone" => st.page_reset_done,
            "CanResetPage" => !reset_sections_for(st.selected_page).is_empty(),
            "IsUninstallConfirming" => st.is_uninstall_confirming,
            "KeepSettingsOnUninstall" => st.keep_settings_on_uninstall,
            "CanUninstall" => can_uninstall(),
            "HasAvailableUpdate" => crate::update_status::UpdateStatus::has_available_update(),
            "DeveloperMode" => s.general.developer_mode,
            "IsSettingsSearching" => !st.search_query.trim().is_empty(),
            "SettingsSearchNoResult" => !st.search_query.trim().is_empty() && st.search_results.is_empty(),
            "IsDockFullWidth" => s.dock.style == koy_core::dock::DockStyleMode::FullWidth,
            _ => false,
        };
        v == want
    }

    /// Satır dışı metinler (son denetim, bellek, kaldırma açıklaması...).
    pub fn text(&self, name: &str) -> String {
        let st = self.st.borrow();
        match name {
            "LastCheckText" => last_check_text(),
            "CheckButtonText" => Strings::get("CheckNow"),
            "AvailableVersionText" => crate::update_status::UpdateStatus::available_version()
                .map(|v| Strings::format("UpdateAvailableBadge", &[&v]))
                .unwrap_or_default(),
            "MemoryUsageText" => {
                if st.memory_mb > 0.0 {
                    format!("{:.1} MB / {:.0} MB", st.memory_mb, MEMORY_TARGET_MB)
                } else {
                    "–".to_string()
                }
            }
            "UninstallRowDescription" => Strings::get(if can_uninstall() { "UninstallRowDesc" } else { "UninstallOnlyInstalled" }),
            "UninstallError" => st.uninstall_error.clone(),
            "ApplyRevertSecondsLeft" => format!("({})", st.apply_revert_seconds_left),
            "LiveBackgroundFileName" => {
                let p = &self.current.borrow().notch.live_background_file_path;
                if p.trim().is_empty() {
                    Strings::get("LiveBgFileNone")
                } else {
                    std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone())
                }
            }
            "StartIconFileName" => {
                let p = &self.current.borrow().dock.start_icon_path;
                if p.trim().is_empty() {
                    Strings::get("DockStartIconNone")
                } else {
                    std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone())
                }
            }
            "IconPackFolderName" => {
                let p = &self.current.borrow().appearance.icon_pack_folder;
                if p.trim().is_empty() { Strings::get("IconPackNoFolder") } else { p.clone() }
            }
            "AppVersion" => crate::update_status::AppInfo::version().to_string(),
            "AboutStatusText" => about_status_text(),
            "UserStyleStatusText" => user_style_status_text(&self.current.borrow()),
            _ => String::new(),
        }
    }

    pub fn memory(&self) -> (f64, i32) {
        let st = self.st.borrow();
        let pct = (st.memory_mb / MEMORY_TARGET_MB * 100.0).clamp(0.0, 100.0);
        (pct, st.memory_level)
    }

    // ------------------------------------------------------------------ yazma

    /// Satırdan gelen yeni değer. Diskteki ayarın üstüne yazılır (görünüm dışı alanlar korunur), sınırlar uygulanır,
    /// kaydedilir; ardından anahtara göre tema, çentik ve dock yeniden kurulur.
    pub fn set(&self, key: &str, text: &str) {
        // Kaldırma başladıysa ayar dosyası yeniden oluşmasın.
        if super::app_uninstaller::in_progress() {
            return;
        }
        if key == "Ui.RunAtStartup" {
            StartupService::set_enabled(is_true(text));
            self.bump();
            return;
        }

        let current = self.store.load();
        let previous_hide_taskbar = current.dock.hide_windows_taskbar;
        let mut updated = current.clone();
        if let Some(panel) = key.strip_prefix("Ui.Panel") {
            let Some(panel) = panel_of(panel) else { return };
            let mut enabled = NotchPanels::parse_enabled(Some(&current.notch.enabled_panels), true);
            enabled.retain(|p| *p != panel);
            if is_true(text) {
                enabled.push(panel);
            }
            updated.notch.enabled_panels = enabled_panel_keys(&enabled);
        } else if let Some(name) = key.strip_prefix("Motion.") {
            let m = &mut updated.appearance.motion;
            let v = is_true(text);
            match name {
                "NotchOpenClose" => m.notch_open_close = v,
                "TabTransitions" => m.tab_transitions = v,
                "Osd" => m.osd = v,
                "DockMagnify" => m.dock_magnify = v,
                "FolderPanel" => m.folder_panel = v,
                _ => return,
            }
        } else if key == "Notch.ClockCustomDateFormat" {
            // Geçersiz özel biçim kaydedilmez (önceki korunur); boş metin yerleşik biçime döner.
            let valid = text.trim().is_empty() || ClockFormatPolicy::is_valid_custom_date_format(Some(text));
            self.st.borrow_mut().clock_custom_date_format_invalid = !valid;
            if !valid {
                self.bump();
                return;
            }
            updated.notch.clock_custom_date_format = if text.trim().is_empty() { String::new() } else { text.to_string() };
        } else {
            let (error, result) = AdvancedSettings::try_set(&current, key, Some(text));
            if error != AdvancedSettingError::None {
                // Değer geri döner (satır eski değeri gösterir).
                self.bump();
                return;
            }
            updated = result;
        }

        // OnDockStyleChanged: tam genişlikte sunulmayan yakınlık geçerli karşılığına çekilir.
        if key == "Dock.Style" {
            updated.dock.proximity = DockProximityMapping::effective(updated.dock.style, updated.dock.proximity);
        }
        // OnDockNewTrayIconVisibilityChanged: kaydı olmayan satırlar yeni varsayılanı izler (kayda girmez).
        if key == "Dock.NewTrayIconVisibility" {
            let shown = updated.dock.new_tray_icon_visibility != TrayIconVisibility::Hidden;
            for row in self.st.borrow_mut().tray_rows.iter_mut().filter(|r| !r.is_explicit) {
                row.is_shown = shown;
            }
        }
        normalize(&mut updated);
        self.commit(key, updated);

        if key == "Dock.HideWindowsTaskbar" {
            self.on_hide_windows_taskbar_changed(previous_hide_taskbar, self.current.borrow().dock.hide_windows_taskbar);
        }
    }

    /// Sayfanın kendi akışıyla (kısayollar, profiller) kurulmuş ayar: kaydedilir ve anahtara göre yönlendirilir.
    pub fn commit_external(&self, key: &str, updated: AppSettings) {
        if super::app_uninstaller::in_progress() {
            return;
        }
        self.commit(key, updated);
    }

    /// `SaveSettings` + `OnPropertyChanged` yönlendirmesi.
    fn commit(&self, key: &str, updated: AppSettings) {
        let _ = self.store.save(&updated);
        self.set_current(updated.clone());
        self.bump();
        self.settings_saved.notify();

        if key == "General.Language" {
            self.on_language_changed(&updated.general.language);
        }
        if affects_theme(key) {
            // Animasyon anahtarları da buradan (Motion::refresh) canlı uygulanır; çentik ve dock yeniden kurulmaz.
            ThemeManager::apply(&updated);
        }
        self.hook(|h| {
            if key.starts_with("Shortcuts") {
                if let Some(f) = &h.apply_shortcuts {
                    f();
                }
                return;
            }
            if key.starts_with("HotCorners.") {
                if let Some(f) = &h.apply_hot_corners {
                    f();
                }
                return;
            }
            if key == "Notch.Enabled" {
                if let Some(f) = &h.update_notch_state {
                    f(updated.notch.enabled);
                }
            } else if key == "Dock.Enabled" {
                if let Some(f) = &h.update_dock_state {
                    f(updated.dock.enabled);
                }
            } else {
                // Özellikler ayarı kurulurken okur; seçenek değişince ilgili özellik yeni ayarla yeniden kurulur.
                if affects_dock(key) {
                    if let Some(f) = &h.reload_dock {
                        f();
                    }
                }
                if affects_notch(key) {
                    if let Some(f) = &h.reload_notch {
                        f();
                    }
                }
            }
        });
    }

    fn on_language_changed(&self, language: &str) {
        Strings::set_ui_culture(language);
        self.lang_rev.set(self.lang_rev.get().wrapping_add(1));
        // Dile bağlı listeler (arama kaynakları, kartlar, sekmeler) yeniden kurulur.
        self.rebuild_lists();
        self.st.borrow_mut().search_results.clear();
        let q = self.st.borrow().search_query.clone();
        self.set_search_query(&q);
        self.bump();
    }

    // ------------------------------------------------------------------ listeler

    pub fn tray_rows(&self) -> Vec<TrayAppRow> {
        self.st.borrow().tray_rows.clone()
    }

    /// Tepsi satırı değişti: satır kayda girer, dock yeniden kurulur (C# `DockTrayAppsVersion`).
    pub fn set_tray_row(&self, index: usize, shown: bool) {
        {
            let mut st = self.st.borrow_mut();
            let Some(row) = st.tray_rows.get_mut(index) else { return };
            row.is_shown = shown;
            row.is_explicit = true;
        }
        let mut updated = self.store.load();
        updated.dock.tray_app_visibility = self.tray_apps_for_save();
        self.commit("Dock.TrayAppVisibility", updated);
    }

    fn tray_apps_for_save(&self) -> ValueDictionary<TrayIconVisibility> {
        let st = self.st.borrow();
        let mut result = ValueDictionary::new();
        for row in st.tray_rows.iter().filter(|r| r.is_explicit) {
            // Kayıtlı "Shown" korunur: bugün tepsi penceresinde görünür.
            let v = if !row.is_shown {
                TrayIconVisibility::Hidden
            } else if st.saved_tray_apps.get(&row.exe_name) == Some(&TrayIconVisibility::Shown) {
                TrayIconVisibility::Shown
            } else {
                TrayIconVisibility::Overflow
            };
            result.insert(&row.exe_name, v);
        }
        result
    }

    pub fn search_sources(&self) -> Vec<SearchSourceToggle> {
        self.st.borrow().search_sources.clone()
    }

    pub fn set_search_source(&self, index: usize, enabled: bool) {
        let disabled: Vec<String> = {
            let mut st = self.st.borrow_mut();
            let Some(row) = st.search_sources.get_mut(index) else { return };
            row.is_enabled = enabled;
            st.search_sources.iter().filter(|s| !s.is_enabled).map(|s| s.id.clone()).collect()
        };
        let mut updated = self.store.load();
        updated.search.disabled_sources = ValueList::from_items(disabled);
        self.commit("Search.DisabledSources", updated);
    }

    pub fn home_cards(&self) -> Vec<HomeCardRow> {
        self.st.borrow().home_cards.clone()
    }

    pub fn set_home_card_enabled(&self, index: usize, enabled: bool) {
        {
            let mut st = self.st.borrow_mut();
            let Some(row) = st.home_cards.get_mut(index) else { return };
            row.is_enabled = enabled;
        }
        self.save_home_cards();
    }

    pub fn move_home_card(&self, index: usize, step: i32) {
        {
            let mut st = self.st.borrow_mut();
            let target = index as i32 + step;
            if target < 0 || target >= st.home_cards.len() as i32 || index >= st.home_cards.len() {
                return;
            }
            let item = st.home_cards.remove(index);
            st.home_cards.insert(target as usize, item);
        }
        self.save_home_cards();
    }

    fn save_home_cards(&self) {
        let cards: Vec<HomeCard> = self.st.borrow().home_cards.iter().filter(|r| r.is_enabled).map(|r| r.card).collect();
        let mut updated = self.store.load();
        updated.notch.home_cards = ValueList::from_items(cards);
        self.commit("Notch.HomeCards", updated);
    }

    pub fn tab_order(&self) -> Vec<TabOrderRow> {
        self.st.borrow().tab_order.clone()
    }

    pub fn move_tab(&self, index: usize, step: i32) {
        let Some(panel) = self.st.borrow().tab_order.get(index).map(|r| r.panel) else { return };
        let current = self.store.load();
        let order: Vec<String> = self.st.borrow().tab_order.iter().map(|r| NotchPanels::to_key(r.panel).to_string()).collect();
        let moved = NotchPanels::r#move(Some(&order), panel, step);
        if moved == order {
            return;
        }
        let mut updated = current;
        // Varsayılan sıra boş liste olarak kalır ("yalnız değişenler" onu değişmiş saymasın).
        let default_order: Vec<String> = NotchPanels::order().iter().map(|p| NotchPanels::to_key(*p).to_string()).collect();
        updated.notch.panel_order = if moved == default_order { ValueList::new() } else { ValueList::from_items(moved.clone()) };
        self.st.borrow_mut().tab_order = NotchPanels::normalize_order(Some(&moved))
            .into_iter()
            .map(|panel| TabOrderRow { panel, label: crate::features::notch::notch_view_model::create_tab(panel, false).label })
            .collect();
        self.commit("Notch.PanelOrder", updated);
    }

    // ------------------------------------------------------------------ uygula / geri al

    // Riskli ayar (görev çubuğunu gizleme): değişiklik hemen uygulanır, kullanıcı süre içinde onaylamazsa önceki değere
    // dönülür. Karar Core'da (ApplyRevertPolicy); zamanlayıcı yalnız bekleyen değişiklik varken çalışır.
    fn on_hide_windows_taskbar_changed(self: &Self, old_value: bool, new_value: bool) {
        let pending_to = {
            let mut st = self.st.borrow_mut();
            // Bekleyen bir değişiklik varken yeniden değişirse ilk (bilinen iyi) değer korunur.
            let to = *st.revert_hide_taskbar_to.get_or_insert(old_value);
            to
        };
        if pending_to == new_value {
            self.stop_apply_revert();
            return;
        }
        {
            let mut st = self.st.borrow_mut();
            st.applied_at = Some(chrono::Utc::now().fixed_offset());
            st.apply_revert_seconds_left = ApplyRevertPolicy::DEFAULT_TIMEOUT.total_seconds().ceil() as i32;
        }
        self.bump();
    }

    /// Pencere sahibi saniyede bir çağırır (zamanlayıcı pencere sahibinde; bekleyen değişiklik yokken durur).
    pub fn apply_revert_tick(&self) {
        self.decide_apply_revert(ApplyRevertChoice::None);
    }

    pub fn is_apply_revert_pending(&self) -> bool {
        self.st.borrow().revert_hide_taskbar_to.is_some()
    }

    pub fn decide_apply_revert(&self, choice: ApplyRevertChoice) {
        let (previous, applied_at) = {
            let st = self.st.borrow();
            (st.revert_hide_taskbar_to, st.applied_at)
        };
        let (Some(previous), Some(applied_at)) = (previous, applied_at) else {
            self.stop_apply_revert();
            return;
        };
        let now: DateTimeOffset = chrono::Utc::now().fixed_offset();
        match ApplyRevertPolicy::decide(applied_at, now, choice, ApplyRevertPolicy::DEFAULT_TIMEOUT) {
            ApplyRevertDecision::Wait => {
                self.st.borrow_mut().apply_revert_seconds_left =
                    ApplyRevertPolicy::remaining(applied_at, now, ApplyRevertPolicy::DEFAULT_TIMEOUT).total_seconds().ceil() as i32;
                self.bump();
            }
            ApplyRevertDecision::Keep => self.stop_apply_revert(),
            ApplyRevertDecision::Revert => {
                self.stop_apply_revert();
                // Normal kaydetme ve dock yeniden kurma yolundan geçer; geri sayım yeniden başlamaz.
                let mut updated = self.store.load();
                updated.dock.hide_windows_taskbar = previous;
                self.commit("Dock.HideWindowsTaskbar", updated);
            }
        }
    }

    fn stop_apply_revert(&self) {
        {
            let mut st = self.st.borrow_mut();
            st.revert_hide_taskbar_to = None;
            st.applied_at = None;
        }
        self.revert_timer.stop();
        self.bump();
    }

    // ------------------------------------------------------------------ sayfalar, arama, sıfırlama

    pub fn selected_page(&self) -> i32 {
        self.st.borrow().selected_page
    }

    pub fn sub_tab(&self, page: i32) -> i32 {
        sub_tab_slot(page).map_or(0, |i| self.st.borrow().sub_tabs[i])
    }

    /// `SelectPage`: Performans açılınca bellek yenilenir; sayfa değişince sıfırlama onayı kapanır.
    pub fn select_page(&self, index: i32) {
        {
            let mut st = self.st.borrow_mut();
            st.selected_page = index;
            st.is_page_reset_confirming = false;
            st.page_reset_done = false;
        }
        if index == PERFORMANCE_PAGE_INDEX {
            self.refresh_memory();
        }
        self.bump();
    }

    pub fn select_sub_tab(&self, page: i32, sub: i32) {
        if let Some(i) = sub_tab_slot(page) {
            self.st.borrow_mut().sub_tabs[i] = sub;
            self.bump();
        }
    }

    pub fn search_query(&self) -> String {
        self.st.borrow().search_query.clone()
    }

    pub fn search_results(&self) -> Vec<SearchResultRow> {
        self.st.borrow().search_results.clone()
    }

    /// Ayar araması: satır dizini sayfaların kendisinden (dönüştürücüyle) çıkarıldı; puanlama Core'da.
    pub fn set_search_query(&self, query: &str) {
        let results = if query.trim().is_empty() {
            Vec::new()
        } else {
            settings_index::search(query)
                .into_iter()
                .map(|(row, entry)| SearchResultRow { title: entry.title.clone(), page: entry.page.clone(), row })
                .collect()
        };
        {
            let mut st = self.st.borrow_mut();
            st.search_query = query.to_string();
            st.search_results = results;
        }
        self.bump();
    }

    /// Sonuca gidilir: sayfa ve alt sekme seçilir, satır vurgulanır; arama kapanır.
    pub fn open_search_result(&self, index: usize) {
        let Some(result) = self.st.borrow().search_results.get(index).cloned() else { return };
        let row: &IndexRow = &settings_index::ROWS[result.row];
        self.select_page(row.page);
        self.select_sub_tab(row.page, row.sub_tab);
        {
            let mut st = self.st.borrow_mut();
            st.highlight = Strings::get(row.title);
            st.search_query.clear();
            st.search_results.clear();
        }
        self.bump();
    }

    pub fn highlight(&self) -> String {
        self.st.borrow().highlight.clone()
    }

    pub fn clear_highlight(&self) {
        self.st.borrow_mut().highlight.clear();
        self.bump();
    }

    pub fn show_changed_only(&self) -> bool {
        self.st.borrow().show_changed_only
    }

    pub fn set_show_changed_only(&self, on: bool) {
        self.st.borrow_mut().show_changed_only = on;
        self.bump();
    }

    fn confirm_page_reset(&self) {
        self.st.borrow_mut().is_page_reset_confirming = false;
        let sections = reset_sections_for(self.selected_page());
        if sections.is_empty() {
            return;
        }
        let _ = self.store.save(&SettingsPageReset::reset(&self.store.load(), sections));
        self.apply_loaded_settings();
        self.st.borrow_mut().page_reset_done = true;
        self.bump();
    }

    fn reset_settings(&self) {
        self.st.borrow_mut().is_reset_confirming = false;
        // Kurulum sihirbazı tamamlandı bilgisi korunur; yoksa sıfırlama sihirbazı yeniden açardı.
        let _ = self.store.save(&FirstRunPolicy::create_reset(&self.store.load()));
        self.apply_loaded_settings();
    }

    /// Bellek göstergesi: özel bayt, hedef 150 MB, sınır 200 MB.
    pub fn refresh_memory(&self) {
        // Görev Yöneticisi'yle aynı sayı (özel çalışma kümesi); ölçülemediyse uydurma değer gösterilmez.
        let bytes = koy_platform::diagnostics::ProcessMemory::get_private_working_set_bytes();
        let mb = if bytes > 0 { (bytes as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0 } else { 0.0 };
        let level = if mb > MEMORY_LIMIT_MB {
            2
        } else if mb > MEMORY_TARGET_MB {
            1
        } else {
            0
        };
        let mut st = self.st.borrow_mut();
        st.memory_mb = mb;
        st.memory_level = level;
    }

    // ------------------------------------------------------------------ komutlar

    /// Düğme eylemleri (C# RelayCommand adlarıyla).
    pub fn action(&self, name: &str) {
        match name {
            "CheckUpdatesCommand" => self.hook(|h| {
                if let Some(f) = &h.check_updates {
                    f();
                }
            }),
            "OpenUpdateOverlayCommand" => self.hook(|h| {
                if let Some(f) = &h.open_update_overlay {
                    f();
                }
            }),
            "ExitAppCommand" => self.hook(|h| {
                if let Some(f) = &h.exit_app {
                    f();
                }
            }),
            "PickLiveBackgroundFileCommand" => self.pick_live_background_file(),
            "PickStartIconCommand" => self.pick_start_icon(),
            "PickIconPackFolderCommand" => self.pick_icon_pack_folder(),
            "ClearSearchHistoryCommand" => crate::features::search::search_factory::clear_history(),
            "OpenUserStyleFolderCommand" => {
                let folder = koy_core::appearance::UserStyleFile::default_folder();
                let _ = std::fs::create_dir_all(&folder);
                open_external(&folder.to_string_lossy());
            }
            "ReloadUserStyleCommand" => {
                ThemeManager::reload_user_style();
                self.bump();
            }
            "CopyDiagnosticsCommand" => self.copy_diagnostics(),
            "RestoreTaskbarCommand" => {
                koy_platform::shell::taskbar_service::restore(0, true);
                // Dock çalışıyorsa Koy'un tepsi penceresi de bırakılır.
                self.hook(|h| {
                    if let Some(f) = &h.notify_taskbar_restored_manually {
                        f();
                    }
                });
            }
            "RequestResetCommand" => {
                self.st.borrow_mut().is_reset_confirming = true;
                self.bump();
            }
            "CancelResetCommand" => {
                self.st.borrow_mut().is_reset_confirming = false;
                self.bump();
            }
            "ResetSettingsCommand" => self.reset_settings(),
            "RequestPageResetCommand" => {
                let mut st = self.st.borrow_mut();
                st.page_reset_done = false;
                st.is_page_reset_confirming = true;
                drop(st);
                self.bump();
            }
            "CancelPageResetCommand" => {
                self.st.borrow_mut().is_page_reset_confirming = false;
                self.bump();
            }
            "ConfirmPageResetCommand" => self.confirm_page_reset(),
            "ConfirmApplyRevertCommand" => self.decide_apply_revert(ApplyRevertChoice::Confirm),
            "RevertApplyRevertCommand" => self.decide_apply_revert(ApplyRevertChoice::Revert),
            "RequestUninstallCommand" => {
                if can_uninstall() {
                    let mut st = self.st.borrow_mut();
                    st.uninstall_error.clear();
                    st.keep_settings_on_uninstall = true;
                    st.is_uninstall_confirming = true;
                    drop(st);
                    self.bump();
                }
            }
            "CancelUninstallCommand" => {
                self.st.borrow_mut().is_uninstall_confirming = false;
                self.bump();
            }
            "ToggleKeepSettingsOnUninstall" => {
                let mut st = self.st.borrow_mut();
                st.keep_settings_on_uninstall = !st.keep_settings_on_uninstall;
                drop(st);
                self.bump();
            }
            "ConfirmUninstallCommand" => {
                let keep = self.st.borrow().keep_settings_on_uninstall;
                let started = super::app_uninstaller::begin(keep, || {
                    self.hook(|h| {
                        if let Some(f) = &h.exit_app {
                            f();
                        }
                    })
                });
                if !started {
                    self.st.borrow_mut().uninstall_error = Strings::get("UninstallFailed");
                    self.bump();
                }
            }
            "OpenGitHubCommand" => open_external("https://github.com/koyhq/koy"),
            "OpenWebsiteCommand" => open_external("https://koyhq.github.io/koy"),
            "OpenNectarCommand" => open_external("https://github.com/kua8/Nectar"),
            "OpenSeelenUiCommand" => open_external("https://github.com/eythaann/Seelen-UI"),
            "OpenOpenMeteoCommand" => open_external("https://open-meteo.com/"),
            "OpenOpenStreetMapCommand" => open_external("https://www.openstreetmap.org/copyright"),
            n if n.starts_with("OpenStoreCommand") => {
                let category = n.split_once(':').map_or("", |(_, c)| c).to_string();
                self.hook(|h| {
                    if let Some(f) = &h.open_store {
                        f(&category);
                    }
                });
                self.select_page(EXTENSIONS_PAGE_INDEX);
            }
            _ => {}
        }
    }

    fn pick_live_background_file(&self) {
        let pattern = koy_core::notch::LiveBackgroundFileValidator::dialog_pattern();
        let filter = format!("{}|{}", Strings::get("LiveBgFileFilter"), pattern);
        let Some(p) = crate::infra::file_dialog::pick_file(None, &Strings::get("LiveBgFileChoose"), &filter, true) else {
            return;
        };
        let size = std::fs::metadata(&p).map_or(0, |m| m.len() as i64);
        if koy_core::notch::LiveBackgroundFileValidator::validate(Some(&p), size) != koy_core::notch::LiveBackgroundFileError::None {
            return;
        }
        self.set("Notch.LiveBackgroundFilePath", &p);
    }

    fn pick_start_icon(&self) {
        let filter = format!("{}|*.png;*.jpg;*.jpeg;*.ico;*.bmp", Strings::get("DockStartIconFilter"));
        let Some(path) = crate::infra::file_dialog::pick_file(None, &Strings::get("DockStartIconChoose"), &filter, true) else {
            return;
        };
        self.set("Dock.StartIconPath", &path);
        self.set("Dock.StartIcon", "Custom");
    }

    fn pick_icon_pack_folder(&self) {
        let Some(path) = crate::infra::file_dialog::pick_folder(None, &Strings::get("IconPackChooseFolder")) else { return };
        self.set("Appearance.IconPackFolder", &path);
        self.set("Appearance.IconPack", "Folder");
    }

    fn copy_diagnostics(&self) {
        self.refresh_memory();
        let mb = self.st.borrow().memory_mb;
        let text = format!(
            "Koy v{}\nOS: {}\nPrivate Memory: {:.1} MB\nProcess ID: {}",
            crate::update_status::AppInfo::version(),
            os_version_string(),
            mb,
            std::process::id()
        );
        let _ = crate::infra::clipboard_text::set_text(&text);
    }
}

/// Sayfa başına "varsayılana döndür" bölümleri. Dock ve Ekranlar yok (görev çubuğu / AppBar etkisi).
pub fn reset_sections_for(page: i32) -> &'static [&'static str] {
    match page {
        0 => &["General"],
        // Hava durumu konumu kullanıcı verisidir, sayfa sıfırlamada korunur.
        1 => &["Notch", "Music", "Focus", "Search", "Clipboard"],
        4 => &["Appearance"],
        5 => &["Shortcuts"],
        6 => &["Voice"],
        _ => &[],
    }
}

fn sub_tab_slot(page: i32) -> Option<usize> {
    match page {
        0 => Some(0),
        1 => Some(1),
        2 => Some(2),
        4 => Some(3),
        _ => None,
    }
}

fn panel_of(name: &str) -> Option<NotchPanel> {
    Some(match name {
        "Focus" => NotchPanel::Focus,
        "Shelf" => NotchPanel::Shelf,
        "Calendar" => NotchPanel::Calendar,
        "ControlCenter" => NotchPanel::ControlCenter,
        "Clipboard" => NotchPanel::Clipboard,
        "System" => NotchPanel::System,
        _ => return None,
    })
}

/// Ana her zaman açık; sekme sırası sabit, ayar yalnız hangilerinin açık olduğunu tutar (SaveSettings sırası).
fn enabled_panel_keys(enabled: &[NotchPanel]) -> ValueList<String> {
    let mut keys = vec![NotchPanels::to_key(NotchPanel::Home).to_string()];
    for panel in
        [NotchPanel::Focus, NotchPanel::Calendar, NotchPanel::Shelf, NotchPanel::Clipboard, NotchPanel::System, NotchPanel::ControlCenter]
    {
        if enabled.contains(&panel) {
            keys.push(NotchPanels::to_key(panel).to_string());
        }
    }
    ValueList::from_items(keys)
}

fn home_card_label(card: HomeCard) -> String {
    Strings::get(match card {
        HomeCard::Music => "HomeCardMusic",
        HomeCard::HeadphoneBattery => "HomeCardHeadphoneBattery",
        HomeCard::SystemNetwork => "HomeCardSystemNetwork",
        HomeCard::QuickLaunch => "HomeCardQuickLaunch",
        HomeCard::Weather => "HomeCardWeather",
        HomeCard::Timers => "HomeCardTimers",
    })
}

/// Dosyadaki (elle yazılmış) gecikme listede yoksa en yakın seçenek gösterilir.
fn nearest_delay(ms: i32) -> i32 {
    let clamped = NotchOpenPolicy::clamp_delay(ms);
    *NOTCH_DELAY_OPTIONS.iter().min_by_key(|o| (*o - clamped).abs()).unwrap_or(&0)
}

fn format_number(v: f64) -> String {
    if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{v}") }
}

fn motion_toggles_active(s: &AppSettings) -> bool {
    koy_core::appearance::MotionPolicy::scale(
        s.appearance.animation_speed,
        s.appearance.respect_reduce_motion,
        crate::resources::motion::client_area_animation(),
        false,
    ) > 0.0
}

/// `SaveSettings`'in sınırlamaları (dosyaya yazılmadan önce).
fn normalize(s: &mut AppSettings) {
    s.notch.edge_line_thickness = NotchEdgeLine::clamp_thickness(s.notch.edge_line_thickness);
    s.notch.open_delay_ms = NotchOpenPolicy::clamp_delay(s.notch.open_delay_ms);
    s.notch.close_delay_ms = NotchOpenPolicy::clamp_delay(s.notch.close_delay_ms);
    s.notch.live_background_scrim_percent = LiveBackgroundLimits::clamp_scrim_percent(s.notch.live_background_scrim_percent);
    s.music.visualizer_bar_count = NotchVisualizer::clamp_bar_count(s.music.visualizer_bar_count);
    s.appearance.base_font_size = FontScalePolicy::clamp_base_font_size(s.appearance.base_font_size);
    s.appearance.font_weight = FontScalePolicy::clamp_font_weight(s.appearance.font_weight);
    s.appearance.ui_scale_percent = FontScalePolicy::clamp_ui_scale(s.appearance.ui_scale_percent);
    if s.voice.model.trim().is_empty() {
        s.voice.model = "default".to_string();
    }
    koy_core::dock::DockLayoutLimits::sanitize(&mut s.dock);
    s.hot_corners.dwell_ms = koy_core::input::CornerActions::clamp_dwell(s.hot_corners.dwell_ms);
}

fn affects_theme(key: &str) -> bool {
    key.starts_with("Motion.")
        || matches!(
            key,
            "Appearance.Theme"
                | "Appearance.AccentColor"
                | "Appearance.UseWindowsAccent"
                | "Appearance.Material"
                | "Appearance.MaterialReadabilityGuard"
                | "Notch.FollowsMaterial"
                | "Appearance.CornerRadius"
                | "Appearance.AnimationSpeed"
                | "Appearance.RespectReduceMotion"
                | "Appearance.BatterySaverReduceMotion"
                | "Appearance.FontFamily"
                | "Appearance.ClockFontFamily"
                | "Appearance.BaseFontSize"
                | "Appearance.FontWeight"
                | "Appearance.UiScalePercent"
                | "Appearance.UserStyleOverridesEnabled"
        )
}

/// `AffectsDock`: dock bölümü (açık/kapalı hariç) ve dock'un okuduğu görünüm ayarları.
fn affects_dock(key: &str) -> bool {
    matches!(key, "Displays.DockOn" | "Displays.NewScreenDefault")
        || (key.starts_with("Dock.") && key != "Dock.Enabled")
        || matches!(
            key,
            "Appearance.BatterySaverReduceMotion"
                | "Appearance.Material"
                | "Appearance.IconPack"
                | "Appearance.IconPackFolder"
                | "Appearance.MaterialReadabilityGuard"
                | "Appearance.AnimationSpeed"
                | "Appearance.RespectReduceMotion"
        )
}

/// `AffectsNotch`.
fn affects_notch(key: &str) -> bool {
    matches!(key, "Displays.NotchOn" | "Displays.NewScreenDefault")
        || (key.starts_with("Notch.") && key != "Notch.Enabled")
        || key.starts_with("Music.")
        || key.starts_with("Focus.")
        || key.starts_with("Voice.")
        || key.starts_with("Search.")
        || key.starts_with("Ui.Panel")
        || matches!(
            key,
            "Clipboard.HistoryEnabled"
                | "Weather.RefreshInterval"
                | "Weather.Location"
                | "General.TemperatureUnit"
                | "General.WindSpeedUnit"
                | "General.NetworkSpeedUnit"
                | "Appearance.Material"
                | "Appearance.MaterialReadabilityGuard"
                | "Appearance.BatterySaverReduceMotion"
                | "Dock.WheelAction"
        )
}

fn can_uninstall() -> bool {
    crate::features::settings::app_uninstaller::availability() == crate::features::settings::app_uninstaller::UninstallAvailability::Available
}

fn last_check_text() -> String {
    use crate::update_status::UpdateStatus;
    match UpdateStatus::last_check() {
        None => Strings::get("UpdateNeverChecked"),
        Some(t) => {
            let now = chrono::Local::now();
            let when = if t.date_naive() == now.date_naive() {
                t.format("%H:%M").to_string()
            } else {
                t.format("%d.%m.%Y %H:%M").to_string()
            };
            Strings::format("UpdateLastCheck", &[&when])
        }
    }
}

fn about_status_text() -> String {
    use crate::update_status::UpdateStatus;
    match UpdateStatus::available_version() {
        Some(v) => Strings::format("AboutUpdateAvailable", &[&v]),
        None => Strings::get("AboutUpToDate"),
    }
}

/// Durum satırı: kapalı / dosya yok / "N uygulandı, M yok sayıldı" (C# `UserStyleResources.StatusText`).
fn user_style_status_text(s: &AppSettings) -> String {
    if !s.appearance.user_style_overrides_enabled {
        return Strings::get("UserStyleStatusOff");
    }
    if !ThemeManager::user_style_path().exists() {
        return Strings::get("UserStyleStatusNoFile");
    }
    let result = ThemeManager::user_style_result();
    let ignored = result.diagnostics.len() as i32 + result.omitted_diagnostics;
    Strings::format("UserStyleStatusLoaded", &[&result.valid_entries.len().to_string(), &ignored.to_string()])
}

/// "Windows 11 Pro 26100" biçiminde işletim sistemi satırı (C# `Environment.OSVersion.VersionString` yerine).
fn os_version_string() -> String {
    use koy_platform::registry::{Hive, get_string};
    const KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let name = get_string(Hive::LocalMachine, KEY, Some("ProductName"), false).unwrap_or_else(|| "Windows".to_string());
    let build = get_string(Hive::LocalMachine, KEY, Some("CurrentBuild"), false).unwrap_or_default();
    format!("{name} {build}").trim().to_string()
}

/// Bağlantı ya da klasörü varsayılan uygulamayla açar (C# `Process.Start(UseShellExecute)`).
pub fn open_external(target: &str) {
    koy_platform::wireless::SettingsUri::open(target);
}
