use std::sync::{Arc, Mutex, MutexGuard};

use super::{NotchOpenPolicy, NotchPanel, NotchPanels, NotchState, NotchStateTransitions, NotchTrigger};
use crate::event::Event;
use crate::time::{SystemTimeProvider, TimeProvider, TimeSpan, Timer};

/// Otomatik ipucu (Peek) süresi: parça değişince çentik bu kadar açık kalır.
const AUTO_PEEK_DURATION: TimeSpan = TimeSpan::from_secs(3);

struct Inner {
    enabled_panels: Vec<NotchPanel>,
    state: NotchState,
    current_panel: NotchPanel,
    is_mouse_over: bool,
    open_on_hover: bool,
    open_delay_ms: i32,
    close_delay_ms: i32,
    // Sekme sırası (kullanıcının sırası; boşsa varsayılan). Geçici eklenen panel (sürükle-bırakta Raf) bu sıraya göre yerleşir.
    panel_order: Vec<NotchPanel>,
    // Geçici tutma: dosya seçici gibi çentiğin dışında açılan pencereler için. Kullanıcının iğnesinden (is_pinned)
    // ayrıdır: iğne simgesi yanmaz, kullanıcı seçimi bozulmaz. Sayaçlı: iç içe tutmalar birbirini açmaz.
    holds: i32,
    is_pinned: bool,
    disposed: bool,
    auto_peek_timer: Option<Box<dyn Timer>>,
    mouse_leave_timer: Option<Box<dyn Timer>>,
    hover_open_timer: Option<Box<dyn Timer>>,
    hover_open_suppressed: bool,
    suppressed_until_mouse_leave: bool,
}

/// Zamanlayıcı iş parçacıklarıyla paylaşılan çekirdek (`Arc` + `Weak` kalıbı, PORTING.md #10).
/// Olaylar kilit dışında tetiklenir: işleyici denetleyiciyi yeniden sorgulayabilir.
struct Shared {
    inner: Mutex<Inner>,
    time_provider: Arc<dyn TimeProvider>,
    state_changed: Event<NotchState>,
    panel_changed: Event<NotchPanel>,
    panel_transition_triggered: Event<i32>,
    enabled_panels_changed: Event<()>,
    pinned_changed: Event<()>,
}

fn cancel(timer: &mut Option<Box<dyn Timer>>) {
    if let Some(t) = timer.as_mut() {
        t.change(TimeSpan::INFINITE, TimeSpan::INFINITE);
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap()
    }

    fn cancel_auto_peek(&self) {
        cancel(&mut self.lock().auto_peek_timer);
    }

    fn set_enabled_panels(&self, panels: &[NotchPanel]) {
        {
            let mut g = self.lock();
            g.enabled_panels.clear();
            g.enabled_panels.extend_from_slice(panels);
            if g.enabled_panels.is_empty() {
                g.enabled_panels.push(NotchPanel::Home);
            }
        }

        self.enabled_panels_changed.invoke(&());

        let first = {
            let g = self.lock();
            if g.enabled_panels.contains(&g.current_panel) { None } else { Some(g.enabled_panels[0]) }
        };
        if let Some(first) = first {
            self.select_panel(first);
        }
    }

    fn next_panel(&self) {
        let target = {
            let g = self.lock();
            if g.enabled_panels.len() <= 1 {
                return;
            }
            let n = g.enabled_panels.len() as i32;
            let current = g.enabled_panels.iter().position(|p| *p == g.current_panel).map_or(-1, |i| i as i32);
            g.enabled_panels[((current + 1) % n) as usize]
        };
        self.panel_transition_triggered.invoke(&1);
        self.select_panel(target);
    }

    fn previous_panel(&self) {
        let target = {
            let g = self.lock();
            if g.enabled_panels.len() <= 1 {
                return;
            }
            let n = g.enabled_panels.len() as i32;
            let current = g.enabled_panels.iter().position(|p| *p == g.current_panel).map_or(-1, |i| i as i32);
            g.enabled_panels[((current - 1 + n) % n) as usize]
        };
        self.panel_transition_triggered.invoke(&-1);
        self.select_panel(target);
    }

    fn go_to_panel(&self, panel: NotchPanel) {
        let direction = {
            let g = self.lock();
            if !g.enabled_panels.contains(&panel) || g.current_panel == panel {
                return;
            }
            let idx = |p: NotchPanel| g.enabled_panels.iter().position(|x| *x == p).map_or(-1, |i| i as i32);
            if idx(panel) > idx(g.current_panel) { 1 } else { -1 }
        };
        self.panel_transition_triggered.invoke(&direction);
        self.select_panel(panel);
    }

    fn select_panel(&self, panel: NotchPanel) {
        {
            let mut g = self.lock();
            if !g.enabled_panels.contains(&panel) || g.current_panel == panel {
                return;
            }
            g.current_panel = panel;
        }
        self.panel_changed.invoke(&panel);
    }

    fn expand(self: &Arc<Self>, panel: Option<NotchPanel>) {
        {
            let mut g = self.lock();
            cancel(&mut g.auto_peek_timer);
            cancel(&mut g.mouse_leave_timer);
        }
        self.set_state(NotchState::Expanded);
        if let Some(panel) = panel {
            let added = {
                let mut g = self.lock();
                if !g.enabled_panels.contains(&panel) {
                    // Ayarlarda kapalı panel (ör. sürükle-bırakta Raf) geçici olarak listeye girer; sekme çubuğu da bilsin.
                    g.enabled_panels.push(panel);
                    let order = g.panel_order.clone();
                    let index_of = |p: &NotchPanel| order.iter().position(|x| x == p).map_or(-1, |i| i as i32);
                    g.enabled_panels.sort_by_key(index_of);
                    true
                } else {
                    false
                }
            };
            if added {
                self.enabled_panels_changed.invoke(&());
            }
            self.select_panel(panel);
        }
    }

    fn on_mouse_enter(self: &Arc<Self>) {
        let (open_on_hover, state, delay) = {
            let mut g = self.lock();
            g.is_mouse_over = true;
            cancel(&mut g.mouse_leave_timer);
            if g.hover_open_suppressed || g.suppressed_until_mouse_leave {
                g.hover_open_timer = None;
                return;
            }
            (g.open_on_hover, g.state, g.open_delay_ms)
        };

        if open_on_hover && state != NotchState::Expanded && delay == 0 {
            self.cancel_auto_peek();
            self.set_state(NotchState::Expanded);
        } else if state == NotchState::Collapsed {
            self.set_state(NotchStateTransitions::transition(state, NotchTrigger::MouseEnter));
        }

        // Gecikmeli açılma: imleç süre boyunca üstünde kalırsa açılır; çıkarsa zamanlayıcı bırakılır.
        let mut g = self.lock();
        if g.open_on_hover && g.state != NotchState::Expanded && g.open_delay_ms > 0 {
            g.hover_open_timer = None;
            let weak = Arc::downgrade(self);
            let due = TimeSpan::from_millis(g.open_delay_ms as i64);
            g.hover_open_timer = Some(self.time_provider.create_timer(
                Box::new(move || {
                    if let Some(shared) = weak.upgrade() {
                        shared.on_hover_open_timeout();
                    }
                }),
                due,
                TimeSpan::INFINITE,
            ));
        }
    }

    fn on_hover_open_timeout(self: &Arc<Self>) {
        let open = {
            let mut g = self.lock();
            g.hover_open_timer = None;
            g.is_mouse_over
                && g.open_on_hover
                && !(g.hover_open_suppressed || g.suppressed_until_mouse_leave)
                && g.state != NotchState::Expanded
                && !g.disposed
        };
        if open {
            self.cancel_auto_peek();
            self.set_state(NotchState::Expanded);
        }
    }

    fn on_mouse_leave(self: &Arc<Self>) {
        let (state, pinned, held) = {
            let mut g = self.lock();
            g.is_mouse_over = false;
            g.suppressed_until_mouse_leave = false;
            g.hover_open_timer = None;
            (g.state, g.is_pinned, g.holds > 0)
        };

        if state == NotchState::Peek {
            self.cancel_auto_peek();
            self.set_state(NotchStateTransitions::transition(state, NotchTrigger::MouseLeave));
        } else if state == NotchState::Expanded && !pinned && !held {
            self.start_mouse_leave_timer();
        }
    }

    fn start_mouse_leave_timer(self: &Arc<Self>) {
        let mut g = self.lock();
        g.mouse_leave_timer = None;
        let weak = Arc::downgrade(self);
        let due = TimeSpan::from_millis(g.close_delay_ms as i64);
        g.mouse_leave_timer = Some(self.time_provider.create_timer(
            Box::new(move || {
                if let Some(shared) = weak.upgrade() {
                    shared.on_mouse_leave_timeout();
                }
            }),
            due,
            TimeSpan::INFINITE,
        ));
    }

    fn hold_open(&self) {
        let mut g = self.lock();
        cancel(&mut g.mouse_leave_timer);
        g.holds += 1;
    }

    fn release_hold(self: &Arc<Self>) {
        let start = {
            let mut g = self.lock();
            if g.holds == 0 {
                return;
            }
            g.holds -= 1;
            g.holds == 0 && g.state == NotchState::Expanded && !g.is_mouse_over && !g.is_pinned
        };
        if start {
            self.start_mouse_leave_timer();
        }
    }

    fn set_pinned(self: &Arc<Self>, pinned: bool) {
        {
            let mut g = self.lock();
            // Yalnız açık çentik tutulabilir; kapalıyken sabitlemenin anlamı yok.
            let pinned = pinned && g.state == NotchState::Expanded;
            if g.is_pinned == pinned {
                return;
            }
            g.is_pinned = pinned;
        }
        self.pinned_changed.invoke(&());

        // Sabit kalkarken fare zaten dışarıdaysa her zamanki gecikmeyle kapansın.
        let start = {
            let g = self.lock();
            !g.is_pinned && g.state == NotchState::Expanded && !g.is_mouse_over
        };
        if start {
            self.start_mouse_leave_timer();
        }
    }

    fn on_click(self: &Arc<Self>) {
        let state = {
            let mut g = self.lock();
            // Seçici açıkken çentiğe tıklamak onu kapatıp seçiciyi yetim bırakmasın.
            if g.state == NotchState::Expanded && g.holds > 0 {
                return;
            }
            cancel(&mut g.auto_peek_timer);
            cancel(&mut g.mouse_leave_timer);
            g.state
        };
        self.set_state(NotchStateTransitions::transition(state, NotchTrigger::Click));
    }

    fn on_track_changed(self: &Arc<Self>) {
        let (state, over) = {
            let g = self.lock();
            (g.state, g.is_mouse_over)
        };
        if state == NotchState::Collapsed {
            self.set_state(NotchStateTransitions::transition(state, NotchTrigger::TrackChanged));
            self.reset_auto_peek_timer();
        } else if state == NotchState::Peek && !over {
            self.reset_auto_peek_timer();
        }
    }

    fn reset_auto_peek_timer(self: &Arc<Self>) {
        let mut g = self.lock();
        g.auto_peek_timer = None;
        let weak = Arc::downgrade(self);
        g.auto_peek_timer = Some(self.time_provider.create_timer(
            Box::new(move || {
                if let Some(shared) = weak.upgrade() {
                    shared.on_auto_peek_timeout();
                }
            }),
            AUTO_PEEK_DURATION,
            TimeSpan::INFINITE,
        ));
    }

    fn on_auto_peek_timeout(self: &Arc<Self>) {
        let (state, over) = {
            let g = self.lock();
            (g.state, g.is_mouse_over)
        };
        if state == NotchState::Peek && !over {
            self.set_state(NotchStateTransitions::transition(state, NotchTrigger::AutoPeekTimeout));
        }
    }

    fn on_mouse_leave_timeout(self: &Arc<Self>) {
        let state = {
            let g = self.lock();
            if g.state == NotchState::Expanded && !g.is_mouse_over && !g.is_pinned && g.holds == 0 { Some(g.state) } else { None }
        };
        if let Some(state) = state {
            self.set_state(NotchStateTransitions::transition(state, NotchTrigger::MouseLeaveDelayTimeout));
        }
    }

    fn set_state(self: &Arc<Self>, new_state: NotchState) {
        {
            let mut g = self.lock();
            if g.state == new_state {
                return;
            }
            g.state = new_state;
        }
        if new_state != NotchState::Expanded {
            self.set_pinned(false);
        }
        let current = self.lock().state;
        self.state_changed.invoke(&current);
    }

    fn dispose(&self) {
        let mut g = self.lock();
        if g.disposed {
            return;
        }

        g.disposed = true;
        g.auto_peek_timer = None;
        g.mouse_leave_timer = None;
        g.hover_open_timer = None;
    }
}

/// Notch durumunu, panellerini ve zamanlayıcılarını yöneten saf iş mantığı denetleyicisi.
/// C# `IDisposable` karşılığı [`NotchController::dispose`] (ve `Drop`).
///
/// Olaylar kilit dışında tetiklenir; bir yöntem birden çok olay üretiyorsa (ör. `set_enabled_panels`) her olay
/// kendi adımından hemen sonra, C#'taki sırayla çıkar.
pub struct NotchController {
    shared: Arc<Shared>,
}

impl NotchController {
    /// `timeProvider` varsayılanı `TimeProvider.System`.
    pub fn new(time_provider: Option<Arc<dyn TimeProvider>>) -> Self {
        let time_provider = time_provider.unwrap_or_else(|| Arc::new(SystemTimeProvider));
        NotchController {
            shared: Arc::new(Shared {
                inner: Mutex::new(Inner {
                    enabled_panels: vec![NotchPanel::Home],
                    state: NotchState::Collapsed,
                    current_panel: NotchPanel::Home,
                    is_mouse_over: false,
                    open_on_hover: false,
                    open_delay_ms: 0,
                    close_delay_ms: NotchOpenPolicy::DEFAULT_CLOSE_DELAY_MS,
                    panel_order: NotchPanels::order().to_vec(),
                    holds: 0,
                    is_pinned: false,
                    disposed: false,
                    auto_peek_timer: None,
                    mouse_leave_timer: None,
                    hover_open_timer: None,
                    hover_open_suppressed: false,
                    suppressed_until_mouse_leave: false,
                }),
                time_provider,
                state_changed: Event::new(),
                panel_changed: Event::new(),
                panel_transition_triggered: Event::new(),
                enabled_panels_changed: Event::new(),
                pinned_changed: Event::new(),
            }),
        }
    }

    /// `event EventHandler<NotchState> StateChanged`.
    pub fn state_changed(&self) -> &Event<NotchState> {
        &self.shared.state_changed
    }

    /// `event EventHandler<NotchPanel> PanelChanged`.
    pub fn panel_changed(&self) -> &Event<NotchPanel> {
        &self.shared.panel_changed
    }

    /// `event Action<int> PanelTransitionTriggered` (kayma yönü: 1 ya da -1).
    pub fn panel_transition_triggered(&self) -> &Event<i32> {
        &self.shared.panel_transition_triggered
    }

    /// Açık paneller listesi (kümesi ya da sırası) değişti; sekme çubuğu yeniden kurulmalı.
    pub fn enabled_panels_changed(&self) -> &Event<()> {
        &self.shared.enabled_panels_changed
    }

    pub fn pinned_changed(&self) -> &Event<()> {
        &self.shared.pinned_changed
    }

    pub fn state(&self) -> NotchState {
        self.shared.lock().state
    }

    pub fn current_panel(&self) -> NotchPanel {
        self.shared.lock().current_panel
    }

    pub fn is_mouse_over(&self) -> bool {
        self.shared.lock().is_mouse_over
    }

    pub fn enabled_panels(&self) -> Vec<NotchPanel> {
        self.shared.lock().enabled_panels.clone()
    }

    /// Açılma davranışı: true ise fare üzerine gelince çentik doğrudan açılır; false ise önce küçük bir ipucu
    /// (Peek) görünür, tıklayınca açılır (ayar: 0 = üzerine gelince, 1 = tıklayınca).
    pub fn open_on_hover(&self) -> bool {
        self.shared.lock().open_on_hover
    }

    pub fn set_open_on_hover(&self, value: bool) {
        self.shared.lock().open_on_hover = value;
    }

    /// Üzerine gelince açılmadan önce bekleme (ms, 0–2000; issue #78). Beklerken çentik ipucu (Peek) gösterir.
    pub fn open_delay_ms(&self) -> i32 {
        self.shared.lock().open_delay_ms
    }

    pub fn set_open_delay_ms(&self, value: i32) {
        self.shared.lock().open_delay_ms = NotchOpenPolicy::clamp_delay(value);
    }

    /// İmleç çıkınca kapanmadan önce bekleme (ms, 0–2000). Varsayılan bugünkü 400 ms.
    pub fn close_delay_ms(&self) -> i32 {
        self.shared.lock().close_delay_ms
    }

    pub fn set_close_delay_ms(&self, value: i32) {
        self.shared.lock().close_delay_ms = NotchOpenPolicy::clamp_delay(value);
    }

    /// Kullanıcının sekme sırasını kurar ([`NotchPanels::normalize_order`]).
    pub fn set_panel_order(&self, keys: Option<&[String]>) {
        self.shared.lock().panel_order = NotchPanels::normalize_order(keys);
    }

    pub fn set_enabled_panels(&self, panels: &[NotchPanel]) {
        self.shared.set_enabled_panels(panels);
    }

    pub fn next_panel(&self) {
        self.shared.next_panel();
    }

    pub fn previous_panel(&self) {
        self.shared.previous_panel();
    }

    /// Sekmeden doğrudan bir panele geçer; kayma yönü panellerin sırasına göre seçilir (sağdaki sekme sağdan gelir).
    pub fn go_to_panel(&self, panel: NotchPanel) {
        self.shared.go_to_panel(panel);
    }

    pub fn select_panel(&self, panel: NotchPanel) {
        self.shared.select_panel(panel);
    }

    /// Çentiği doğrudan Expanded durumuna geçirir ve isteğe bağlı olarak belirtilen paneli seçer.
    /// Sürükle-bırak veya doğrudan kullanıcı etkileşimi için kullanılır. (`Expand(NotchPanel? panel = null)`)
    pub fn expand(&self, panel: Option<NotchPanel>) {
        self.shared.expand(panel);
    }

    pub fn on_mouse_enter(&self) {
        self.shared.on_mouse_enter();
    }

    /// Üzerine gelince açılmanın (Expanded veya Peek) şu an bastırılıp bastırılmadığını gösterir.
    pub fn is_hover_open_suppressed(&self) -> bool {
        let g = self.shared.lock();
        g.hover_open_suppressed || g.suppressed_until_mouse_leave
    }

    /// OSD/kart görünürken fareyle üzerine gelerek çentiğin açılmasını (Expanded ve Peek) bastırır.
    /// İmleç çentik üzerindeyken bastırma kalkarsa, imleç çıkana (`on_mouse_leave`) kadar bastırma sürer.
    pub fn set_hover_open_suppressed(&self, suppressed: bool) {
        let mut g = self.shared.lock();
        if g.hover_open_suppressed == suppressed {
            return;
        }
        g.hover_open_suppressed = suppressed;
        if suppressed {
            g.suppressed_until_mouse_leave = false;
            g.hover_open_timer = None;
        } else {
            g.suppressed_until_mouse_leave = g.is_mouse_over;
        }
    }

    /// Gecikmeli açılmanın süresi doldu (zamanlayıcı ya da test çağırır).
    pub fn on_hover_open_timeout(&self) {
        self.shared.on_hover_open_timeout();
    }

    pub fn on_mouse_leave(&self) {
        self.shared.on_mouse_leave();
    }

    /// Bir pencere (ör. dosya seçici) açık olduğu için çentik kapanmaz: fare ayrılsa da, çentiğe tıklansa da.
    pub fn is_held(&self) -> bool {
        self.shared.lock().holds > 0
    }

    /// Tutmayı başlatır; her çağrı bir [`NotchController::release_hold`] ile eşlenmelidir.
    pub fn hold_open(&self) {
        self.shared.hold_open();
    }

    /// Tutmayı bırakır. Sonuncusu bırakılınca fare dışarıdaysa her zamanki gecikmeyle kapanma başlar.
    pub fn release_hold(&self) {
        self.shared.release_hold();
    }

    /// "Çentiği açık tut": açıkken fare çıkınca kapanmaz. Çentik kapanınca (tıklamayla) kendiliğinden kalkar.
    pub fn is_pinned(&self) -> bool {
        self.shared.lock().is_pinned
    }

    pub fn toggle_pinned(&self) {
        let pinned = self.is_pinned();
        self.set_pinned(!pinned);
    }

    pub fn set_pinned(&self, pinned: bool) {
        self.shared.set_pinned(pinned);
    }

    pub fn on_click(&self) {
        self.shared.on_click();
    }

    pub fn on_track_changed(&self) {
        self.shared.on_track_changed();
    }

    pub fn on_auto_peek_timeout(&self) {
        self.shared.on_auto_peek_timeout();
    }

    pub fn on_mouse_leave_timeout(&self) {
        self.shared.on_mouse_leave_timeout();
    }

    pub fn dispose(&self) {
        self.shared.dispose();
    }
}

impl Drop for NotchController {
    fn drop(&mut self) {
        self.shared.dispose();
    }
}
