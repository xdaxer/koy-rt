use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use super::dock_click_policy::{DockClickKind, DockClickPolicy};
use super::dock_enums::{DockGroupingMode, DockLeftClickAction};
use super::dock_item::{DockItem, DockWindowEntry};
use super::i_pinned_apps_service::IPinnedAppsService;
use super::known_app_user_model_ids::KnownAppUserModelIds;
use super::pinned_app_matcher::PinnedAppMatcher;
use super::pinned_order::PinnedOrder;
use super::refresh_debouncer::{PostedWork, RefreshDebouncer};
use crate::event::{Event, Subscription};
use crate::shell::{IFullscreenDetector, IShellActionService, IWindowTracker, OpenWindowInfo, WindowInfo};
use crate::text;
use crate::time::{TimeProvider, TimeSpan};
use crate::win_path;

struct State {
    flashing_handles: HashSet<isize>,
    stable_unpinned_order: Vec<String>,
    last_foreground_handle: isize,
    items: Vec<DockItem>,
    is_hidden_by_fullscreen: bool,
    left_click_action: DockLeftClickAction,
    grouping_mode: DockGroupingMode,
    refresh_on_foreground_move: bool,
}

struct Inner {
    pinned_apps_service: Arc<dyn IPinnedAppsService>,
    window_tracker: Arc<dyn IWindowTracker>,
    shell_action_service: Arc<dyn IShellActionService>,
    fullscreen_detector: Option<Arc<dyn IFullscreenDetector>>,
    refresh_debouncer: Option<RefreshDebouncer>,
    state: Mutex<State>,
    disposed: AtomicBool,
    items_changed: Event<()>,
    fullscreen_state_changed: Event<bool>,
}

/// Sabitlenmiş uygulamalar ve açık pencereleri birleştirip Dock öğelerini yöneten saf mantık denetleyicisi.
/// Hiçbir Windows veya WPF tipi içermez; birim testlerle tamamen sınanabilir.
pub struct DockController {
    inner: Arc<Inner>,
    subscriptions: Mutex<Option<(Subscription, Subscription, Subscription)>>,
}

fn is_null_or_empty(s: &Option<String>) -> bool {
    s.as_deref().is_none_or(str::is_empty)
}

impl DockController {
    /// `refresh_debounce`: verilir ve sıfırdan büyükse pencere olay fırtınası ertelenir; verilmezse yenileme eşzamanlıdır
    /// (testler ve basit kullanım). `post`: ertelenmiş eylemi çalıştıran (ör. UI dispatcher'ı) geri çağrı.
    pub fn new(
        pinned_apps_service: Arc<dyn IPinnedAppsService>,
        window_tracker: Arc<dyn IWindowTracker>,
        shell_action_service: Arc<dyn IShellActionService>,
        fullscreen_detector: Option<Arc<dyn IFullscreenDetector>>,
        refresh_debounce: Option<TimeSpan>,
        post: Option<Arc<dyn Fn(PostedWork) + Send + Sync>>,
        time_provider: Option<Arc<dyn TimeProvider>>,
    ) -> Self {
        let inner = Arc::new_cyclic(|weak: &Weak<Inner>| {
            // Pencere olay fırtınasında (aç/kapa/göster/gizle) her olayda EnumWindows + simge çıkarımı yapılmasın.
            // Verilmezse yenileme eşzamanlıdır (testler ve basit kullanım).
            let refresh_debouncer = match refresh_debounce {
                Some(delay) if delay > TimeSpan::ZERO => {
                    let w = weak.clone();
                    Some(RefreshDebouncer::new(
                        Arc::new(move || {
                            if let Some(i) = w.upgrade() {
                                i.refresh();
                            }
                        }),
                        delay,
                        post,
                        time_provider,
                    ))
                }
                _ => None,
            };

            Inner {
                pinned_apps_service,
                window_tracker,
                shell_action_service,
                fullscreen_detector,
                refresh_debouncer,
                state: Mutex::new(State {
                    flashing_handles: HashSet::new(),
                    stable_unpinned_order: Vec::new(),
                    last_foreground_handle: 0,
                    items: Vec::new(),
                    is_hidden_by_fullscreen: false,
                    left_click_action: DockLeftClickAction::MinimizeIfActive,
                    grouping_mode: DockGroupingMode::Always,
                    refresh_on_foreground_move: false,
                }),
                disposed: AtomicBool::new(false),
                items_changed: Event::new(),
                fullscreen_state_changed: Event::new(),
            }
        });

        let w1 = Arc::downgrade(&inner);
        let s1 = inner.window_tracker.foreground_window_changed().subscribe(move |e| {
            if let Some(i) = w1.upgrade() {
                i.on_foreground_window_changed(e);
            }
        });
        let w2 = Arc::downgrade(&inner);
        let s2 = inner.window_tracker.windows_changed().subscribe(move |_| {
            if let Some(i) = w2.upgrade() {
                i.request_refresh();
            }
        });
        let w3 = Arc::downgrade(&inner);
        let s3 = inner.window_tracker.window_flashing().subscribe(move |handle| {
            if let Some(i) = w3.upgrade() {
                i.on_window_flashing(*handle);
            }
        });

        DockController { inner, subscriptions: Mutex::new(Some((s1, s2, s3))) }
    }

    /// `ItemsChanged` olayı.
    pub fn items_changed(&self) -> &Event<()> {
        &self.inner.items_changed
    }

    /// `FullscreenStateChanged` olayı (yeni durum).
    pub fn fullscreen_state_changed(&self) -> &Event<bool> {
        &self.inner.fullscreen_state_changed
    }

    pub fn items(&self) -> Vec<DockItem> {
        self.inner.state.lock().unwrap().items.clone()
    }

    pub fn is_hidden_by_fullscreen(&self) -> bool {
        self.inner.state.lock().unwrap().is_hidden_by_fullscreen
    }

    /// Çalışan simgeye sol tık eylemi. Varsayılan bugünkü davranış: etkinse küçült, değilse öne getir.
    pub fn left_click_action(&self) -> DockLeftClickAction {
        self.inner.state.lock().unwrap().left_click_action
    }

    pub fn set_left_click_action(&self, value: DockLeftClickAction) {
        self.inner.state.lock().unwrap().left_click_action = value;
    }

    /// Pencere gruplama kipi. Varsayılan `DockGroupingMode::Always` (bugünkü dock davranışı: aynı uygulamanın pencereleri tek simgede).
    pub fn grouping_mode(&self) -> DockGroupingMode {
        self.inner.state.lock().unwrap().grouping_mode
    }

    pub fn set_grouping_mode(&self, value: DockGroupingMode) {
        self.inner.state.lock().unwrap().grouping_mode = value;
    }

    /// Açıksa ön plan pencere taşınınca/boyutlanınca da liste yenilenir (monitör süzgeci pencerenin hangi ekranda olduğuna bakar).
    /// Yeni olay kancası yok: `IWindowTracker::foreground_window_changed` sınırlar değişince zaten tetiklenir.
    pub fn refresh_on_foreground_move(&self) -> bool {
        self.inner.state.lock().unwrap().refresh_on_foreground_move
    }

    pub fn set_refresh_on_foreground_move(&self, value: bool) {
        self.inner.state.lock().unwrap().refresh_on_foreground_move = value;
    }

    /// Şu an ön plandaki pencere (0 = yok).
    pub fn foreground_handle(&self) -> isize {
        self.inner.window_tracker.current_foreground().map_or(0, |w| w.handle)
    }

    pub fn start(&self) {
        self.inner.refresh();
    }

    pub fn refresh(&self) {
        self.inner.refresh();
    }

    /// Sabitli uygulamaların sırasını sürükleyerek değiştirebilir miyiz (kaynak sıralamayı destekliyor mu).
    pub fn can_reorder_pinned(&self) -> bool {
        self.inner.pinned_apps_service.as_reorderable().is_some()
    }

    /// Sabitli uygulamayı yeni dizine (yalnız sabitliler arasında sayılan dizin) taşır, sırayı kalıcı yapar ve listeyi yeniler.
    pub fn move_pinned(&self, item_id: &str, new_pinned_index: i32) {
        let Some(reorderable) = self.inner.pinned_apps_service.as_reorderable() else {
            return;
        };

        let id = {
            let st = self.inner.state.lock().unwrap();
            match st.items.iter().find(|i| i.is_pinned && text::eq_ignore_case(&i.id, item_id)) {
                Some(item) => item.id.clone(),
                None => return,
            }
        };

        reorderable.move_to(&PinnedOrder::id_of(&id), new_pinned_index);
        self.inner.refresh();
    }

    pub fn handle_item_click(&self, item: &DockItem) {
        if !item.is_running {
            let target = Self::launch_target(item);
            // C#'ta başlatma hatası istisna olarak yükselir; Rust'ta `Err` yok sayılır.
            let _ = self.inner.shell_action_service.launch(&target, None);
            return;
        }

        if item.window_handles.is_empty() {
            return;
        }

        let left_click_action;
        {
            let mut st = self.inner.state.lock().unwrap();
            for h in &item.window_handles {
                st.flashing_handles.remove(h);
            }
            left_click_action = st.left_click_action;
        }

        let foreground = self.inner.window_tracker.current_foreground().map_or(0, |w| w.handle);
        let mut plan = DockClickPolicy::decide_left_click(left_click_action, &item.window_handles, foreground, item.is_active);
        if plan.kind == DockClickKind::ShowThumbnails {
            // Önizlemeyi arayüz açar (DockItemsStrip); buraya düştüyse açılamamıştır: varsayılan davranış.
            plan = DockClickPolicy::decide_left_click(DockLeftClickAction::MinimizeIfActive, &item.window_handles, foreground, item.is_active);
        }

        match plan.kind {
            DockClickKind::Minimize => self.inner.window_tracker.minimize_window(plan.handle),
            DockClickKind::Activate => self.inner.window_tracker.activate_window(plan.handle),
            _ => {}
        }
    }

    pub fn activate_window(&self, handle: isize) {
        if handle != 0 {
            self.inner.state.lock().unwrap().flashing_handles.remove(&handle);
            self.inner.window_tracker.activate_window(handle);
        }
    }

    pub fn close_window(&self, handle: isize) {
        if handle != 0 {
            self.inner.state.lock().unwrap().flashing_handles.remove(&handle);
            self.inner.window_tracker.close_window(handle);
        }
    }

    pub fn handle_item_middle_click(&self, item: &DockItem) {
        let target = Self::launch_target(item);
        let _ = self.inner.shell_action_service.launch(&target, None);
    }

    pub fn handle_item_close(&self, item: &DockItem) {
        for handle in &item.window_handles {
            self.inner.window_tracker.close_window(*handle);
        }
    }

    pub fn handle_item_terminate(&self, item: &DockItem) {
        if item.process_id > 0 {
            self.inner.window_tracker.terminate_process(item.process_id);
        }
    }

    pub fn check_fullscreen(&self) {
        self.inner.check_fullscreen();
    }

    pub fn dispose(&self) {
        if self.inner.disposed.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Some(d) = &self.inner.refresh_debouncer {
            d.dispose();
        }

        if let Some((s1, s2, s3)) = self.subscriptions.lock().unwrap().take() {
            self.inner.window_tracker.foreground_window_changed().unsubscribe(s1);
            self.inner.window_tracker.windows_changed().unsubscribe(s2);
            self.inner.window_tracker.window_flashing().unsubscribe(s3);
        }
    }

    // !string.IsNullOrEmpty(TargetPath) ? TargetPath : FilePath ?? Id
    fn launch_target(item: &DockItem) -> String {
        match item.target_path.as_deref() {
            Some(t) if !t.is_empty() => t.to_string(),
            _ => item.file_path.clone().unwrap_or_else(|| item.id.clone()),
        }
    }
}

impl Drop for DockController {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl Inner {
    fn is_disposed(&self) -> bool {
        self.disposed.load(Ordering::SeqCst)
    }

    fn refresh(&self) {
        if self.is_disposed() {
            return;
        }

        // Tam yenileme yapılıyor; bekleyen ertelenmiş istek gereksiz kalır.
        if let Some(d) = &self.refresh_debouncer {
            d.cancel();
        }

        self.check_fullscreen();

        let pinned_apps = self.pinned_apps_service.get_pinned_apps();
        let open_windows = self.window_tracker.get_open_windows();
        let current_foreground = self.window_tracker.current_foreground();
        let fg_handle = current_foreground.map(|w| w.handle);
        let is_fg = |h: isize| fg_handle == Some(h);

        {
            let mut st = self.state.lock().unwrap();

            let mut remaining_windows: Vec<OpenWindowInfo> = open_windows;
            let mut new_items: Vec<DockItem> = Vec::new();

            // 1. Sabitli uygulamaları işle ve eşleşen pencereleri bul
            for pin in &pinned_apps {
                let (matching_windows, rest): (Vec<OpenWindowInfo>, Vec<OpenWindowInfo>) =
                    remaining_windows.into_iter().partition(|w| PinnedAppMatcher::is_match(pin, w));
                remaining_windows = rest;

                let is_running = !matching_windows.is_empty();
                let is_active = matching_windows.iter().any(|w| is_fg(w.handle));
                let handles: Vec<isize> = matching_windows.iter().map(|w| w.handle).collect();
                let process_id = matching_windows.first().map_or(0, |w| w.process_id);

                if is_active {
                    for h in &handles {
                        st.flashing_handles.remove(h);
                    }
                }

                let is_flashing = handles.iter().any(|h| st.flashing_handles.contains(h));

                let window_entries: Vec<DockWindowEntry> =
                    matching_windows.iter().map(|w| DockWindowEntry::new(w.handle, w.title.clone(), w.bounds)).collect();

                new_items.push(DockItem {
                    id: pin.shortcut_path.clone(),
                    title: pin.title.clone(),
                    file_path: Some(pin.shortcut_path.clone()),
                    target_path: pin.target_path.clone(),
                    icon_data: pin.icon_data.clone(),
                    is_pinned: true,
                    is_running,
                    is_active,
                    window_handles: handles,
                    process_id,
                    is_flashing,
                    badge_count: 0,
                    windows: Some(window_entries),
                    // Çalışan pencerenin kimliği öncelikli (ör. Chrome profili); yoksa sabitli kısayolun AUMID'i.
                    app_user_model_id: Self::resolve_app_id(&matching_windows).or_else(|| pin.app_user_model_id.clone()),
                });
            }

            // 2. Sabitlenmemiş açık pencereleri grupla ve ilk görülme sırasını koru (Alt+Tab zıplamasını engelle)
            let should_group = match st.grouping_mode {
                DockGroupingMode::Always => true,
                DockGroupingMode::WhenSpaceRunsOut => (new_items.len() + remaining_windows.len()) > 16,
                DockGroupingMode::Never => false,
            };

            // GroupBy: varsayılan (Ordinal, büyük/küçük harf duyarlı) karşılaştırıcı; gruplar ilk görülme sırasıyla.
            let mut unpinned_groups: Vec<(String, Vec<OpenWindowInfo>)> = Vec::new();
            for w in remaining_windows {
                let key = if should_group {
                    match w.process_path.as_deref() {
                        Some(p) if !p.is_empty() => p.to_string(),
                        _ => w.process_id.to_string(),
                    }
                } else {
                    format!("{}:{}", w.process_path.clone().unwrap_or_else(|| w.process_id.to_string()), w.handle)
                };
                match unpinned_groups.iter_mut().find(|(k, _)| *k == key) {
                    Some((_, list)) => list.push(w),
                    None => unpinned_groups.push((key, vec![w])),
                }
            }

            st.stable_unpinned_order
                .retain(|k| unpinned_groups.iter().any(|(gk, _)| text::eq_ignore_case(gk, k)));

            for (key, _) in &unpinned_groups {
                if !st.stable_unpinned_order.iter().any(|k| text::eq_ignore_case(k, key)) {
                    st.stable_unpinned_order.push(key.clone());
                }
            }

            let order_index = |order: &[String], key: &str| order.iter().position(|k| text::eq_ignore_case(k, key)).map_or(-1, |i| i as i32);
            let stable = st.stable_unpinned_order.clone();
            unpinned_groups.sort_by(|a, b| order_index(&stable, &a.0).cmp(&order_index(&stable, &b.0)));

            for (key, list) in unpinned_groups {
                let first = &list[0];
                let is_active = list.iter().any(|w| is_fg(w.handle));
                let handles: Vec<isize> = list.iter().map(|w| w.handle).collect();

                if is_active {
                    for h in &handles {
                        st.flashing_handles.remove(h);
                    }
                }

                let is_flashing = handles.iter().any(|h| st.flashing_handles.contains(h));

                let title = if !text::is_null_or_white_space(Some(&first.title)) {
                    first.title.clone()
                } else {
                    match first.process_path.as_deref() {
                        Some(p) => win_path::get_file_name_without_extension(p).to_string(),
                        None => "App".to_string(),
                    }
                };

                let window_entries: Vec<DockWindowEntry> = list.iter().map(|w| DockWindowEntry::new(w.handle, w.title.clone(), w.bounds)).collect();

                new_items.push(DockItem {
                    id: key,
                    title,
                    file_path: first.process_path.clone(),
                    target_path: first.process_path.clone(),
                    icon_data: first.icon_data.clone(),
                    is_pinned: false,
                    is_running: true,
                    is_active,
                    window_handles: handles,
                    process_id: first.process_id,
                    is_flashing,
                    badge_count: 0,
                    windows: Some(window_entries),
                    app_user_model_id: Self::resolve_app_id(&list),
                });
            }

            st.items = new_items;
        }

        self.items_changed.invoke(&());
    }

    /// Pencerelerden ilk AppUserModelID (pencerede yoksa bilinen sistem uygulamaları tablosundan). Atlama listesi bu kimliğe
    /// bağlıdır; kimlik yoksa `None` (exe yolundan kimlik türetilmez).
    fn resolve_app_id(windows: &[OpenWindowInfo]) -> Option<String> {
        for w in windows {
            let id = if !is_null_or_empty(&w.app_user_model_id) {
                w.app_user_model_id.clone()
            } else {
                KnownAppUserModelIds::for_window(w.process_path.as_deref(), w.class_name.as_deref())
            };
            if !is_null_or_empty(&id) {
                return id;
            }
        }

        None
    }

    fn check_fullscreen(&self) {
        let Some(detector) = &self.fullscreen_detector else { return };

        let is_fullscreen = detector.is_fullscreen();
        let changed = {
            let mut st = self.state.lock().unwrap();
            if st.is_hidden_by_fullscreen != is_fullscreen {
                st.is_hidden_by_fullscreen = is_fullscreen;
                true
            } else {
                false
            }
        };
        if changed {
            self.fullscreen_state_changed.invoke(&is_fullscreen);
        }
    }

    fn request_refresh(&self) {
        match &self.refresh_debouncer {
            None => self.refresh(),
            Some(d) => d.request(),
        }
    }

    fn on_window_flashing(&self, handle: isize) {
        self.state.lock().unwrap().flashing_handles.insert(handle);
        self.request_refresh();
    }

    fn on_foreground_window_changed(&self, e: &WindowInfo) {
        let (changed, on_move) = {
            let mut st = self.state.lock().unwrap();
            if e.handle != st.last_foreground_handle {
                st.last_foreground_handle = e.handle;
                (true, false)
            } else {
                (false, st.refresh_on_foreground_move)
            }
        };

        if changed {
            self.refresh();
        } else if on_move {
            // Aynı pencere, değişen sınırlar: taşındı ya da boyutlandı. Sürükleme fırtınası ertelemeyle birleşir.
            self.request_refresh();
        }
    }
}
