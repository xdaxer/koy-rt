//! Karşılığı: `koy/src/App/Features/Settings/KoyWindow.xaml.cs`.
//!
//! Koy (ayarlar) penceresi: çerçevesiz, kapatınca gizlenir (Koy kapanmaz). Görünürken settings.json izlenir; dışarıdan
//! yapılan değişiklik pencereye yansır. Görünüm modeline `Tr` (metinler), `S` (anahtarlı ayar satırları), `SL`
//! (listeler) ve `KoyWindowData` (sayfa, arama) küreselleriyle bağlanır.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::infra::native_window;
use crate::resources::strings::Strings;
use crate::resources::theme_manager::ThemeManager;
use crate::resources::user_style_watcher::UserStyleWatcher;
use crate::{ReleaseSectionData, UP, AD, AdvancedRowData, BuiltinCardData, ConfirmFactData, EX, StoreCardData, CA, DP, PR, RU, WX, ProfileItemData, RuleItemData, SectionOptionData, KoyWindow, KoyWindowData, MonitorCardData, OrderItem, S, SC, SL, SearchResultData, ShortcutRowData, ToggleItem, Tr};

use super::advanced_view_model::AdvancedViewModel;
use crate::features::extensions::extension_store_view_model::ExtensionStoreViewModel;
use super::accounts_view_models::{CalendarAccountsViewModel, WeatherLocationViewModel};
use super::displays_view_model::DisplaysViewModel;
use super::profile_service::ProfileService;
use super::profiles_view_model::{ProfilesViewModel, section_label};
use super::rules_view_model::RulesViewModel;
use crate::features::rules::context_rule_service::ContextRuleService;
use super::settings_view_model::SettingsViewModel;
use super::shortcuts_view_model::ShortcutsViewModel;

thread_local! {
    static CURRENT: RefCell<Option<std::rc::Weak<KoyWindowHost>>> = const { RefCell::new(None) };
}

pub struct KoyWindowHost {
    window: KoyWindow,
    vm: Rc<SettingsViewModel>,
    shortcuts: Rc<ShortcutsViewModel>,
    displays: Rc<DisplaysViewModel>,
    profiles: Rc<ProfilesViewModel>,
    rules: Rc<RulesViewModel>,
    advanced: Rc<AdvancedViewModel>,
    extensions: Rc<ExtensionStoreViewModel>,
    weather: Rc<WeatherLocationViewModel>,
    calendar: Rc<CalendarAccountsViewModel>,
    watcher: RefCell<Option<UserStyleWatcher>>,
    revert_timer: slint::Timer,
    highlight_timer: slint::Timer,
}

fn ss(s: impl AsRef<str>) -> SharedString {
    SharedString::from(s.as_ref())
}

impl KoyWindowHost {
    pub fn new(vm: Rc<SettingsViewModel>) -> Result<Rc<Self>, slint::PlatformError> {
        let window = KoyWindow::new()?;
        ThemeManager::install(&window);
        let shortcuts = ShortcutsViewModel::new(Rc::clone(&vm));
        let displays = DisplaysViewModel::new(Rc::clone(&vm));
        let profiles = ProfilesViewModel::new(ProfileService::instance());
        let advanced = AdvancedViewModel::new(Rc::clone(&vm));
        let extensions = ExtensionStoreViewModel::new(ProfileService::instance());
        let rules = RulesViewModel::new(ProfileService::instance(), ContextRuleService::instance());
        let weather = WeatherLocationViewModel::new(Rc::clone(&vm));
        let calendar = CalendarAccountsViewModel::new();
        let host = Rc::new(KoyWindowHost {
            window,
            vm,
            shortcuts,
            displays,
            profiles,
            rules,
            advanced,
            extensions,
            weather,
            calendar,
            watcher: RefCell::new(None),
            revert_timer: slint::Timer::default(),
            highlight_timer: slint::Timer::default(),
        });
        host.bind();
        host.sync();
        CURRENT.with(|c| *c.borrow_mut() = Some(Rc::downgrade(&host)));
        let weak = Rc::downgrade(&host);
        host.vm.changed.subscribe(move || {
            if let Some(h) = weak.upgrade() {
                h.sync();
            }
        });
        let weak = Rc::downgrade(&host);
        host.shortcuts.changed.subscribe(move || {
            if let Some(h) = weak.upgrade() {
                h.sync_shortcuts();
            }
        });
        let weak = Rc::downgrade(&host);
        host.displays.changed.subscribe(move || {
            if let Some(h) = weak.upgrade() {
                h.sync_displays();
            }
        });
        host.sync_shortcuts();
        let weak = Rc::downgrade(&host);
        host.profiles.changed.subscribe(move || {
            if let Some(h) = weak.upgrade() {
                h.sync_profiles();
            }
        });
        let weak = Rc::downgrade(&host);
        host.rules.changed.subscribe(move || {
            if let Some(h) = weak.upgrade() {
                h.sync_rules();
            }
        });
        // Profil listesi değişince kuralların profil seçenekleri yenilenir.
        let r = Rc::downgrade(&host.rules);
        host.profiles.list_changed.subscribe(move |_| {
            if let Some(r) = r.upgrade() {
                r.refresh();
            }
        });
        // Profil (bu pencereden ya da bir kuraldan) uygulandı / eklenti ayarı yazdı: değerler diskten okunur.
        let weak = Rc::downgrade(&host);
        ProfileService::instance().applied.subscribe(move |_| {
            if let Some(h) = weak.upgrade() {
                h.vm.load_from_settings();
            }
        });
        let weak = Rc::downgrade(&host);
        ProfileService::instance().settings_committed.subscribe(move || {
            if let Some(h) = weak.upgrade() {
                h.vm.load_from_settings();
            }
        });
        let weak = Rc::downgrade(&host);
        host.weather.changed.subscribe(move || {
            if let Some(h) = weak.upgrade() {
                h.sync_weather();
            }
        });
        let weak = Rc::downgrade(&host);
        host.calendar.changed.subscribe(move || {
            if let Some(h) = weak.upgrade() {
                h.sync_calendar();
            }
        });
        let weak = Rc::downgrade(&host);
        host.advanced.changed.subscribe(move || {
            if let Some(h) = weak.upgrade() {
                h.sync_advanced();
            }
        });
        let weak = Rc::downgrade(&host);
        host.extensions.changed.subscribe(move || {
            if let Some(h) = weak.upgrade() {
                h.sync_extensions();
            }
        });
        // Eklenti profil ekledi: Genel sayfasındaki profil listesi tazelenir.
        let p = Rc::downgrade(&host.profiles);
        host.extensions.profiles_changed.subscribe(move |_| {
            if let Some(p) = p.upgrade() {
                p.refresh();
            }
        });
        host.sync_profiles();
        host.sync_rules();
        host.sync_weather();
        host.sync_calendar();
        Ok(host)
    }

    pub fn window(&self) -> &KoyWindow {
        &self.window
    }

    pub fn view_model(&self) -> &Rc<SettingsViewModel> {
        &self.vm
    }

    pub fn shortcuts(&self) -> &Rc<ShortcutsViewModel> {
        &self.shortcuts
    }

    fn sync_shortcuts(&self) {
        let rows: Vec<ShortcutRowData> = self
            .shortcuts
            .rows()
            .into_iter()
            .map(|r| ShortcutRowData {
                title: ss(&r.title),
                tokens: ModelRc::new(VecModel::from(r.key_tokens().into_iter().map(SharedString::from).collect::<Vec<_>>())),
                capturing: r.is_capturing,
                error: ss(r.error_text.clone().unwrap_or_default()),
                is_default: r.is_default(),
                has_shortcut: r.has_shortcut(),
            })
            .collect();
        self.window.global::<SC>().set_rows(ModelRc::new(VecModel::from(rows)));
    }

    fn sync_profiles(&self) {
        let pr = self.window.global::<PR>();
        let items: Vec<ProfileItemData> = self
            .profiles
            .items()
            .into_iter()
            .map(|i| ProfileItemData {
                name: ss(&i.name),
                sections: ss(&i.sections_text),
                renaming: i.is_renaming,
                edit_name: ss(&i.edit_name),
                delete_confirming: i.is_delete_confirming,
            })
            .collect();
        pr.set_items(ModelRc::new(VecModel::from(items)));
        let sections: Vec<SectionOptionData> = self
            .profiles
            .sections()
            .into_iter()
            .map(|o| SectionOptionData { label: ss(section_label(o.section)), selected: o.is_selected })
            .collect();
        pr.set_sections(ModelRc::new(VecModel::from(sections)));
        pr.set_new_name(ss(self.profiles.new_name()));
        let (status, error) = self.profiles.status();
        pr.set_status(ss(status));
        pr.set_status_error(error);
        match self.profiles.pending() {
            Some((name, sections, changes)) => {
                pr.set_has_pending(true);
                pr.set_pending_name(ss(name));
                pr.set_pending_sections(ss(sections));
                pr.set_pending_changes(ss(changes));
            }
            None => pr.set_has_pending(false),
        }
    }

    /// Hesap bağlandı / kesildi / eşitlendi (çentiğin takvimi önbellekten yeniden okur).
    pub fn calendar_accounts(&self) -> &Rc<CalendarAccountsViewModel> {
        &self.calendar
    }

    fn sync_weather(&self) {
        let wx = self.window.global::<WX>();
        let (use_location, query, busy, status, link, no_results, has_location, results) = self.weather.view();
        wx.set_use_location(use_location);
        wx.set_query(ss(query));
        wx.set_busy(busy);
        wx.set_status(ss(status));
        wx.set_show_settings_link(link);
        wx.set_no_results(no_results);
        wx.set_has_location(has_location);
        wx.set_results(ModelRc::new(VecModel::from(results.into_iter().map(SharedString::from).collect::<Vec<_>>())));
    }

    fn sync_calendar(&self) {
        let ca = self.window.global::<CA>();
        let v = self.calendar.view();
        ca.set_google_client_id(ss(&v.google_client_id));
        ca.set_google_client_secret(ss(&v.google_client_secret));
        ca.set_show_google_fields(v.show_google_client_fields);
        ca.set_outlook_client_id(ss(&v.outlook_client_id));
        ca.set_google_connected(v.is_google_connected);
        ca.set_outlook_connected(v.is_outlook_connected);
        ca.set_outlook_available(v.is_outlook_available);
        ca.set_google_busy(v.is_google_busy);
        ca.set_outlook_busy(v.is_outlook_busy);
        ca.set_google_status(ss(&v.google_status_text));
        ca.set_outlook_status(ss(&v.outlook_status_text));
        ca.set_message(ss(&v.status_message));
    }

    pub fn extensions(&self) -> &Rc<ExtensionStoreViewModel> {
        &self.extensions
    }

    fn sync_extensions(&self) {
        let ex = self.window.global::<EX>();
        let m = &self.extensions;
        let cards: Vec<StoreCardData> = m
            .cards()
            .into_iter()
            .map(|c| StoreCardData {
                id: ss(&c.id),
                name: ss(&c.name),
                meta: ss(&c.meta_text),
                kind_label: ss(&c.kind_label),
                summary: ss(&c.summary),
                description: ss(&c.description),
                details: ss(&c.details_text),
                has_homepage: c.has_homepage,
                busy: c.is_busy,
                show_install: c.show_install,
                show_update: c.show_update,
                show_apply: c.show_apply,
                applied: c.is_applied,
                show_uninstall: c.show_uninstall,
                apply_label: ss(&c.apply_label),
                progress: c.progress as f32,
                uninstall_confirming: c.is_uninstall_confirming,
                expanded: c.is_expanded,
                message: ss(&c.message),
                message_error: c.message_is_error,
                has_preview: c.preview.is_some(),
                preview: c.preview.unwrap_or_default(),
            })
            .collect();
        ex.set_cards(ModelRc::new(VecModel::from(cards)));
        let builtin: Vec<BuiltinCardData> = m
            .builtin_cards()
            .into_iter()
            .map(|b| BuiltinCardData {
                id: ss(&b.id),
                name: ss(&b.name),
                kind_label: ss(&b.kind_label),
                c1: hex_color(b.colors.first().map(String::as_str).unwrap_or("#333333")),
                c2: hex_color(b.colors.get(1).or(b.colors.first()).map(String::as_str).unwrap_or("#333333")),
                applied: b.is_applied,
            })
            .collect();
        ex.set_builtin(ModelRc::new(VecModel::from(builtin)));
        ex.set_search(ss(m.search_text()));
        ex.set_category(m.category_index());
        ex.set_filter(m.filter_index());
        ex.set_sort(m.sort_index());
        let (loading, index_error, status, status_error, updates, count, show_more, has_more, empty, can_file) = m.texts();
        ex.set_loading(loading);
        ex.set_index_error(ss(index_error));
        ex.set_status(ss(status));
        ex.set_status_error(status_error);
        ex.set_updates_label(ss(updates));
        ex.set_count_text(ss(count));
        ex.set_show_more_text(ss(show_more));
        ex.set_has_more(has_more);
        ex.set_empty(empty);
        ex.set_can_install_file(can_file);
        let (confirming, title, button, note, facts) = m.confirm();
        ex.set_confirming(confirming);
        ex.set_confirm_title(ss(title));
        ex.set_confirm_button(ss(button));
        ex.set_confirm_note(ss(note));
        let facts: Vec<ConfirmFactData> = facts.into_iter().map(|f| ConfirmFactData { label: ss(f.label), value: ss(f.value) }).collect();
        ex.set_confirm_facts(ModelRc::new(VecModel::from(facts)));
    }

    fn sync_advanced(&self) {
        use koy_core::settings::AdvancedSettingKind as K;
        let ad = self.window.global::<AD>();
        let rows: Vec<AdvancedRowData> = self
            .advanced
            .rows()
            .into_iter()
            .map(|r| {
                let choice_index = r.choices.iter().position(|c| c.eq_ignore_ascii_case(&r.value)).map_or(-1, |i| i as i32);
                AdvancedRowData {
                    header: r.is_header,
                    key: ss(&r.key),
                    title: ss(&r.title),
                    kind: match r.kind {
                        K::Bool => 0,
                        K::Integer => 1,
                        K::Number => 2,
                        K::Text => 3,
                        K::Choice => 4,
                    },
                    value: ss(&r.value),
                    default_text: ss(&r.default_text),
                    choices: ModelRc::new(VecModel::from(r.choices.into_iter().map(SharedString::from).collect::<Vec<_>>())),
                    choice_index,
                    changed: r.is_changed,
                    error: ss(&r.error),
                }
            })
            .collect();
        ad.set_rows(ModelRc::new(VecModel::from(rows)));
        ad.set_filter(ss(self.advanced.filter()));
    }

    fn sync_rules(&self) {
        let ru = self.window.global::<RU>();
        let items: Vec<RuleItemData> = self
            .rules
            .items()
            .into_iter()
            .map(|r| RuleItemData { title: ss(&r.title), profile: ss(&r.profile_text), enabled: r.is_enabled })
            .collect();
        ru.set_items(ModelRc::new(VecModel::from(items)));
        let apps: Vec<SharedString> = self.rules.running_apps().into_iter().map(SharedString::from).collect();
        ru.set_running_apps(ModelRc::new(VecModel::from(apps)));
        let opts: Vec<SharedString> = self.rules.profile_options().into_iter().map(|o| SharedString::from(o.name)).collect();
        ru.set_profile_options(ModelRc::new(VecModel::from(opts)));
        ru.set_selected_profile(self.rules.selected_profile());
        ru.set_process_text(ss(self.rules.process_text()));
        let (status, error) = self.rules.status();
        ru.set_status(ss(status));
        ru.set_status_error(error);
    }

    fn sync_displays(&self) {
        use koy_core::display::NewScreenMode;
        let dp = self.window.global::<DP>();
        let cards: Vec<MonitorCardData> = self
            .displays
            .cards()
            .into_iter()
            .map(|c| MonitorCardData {
                number: c.number,
                left: c.left as f32,
                top: c.top as f32,
                width: c.width as f32,
                height: c.height as f32,
                selected: c.is_selected,
                primary: c.monitor.is_primary,
                notch: c.shows_notch,
                dock: c.shows_dock,
            })
            .collect();
        dp.set_cards(ModelRc::new(VecModel::from(cards)));
        let (w, h) = self.displays.schema_size();
        dp.set_schema_width(w as f32);
        dp.set_schema_height(h as f32);
        match self.displays.selected() {
            Some((header, name, details, primary, notch, dock)) => {
                dp.set_has_selection(true);
                dp.set_header(ss(header));
                dp.set_name(ss(name));
                dp.set_details(ss(details));
                dp.set_primary(primary);
                dp.set_notch_on(notch);
                dp.set_dock_on(dock);
            }
            None => dp.set_has_selection(false),
        }
        dp.set_new_screen(match self.displays.new_screen_mode() {
            NewScreenMode::NotchAndDock => 0,
            NewScreenMode::NotchOnly => 1,
            NewScreenMode::None => 2,
        });
    }

    fn bind(self: &Rc<Self>) {
        let w = &self.window;

        let tr = w.global::<Tr>();
        tr.on_lookup(|k, _| ss(Strings::get(&k)));
        tr.on_format1(|k, a, _| ss(Strings::format(&k, &[&a])));

        let s = w.global::<S>();
        let vm = Rc::clone(&self.vm);
        s.on_value(move |k, _| ss(vm.value(&k)));
        let vm = Rc::clone(&self.vm);
        s.on_is_changed_of(move |k, _| vm.is_changed(&k));
        let vm = Rc::clone(&self.vm);
        s.on_flag_of(move |k, _| vm.flag(&k));
        let vm = Rc::clone(&self.vm);
        s.on_text_of(move |k, _| ss(vm.text(&k)));
        let vm = Rc::clone(&self.vm);
        s.on_list_of(move |k, _| {
            let items: Vec<SharedString> = vm.list(&k).into_iter().map(SharedString::from).collect();
            ModelRc::new(VecModel::from(items))
        });
        let vm = Rc::clone(&self.vm);
        s.on_choice_index(move |k, values, _| {
            let values: Vec<String> = values.iter().map(|v| v.to_string()).collect();
            vm.choice_index(&k, &values)
        });
        // Yazma ve eylemler bağlama değerlendirmesi sırasında değil, sonraki turda yapılır (pencere yeniden çizilirken
        // görünüm modelinin ödünç alınmış olmaması için).
        let vm = Rc::downgrade(&self.vm);
        s.on_set(move |k, v| {
            let vm = vm.clone();
            crate::infra::dispatch::post_local(move || {
                if let Some(vm) = vm.upgrade() {
                    vm.set(&k, &v);
                }
            });
        });
        let vm = Rc::downgrade(&self.vm);
        let me = Rc::downgrade(self);
        s.on_action(move |name| {
            let vm = vm.clone();
            let me = me.clone();
            crate::infra::dispatch::post_local(move || {
                // "Mağazadan daha fazla al…": Eklentiler sayfası verilen kategoriyle açılır.
                if let Some(category) = name.strip_prefix("OpenStoreCommand") {
                    if let Some(h) = me.upgrade() {
                        h.extensions.show_category(category.trim_start_matches(':'));
                        h.vm.select_page(super::settings_view_model::EXTENSIONS_PAGE_INDEX);
                        h.update_page_activation();
                        h.window.invoke_scroll_top();
                    }
                    return;
                }
                if let Some(vm) = vm.upgrade() {
                    vm.action(&name);
                }
            });
        });

        let sl = w.global::<SL>();
        let vm = Rc::clone(&self.vm);
        sl.on_tray_app_toggled(move |i, v| vm.set_tray_row(i as usize, v));
        let vm = Rc::clone(&self.vm);
        sl.on_search_source_toggled(move |i, v| vm.set_search_source(i as usize, v));
        let vm = Rc::clone(&self.vm);
        sl.on_home_card_toggled(move |i, v| vm.set_home_card_enabled(i as usize, v));
        let vm = Rc::clone(&self.vm);
        sl.on_home_card_move(move |i, step| vm.move_home_card(i as usize, step));
        let vm = Rc::clone(&self.vm);
        sl.on_tab_move(move |i, step| vm.move_tab(i as usize, step));

        let sc = w.global::<SC>();
        let m = Rc::downgrade(&self.shortcuts);
        sc.on_start_capture(move |i| {
            if let Some(m) = m.upgrade() {
                m.start_capture(i as usize);
            }
        });
        let m = Rc::downgrade(&self.shortcuts);
        sc.on_cancel_capture(move |i| {
            if let Some(m) = m.upgrade() {
                m.cancel_capture(i as usize);
            }
        });
        let m = Rc::downgrade(&self.shortcuts);
        sc.on_reset(move |i| {
            let m = m.clone();
            crate::infra::dispatch::post_local(move || {
                if let Some(m) = m.upgrade() {
                    m.reset_to_default(i as usize);
                }
            });
        });
        let m = Rc::downgrade(&self.shortcuts);
        sc.on_key(move |i, text, ctrl, alt, shift, win| m.upgrade().is_some_and(|m| m.handle_key(i as usize, &text, ctrl, alt, shift, win)));

        let pr = w.global::<PR>();
        let m = Rc::downgrade(&self.profiles);
        pr.on_new_name_edited(move |t| {
            if let Some(m) = m.upgrade() {
                m.set_new_name(&t);
            }
        });
        let m = Rc::downgrade(&self.profiles);
        pr.on_section_toggled(move |i, v| {
            if let Some(m) = m.upgrade() {
                m.set_section_selected(i as usize, v);
            }
        });
        let m = Rc::downgrade(&self.profiles);
        pr.on_edit_name_edited(move |i, t| {
            if let Some(m) = m.upgrade() {
                m.set_edit_name(i as usize, &t);
            }
        });
        // Dosya pencereleri ve profil uygulama kendi ileti döngüsünü çalıştırır: sonraki turda.
        let m = Rc::downgrade(&self.profiles);
        pr.on_command(move |name| {
            let m = m.clone();
            crate::infra::dispatch::post_local(move || {
                let Some(m) = m.upgrade() else { return };
                match name.as_str() {
                    "SaveCurrent" => m.save_current(),
                    "ExportCurrent" => m.export_current(),
                    "Import" => m.import(),
                    "ConfirmImportSave" => m.confirm_import_save(),
                    "ConfirmImportApply" => m.confirm_import_apply(),
                    "CancelImport" => m.cancel_import(),
                    _ => {}
                }
            });
        });
        let m = Rc::downgrade(&self.profiles);
        pr.on_item_command(move |i, name| {
            let m = m.clone();
            crate::infra::dispatch::post_local(move || {
                let Some(m) = m.upgrade() else { return };
                let i = i as usize;
                match name.as_str() {
                    "Apply" => m.apply(i),
                    "Export" => m.export(i),
                    "Duplicate" => m.duplicate(i),
                    "BeginRename" => m.begin_rename(i),
                    "CommitRename" => m.commit_rename(i),
                    "CancelRename" => m.cancel_rename(i),
                    "RequestDelete" => m.request_delete(i, true),
                    "CancelDelete" => m.request_delete(i, false),
                    "ConfirmDelete" => m.delete(i),
                    _ => {}
                }
            });
        });

        let ex = w.global::<EX>();
        let m = Rc::downgrade(&self.extensions);
        ex.on_search_edited(move |t| {
            if let Some(m) = m.upgrade() {
                m.set_search_text(&t);
            }
        });
        let m = Rc::downgrade(&self.extensions);
        ex.on_category_changed(move |i| {
            if let Some(m) = m.upgrade() {
                m.set_category(i as usize);
            }
        });
        let m = Rc::downgrade(&self.extensions);
        ex.on_filter_changed(move |i| {
            if let Some(m) = m.upgrade() {
                m.set_filter(i as usize);
            }
        });
        let m = Rc::downgrade(&self.extensions);
        ex.on_sort_changed(move |i| {
            if let Some(m) = m.upgrade() {
                m.set_sort(i as usize);
            }
        });
        // Dosya pencereleri ve uygulama (çentik/dock yeniden kurulumu) bağlama dışında, sonraki turda.
        let m = Rc::downgrade(&self.extensions);
        ex.on_command(move |name| {
            let m = m.clone();
            crate::infra::dispatch::post_local(move || {
                let Some(m) = m.upgrade() else { return };
                match name.as_str() {
                    "RefreshIndex" => m.refresh(),
                    "InstallFromFile" => m.install_from_file(),
                    "ShowMore" => m.show_more(),
                    "ConfirmInstall" => m.confirm_install(),
                    "CancelConfirm" => m.cancel_confirm(),
                    _ => {}
                }
            });
        });
        let m = Rc::downgrade(&self.extensions);
        ex.on_card_command(move |id, name| {
            let m = m.clone();
            crate::infra::dispatch::post_local(move || {
                let Some(m) = m.upgrade() else { return };
                match name.as_str() {
                    "Install" => m.request_install(&id, false),
                    "Update" => m.request_install(&id, true),
                    "Apply" => m.apply(&id),
                    "Disable" => m.disable(&id),
                    "RequestUninstall" => m.request_uninstall(&id, true),
                    "CancelUninstall" => m.request_uninstall(&id, false),
                    "ConfirmUninstall" => m.uninstall(&id),
                    "ToggleExpanded" => m.toggle_expanded(&id),
                    "OpenHomepage" => m.open_homepage(&id),
                    _ => {}
                }
            });
        });
        let m = Rc::downgrade(&self.extensions);
        ex.on_builtin_command(move |id, name| {
            let m = m.clone();
            crate::infra::dispatch::post_local(move || {
                let Some(m) = m.upgrade() else { return };
                match name.as_str() {
                    "Apply" => m.apply_builtin_background(&id),
                    "Disable" => m.disable_builtin_background(&id),
                    _ => {}
                }
            });
        });

        let ad = w.global::<AD>();
        let m = Rc::downgrade(&self.advanced);
        ad.on_filter_edited(move |t| {
            if let Some(m) = m.upgrade() {
                m.set_filter(&t);
            }
        });
        let m = Rc::downgrade(&self.advanced);
        ad.on_apply(move |k, v| {
            let m = m.clone();
            crate::infra::dispatch::post_local(move || {
                if let Some(m) = m.upgrade() {
                    m.apply(&k, &v);
                }
            });
        });
        let m = Rc::downgrade(&self.advanced);
        ad.on_reset(move |k| {
            let m = m.clone();
            crate::infra::dispatch::post_local(move || {
                if let Some(m) = m.upgrade() {
                    m.reset(&k);
                }
            });
        });

        let ru = w.global::<RU>();
        let m = Rc::downgrade(&self.rules);
        ru.on_process_edited(move |t| {
            if let Some(m) = m.upgrade() {
                m.set_process_text(&t);
            }
        });
        let m = Rc::downgrade(&self.rules);
        ru.on_pick_app(move |i| {
            if let Some(m) = m.upgrade() {
                m.pick_app(i as usize);
            }
        });
        let m = Rc::downgrade(&self.rules);
        ru.on_select_profile(move |i| {
            if let Some(m) = m.upgrade() {
                m.select_profile(i as usize);
            }
        });
        let m = Rc::downgrade(&self.rules);
        ru.on_refresh_running(move || {
            if let Some(m) = m.upgrade() {
                m.refresh_running_apps();
            }
        });
        let m = Rc::downgrade(&self.rules);
        ru.on_add(move || {
            let m = m.clone();
            crate::infra::dispatch::post_local(move || {
                if let Some(m) = m.upgrade() {
                    m.add();
                }
            });
        });
        let m = Rc::downgrade(&self.rules);
        ru.on_toggled(move |i, v| {
            let m = m.clone();
            crate::infra::dispatch::post_local(move || {
                if let Some(m) = m.upgrade() {
                    m.set_enabled(i as usize, v);
                }
            });
        });
        let m = Rc::downgrade(&self.rules);
        ru.on_delete(move |i| {
            let m = m.clone();
            crate::infra::dispatch::post_local(move || {
                if let Some(m) = m.upgrade() {
                    m.delete(i as usize);
                }
            });
        });

        let wx = w.global::<WX>();
        let m = Rc::downgrade(&self.weather);
        wx.on_use_location_toggled(move |v| {
            if let Some(m) = m.upgrade() {
                m.set_use_location(v);
            }
        });
        let m = Rc::downgrade(&self.weather);
        wx.on_query_edited(move |t| {
            if let Some(m) = m.upgrade() {
                m.set_query(&t);
            }
        });
        let m = Rc::downgrade(&self.weather);
        wx.on_search(move || {
            if let Some(m) = m.upgrade() {
                m.search();
            }
        });
        let m = Rc::downgrade(&self.weather);
        wx.on_select(move |i| {
            if let Some(m) = m.upgrade() {
                m.select_place(i as usize);
            }
        });
        let m = Rc::downgrade(&self.weather);
        wx.on_clear(move || {
            if let Some(m) = m.upgrade() {
                m.clear_location();
            }
        });
        let m = Rc::downgrade(&self.weather);
        wx.on_open_location_settings(move || {
            if let Some(m) = m.upgrade() {
                m.open_location_settings();
            }
        });
        let ca = w.global::<CA>();
        let m = Rc::downgrade(&self.calendar);
        ca.on_field_edited(move |i, t| {
            if let Some(m) = m.upgrade() {
                match i {
                    0 => m.set_google_client_id(&t),
                    1 => m.set_google_client_secret(&t),
                    _ => m.set_outlook_client_id(&t),
                }
            }
        });
        let m = Rc::downgrade(&self.calendar);
        ca.on_command(move |name| {
            use koy_core::calendar::CalendarProvider;
            let m = m.clone();
            crate::infra::dispatch::post_local(move || {
                let Some(m) = m.upgrade() else { return };
                match name.as_str() {
                    "ConnectGoogle" => m.connect(CalendarProvider::Google),
                    "DisconnectGoogle" => m.disconnect(CalendarProvider::Google),
                    "ConnectOutlook" => m.connect(CalendarProvider::Outlook),
                    "DisconnectOutlook" => m.disconnect(CalendarProvider::Outlook),
                    "SyncNow" => m.sync_now(),
                    _ => {}
                }
            });
        });

        let dp = w.global::<DP>();
        let d = Rc::downgrade(&self.displays);
        dp.on_identify(move || {
            if let Some(d) = d.upgrade() {
                d.identify();
            }
        });
        let d = Rc::downgrade(&self.displays);
        dp.on_select(move |i| {
            if let Some(d) = d.upgrade() {
                d.select(Some(i as usize));
            }
        });
        let d = Rc::downgrade(&self.displays);
        dp.on_notch_toggled(move |v| {
            if let Some(d) = d.upgrade() {
                d.set_notch_on(v);
            }
        });
        let d = Rc::downgrade(&self.displays);
        dp.on_dock_toggled(move |v| {
            if let Some(d) = d.upgrade() {
                d.set_dock_on(v);
            }
        });
        let d = Rc::downgrade(&self.displays);
        dp.on_new_screen_changed(move |i| {
            use koy_core::display::NewScreenMode;
            if let Some(d) = d.upgrade() {
                d.set_new_screen_mode(match i {
                    1 => NewScreenMode::NotchOnly,
                    2 => NewScreenMode::None,
                    _ => NewScreenMode::NotchAndDock,
                });
            }
        });

        let data = w.global::<KoyWindowData>();
        let me = Rc::downgrade(self);
        data.on_select_page(move |i| {
            if let Some(h) = me.upgrade() {
                h.vm.clear_highlight();
                h.vm.select_page(i);
                h.update_page_activation();
                h.window.invoke_scroll_top();
            }
        });
        let vm = Rc::clone(&self.vm);
        data.on_select_sub_tab(move |page, sub| vm.select_sub_tab(page, sub));
        let vm = Rc::clone(&self.vm);
        data.on_search_changed(move |q| vm.set_search_query(&q));
        let me = Rc::downgrade(self);
        data.on_open_result(move |i| {
            if let Some(h) = me.upgrade() {
                h.vm.open_search_result(i as usize);
                h.window.invoke_scroll_top();
                // Vurgu bir süre sonra kalkar.
                let vm = Rc::downgrade(&h.vm);
                h.highlight_timer.start(slint::TimerMode::SingleShot, Duration::from_secs(4), move || {
                    if let Some(vm) = vm.upgrade() {
                        vm.clear_highlight();
                    }
                });
            }
        });
        let vm = Rc::clone(&self.vm);
        data.on_changed_only_toggled(move |v| vm.set_show_changed_only(v));

        let me = Rc::downgrade(self);
        w.on_minimize(move || {
            if let Some(h) = me.upgrade() {
                h.window.window().set_minimized(true);
            }
        });
        let me = Rc::downgrade(self);
        w.on_hide_window(move || {
            if let Some(h) = me.upgrade() {
                h.hide();
            }
        });
        let me = Rc::downgrade(self);
        w.on_drag_title(move || {
            if let Some(h) = me.upgrade() {
                begin_drag(native_window::hwnd_of(h.window.window()));
            }
        });
        // Pencereyi kapatmak Koy'u kapatmaz; gizler.
        let me = Rc::downgrade(self);
        w.window().on_close_requested(move || {
            if let Some(h) = me.upgrade() {
                h.hide();
            }
            slint::CloseRequestResponse::KeepWindowShown
        });
    }

    /// Görünüm modelinden pencereye.
    fn sync(&self) {
        let w = &self.window;
        let vm = &self.vm;
        let o = vm.update_overlays();
        let up = w.global::<UP>();
        let strs = |v: Vec<String>| ModelRc::new(VecModel::from(v.into_iter().map(SharedString::from).collect::<Vec<_>>()));
        up.set_update_open(o.update_open);
        up.set_update_title(ss(o.update_title));
        up.set_update_intro(ss(o.update_intro));
        up.set_update_highlights(strs(o.update_highlights));
        up.set_downloading(o.downloading);
        up.set_progress(o.download_progress as f32);
        up.set_download_text(ss(o.download_text));
        up.set_whats_new_open(o.whats_new_open);
        up.set_whats_new_title(ss(o.whats_new_title));
        up.set_whats_new_highlights(strs(o.whats_new_highlights));
        let sections: Vec<ReleaseSectionData> =
            o.whats_new_sections.into_iter().map(|(t, items)| ReleaseSectionData { title: ss(t), items: strs(items) }).collect();
        up.set_whats_new_sections(ModelRc::new(VecModel::from(sections)));
        w.global::<Tr>().set_rev(vm.lang_rev());
        let s = w.global::<S>();
        s.set_rev(vm.rev());
        s.set_changed_only(vm.show_changed_only());
        s.set_highlight(ss(vm.highlight()));

        let data = w.global::<KoyWindowData>();
        data.set_page(vm.selected_page());
        let subs: Vec<i32> = [0, 1, 2, 4].iter().map(|p| vm.sub_tab(*p)).collect();
        data.set_sub_tabs(ModelRc::new(VecModel::from(subs)));
        let results: Vec<SearchResultData> =
            vm.search_results().into_iter().map(|r| SearchResultData { title: ss(r.title), page: ss(r.page) }).collect();
        data.set_search_results(ModelRc::new(VecModel::from(results)));
        let (pct, level) = vm.memory();
        data.set_memory_pct(pct as f32);
        data.set_memory_level(level);

        let sl = w.global::<SL>();
        let toggles = |rows: Vec<(String, bool)>| -> ModelRc<ToggleItem> {
            ModelRc::new(VecModel::from(rows.into_iter().map(|(t, on)| ToggleItem { title: ss(t), on }).collect::<Vec<_>>()))
        };
        sl.set_tray_apps(toggles(vm.tray_rows().into_iter().map(|r| (r.exe_name, r.is_shown)).collect()));
        sl.set_search_sources(toggles(vm.search_sources().into_iter().map(|r| (r.name, r.is_enabled)).collect()));
        let order = |rows: Vec<(String, bool)>| -> ModelRc<OrderItem> {
            let n = rows.len();
            ModelRc::new(VecModel::from(
                rows.into_iter()
                    .enumerate()
                    .map(|(i, (t, on))| OrderItem { title: ss(t), on, can_up: i > 0, can_down: i + 1 < n })
                    .collect::<Vec<_>>(),
            ))
        };
        sl.set_home_cards(order(vm.home_cards().into_iter().map(|r| (r.label, r.is_enabled)).collect()));
        sl.set_tab_order(order(vm.tab_order().into_iter().map(|r| (r.label, true)).collect()));

        // Uygula / geri al geri sayımı: zamanlayıcı yalnız bekleyen değişiklik varken çalışır.
        if vm.is_apply_revert_pending() {
            if !self.revert_timer.running() {
                let vm = Rc::downgrade(vm);
                self.revert_timer.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || {
                    if let Some(vm) = vm.upgrade() {
                        vm.apply_revert_tick();
                    }
                });
            }
        } else {
            self.revert_timer.stop();
        }
    }

    /// Pencereyi gösterir; verilen sayfa (ve alt sekme) seçilir. Ayarlar diskten yeniden okunur, izleyici başlar.
    pub fn show(&self, page: Option<i32>) {
        self.vm.load_from_settings();
        self.shortcuts.load();
        self.weather.load();
        self.calendar.load();
        if let Some(p) = page {
            self.vm.select_page(p);
        }
        let _ = self.window.show();
        self.window.window().set_minimized(false);
        self.update_page_activation();
        if self.watcher.borrow().is_none() {
            let folder = self.vm.store().path().parent().map(|p| p.to_path_buf()).unwrap_or_default();
            *self.watcher.borrow_mut() = Some(UserStyleWatcher::watch_folder(folder, on_settings_file_changed));
        }
        let weak = self.window.as_weak();
        // Yerel pencere olay döngüsü dönünce oluşur; öne getirme biraz sonra.
        slint::Timer::single_shot(Duration::from_millis(30), move || {
            if let Some(w) = weak.upgrade() {
                let hwnd = native_window::hwnd_of(w.window());
                if hwnd != 0 {
                    native_window::activate(hwnd);
                }
            }
        });
    }

    /// Ekranlar sayfası yalnız görünürken sistem olaylarını dinler.
    fn update_page_activation(&self) {
        self.extensions.set_active(self.vm.selected_page() == super::settings_view_model::EXTENSIONS_PAGE_INDEX && self.is_visible());
        self.advanced.rebuild(self.vm.selected_page() == super::settings_view_model::ADVANCED_PAGE_INDEX);
        if self.vm.selected_page() == 3 && self.is_visible() {
            self.displays.activate();
        } else {
            self.displays.deactivate();
        }
    }

    /// Gizler (kapatma düğmesi ve Alt+F4); izleyici bırakılır.
    pub fn hide(&self) {
        self.extensions.set_active(false);
        self.displays.deactivate();
        if let Some(w) = self.watcher.borrow_mut().take() {
            w.dispose();
        }
        let _ = self.window.hide();
    }

    pub fn is_visible(&self) -> bool {
        self.window.window().is_visible()
    }
}

fn on_settings_file_changed() {
    let host = CURRENT.with(|c| c.borrow().as_ref().and_then(std::rc::Weak::upgrade));
    if let Some(h) = host {
        h.vm.reload_from_disk_if_changed();
    }
}

/// Başlık çubuğundan sürükleme: sistem taşıma döngüsü (WM_NCLBUTTONDOWN + HTCAPTION).
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

fn hex_color(hex: &str) -> slint::Color {
    let h = hex.trim_start_matches('#');
    let v = u32::from_str_radix(h, 16).unwrap_or(0x333333);
    if h.len() == 8 { slint::Color::from_argb_encoded(v) } else { slint::Color::from_argb_encoded(0xFF00_0000 | v) }
}
