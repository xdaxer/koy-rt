//! Çentik penceresinin yerel kabuğu. Karşılığı: `Features/Notch/NotchWindow.xaml.cs`.
//!
//! Boyut, köşe, dikey kayma, sayfa geçişi ve "yutma" (bırakmada esneme) yaylarla canlandırılır; her karede şekil yolu
//! ve pencere bölgesi yenilenir. Fare girişi / çıkışı, tıklama, tekerlek, sürükle-bırak, klavye (sesli komut onayı)
//! ve etkinleştirme (arama kutusu, yazı alanları) burada görünüm modeline bağlanır.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use slint::ComponentHandle;

use koy_core::animation::Spring;
use koy_core::appearance::MotionKind;
use koy_core::display::{DisplayMonitor, WindowPlacementMath};
use koy_core::notch::{NotchLayout, NotchOpenPolicy, NotchPanel, NotchState, WheelAction};
use koy_core::voice::VoiceState;
use koy_platform::shell::WindowStyles;

use super::notch_shape::{ShapeSpec, apply_region, path_commands};
use super::notch_view_model::NotchViewModel;
use super::osd::osd_indicator_view_model::OsdType;
use crate::NotchWindow;
use crate::infra::native_window;
use crate::infra::ole_dnd::{self, DropData, DropRegistration};
use crate::resources::motion::Motion;

const OPEN_DAMPING: f64 = 26.0;
const GULP_KICK_VELOCITY: f64 = 30.0;
const GULP_STRETCH_X: f64 = 0.025;
const GULP_SQUASH_Y: f64 = 0.05;
const WHEEL_VOLUME_STEP_PERCENT: i32 = 2;
const FRAME: Duration = Duration::from_millis(8);
const WM_DISPLAYCHANGE: u32 = 0x007E;
const WM_LBUTTONDOWN: u32 = 0x0201;

struct Anim {
    width: Spring,
    height: Spring,
    corner: Spring,
    translate_y: Spring,
    panel_x: Spring,
    panel_opacity: Spring,
    gulp: Spring,
    panel_time_scale: f64,
    osd_shape: bool,
    visible: bool,
    last: Option<Instant>,
}

/// Bir ekrandaki çentik penceresi.
pub struct NotchWindowHost {
    pub window: NotchWindow,
    vm: Rc<NotchViewModel>,
    anim: RefCell<Anim>,
    timer: slint::Timer,
    me: RefCell<Weak<Self>>,
    hwnd: Cell<isize>,
    monitor: RefCell<Option<DisplayMonitor>>,
    drag_entered_at: Cell<Option<Instant>>,
    drop: RefCell<Option<DropRegistration>>,
    hooks: RefCell<Vec<usize>>,
    /// Yerel pencere hazır olunca bir kez çağrılır (özelliğin tanıtıcıya ihtiyaç duyan kurulumu).
    pub native_ready: RefCell<Option<Box<dyn FnOnce(isize)>>>,
    subs: RefCell<Vec<(u8, u64)>>,
    /// Ekran düzeni değişti (WM_DISPLAYCHANGE).
    pub display_change_handler: RefCell<Option<Box<dyn Fn()>>>,
    pub voice_confirm_requested: RefCell<Option<Box<dyn Fn()>>>,
    pub voice_cancel_requested: RefCell<Option<Box<dyn Fn()>>>,
    pub voice_reply_requested: RefCell<Option<Box<dyn Fn()>>>,
    live_bg: RefCell<Option<Rc<super::live_background::live_background_renderer::LiveBackgroundRenderer>>>,
}

fn retarget(spring: &mut Spring, target: f64) {
    let damping = if target < spring.value() { spring.critical_damping() } else { OPEN_DAMPING };
    spring.set_damping(damping);
    spring.set_target(target);
}

impl NotchWindowHost {
    pub fn new(vm: Rc<NotchViewModel>, window: NotchWindow) -> Rc<Self> {
        let host = Rc::new(Self {
            window,
            vm: Rc::clone(&vm),
            anim: RefCell::new(Anim {
                width: Spring::new(236.0),
                height: Spring::new(34.0),
                corner: Spring::new(18.0),
                translate_y: Spring::new(0.0),
                panel_x: Spring::new(0.0),
                panel_opacity: Spring::new(1.0),
                gulp: Spring::new(0.0).with_stiffness(260.0).with_damping(14.0),
                panel_time_scale: 1.0,
                osd_shape: false,
                visible: true,
                last: None,
            }),
            timer: slint::Timer::default(),
            me: RefCell::new(Weak::new()),
            hwnd: Cell::new(0),
            monitor: RefCell::new(None),
            drag_entered_at: Cell::new(None),
            drop: RefCell::new(None),
            hooks: RefCell::new(Vec::new()),
            native_ready: RefCell::new(None),
            subs: RefCell::new(Vec::new()),
            live_bg: RefCell::new(None),
            display_change_handler: RefCell::new(None),
            voice_confirm_requested: RefCell::new(None),
            voice_cancel_requested: RefCell::new(None),
            voice_reply_requested: RefCell::new(None),
        });
        *host.me.borrow_mut() = Rc::downgrade(&host);
        host.wire_view_model();
        host.wire_window();
        // İlk durum: hedef boyut ve görünürlük.
        let (w, h) = {
            let st = vm.state();
            (st.target_width, st.target_height)
        };
        host.animate_to(w, h);
        if !vm.state().is_notch_visible {
            let mut a = host.anim.borrow_mut();
            a.visible = false;
            a.translate_y.snap_to(-150.0);
        }
        host.render_frame();
        // Canlı arka plan çizimi (oynatma kararı görünüm modelinde).
        let renderer = super::live_background::live_background_renderer::LiveBackgroundRenderer::attach(
            &host.window,
            Rc::clone(&host.vm.live_background),
        );
        *host.live_bg.borrow_mut() = Some(renderer);
        host
    }

    pub fn hwnd(&self) -> isize {
        self.hwnd.get()
    }

    pub fn target_monitor(&self) -> Option<DisplayMonitor> {
        self.monitor.borrow().clone()
    }

    /// Pencereyi gösterir ve yerel biçimleri uygular (C# `OnSourceInitialized`).
    pub fn show(self: &Rc<Self>) {
        if self.window.show().is_err() {
            return;
        }
        self.attach_native(0);
    }

    /// winit yerel pencereyi olay döngüsü dönmeye başlayınca kurar: tanıtıcı gelene kadar kısa aralıklarla yeniden denenir.
    fn attach_native(self: &Rc<Self>, attempt: u32) {
        let hwnd = native_window::hwnd_of(self.window.window());
        if hwnd == 0 {
            if attempt < 200 {
                let weak = Rc::downgrade(self);
                slint::Timer::single_shot(Duration::from_millis(15), move || {
                    if let Some(h) = weak.upgrade() {
                        h.attach_native(attempt + 1);
                    }
                });
            }
            return;
        }
        self.hwnd.set(hwnd);
        crate::infra::native_window::make_popup(hwnd);
        WindowStyles::apply_tool_window_and_no_activate(hwnd);
        WindowStyles::reassert_topmost(hwnd);
        self.place_at_top_center(None);
        let weak = Rc::downgrade(self);
        let id = native_window::add_hook(
            hwnd,
            Box::new(move |msg, wparam, _lparam| {
                let host = weak.upgrade()?;
                if msg == WM_DISPLAYCHANGE {
                    if let Some(f) = host.display_change_handler.borrow().as_ref() {
                        f();
                    }
                    host.place_at_top_center(None);
                } else if native_window::is_deactivate(msg, wparam) {
                    host.on_deactivated();
                } else if msg == WM_LBUTTONDOWN {
                    host.on_left_button_down();
                }
                None
            }),
        );
        self.hooks.borrow_mut().push(id);
        // Sürükle-bırak: dosya ya da metin gelince çentik Raf'a açılır, bırakılınca Raf'a eklenir.
        let over = Rc::downgrade(self);
        let dropped = Rc::downgrade(self);
        *self.drop.borrow_mut() = ole_dnd::register_drop_with_hover(
            hwnd,
            move |entered| {
                if let Some(h) = over.upgrade() {
                    h.on_drag_over(entered);
                }
            },
            move |data| {
                if let Some(h) = dropped.upgrade() {
                    h.on_drop(data);
                }
            },
        );
        let visible = self.anim.borrow().visible;
        if visible {
            self.animate_visibility(true);
        } else {
            WindowStyles::set_click_through(hwnd, true);
        }
        self.render_frame();
        if let Some(f) = self.native_ready.borrow_mut().take() {
            f(hwnd);
        }
    }

    pub fn set_target_monitor(&self, monitor: Option<DisplayMonitor>) {
        *self.monitor.borrow_mut() = monitor;
        self.place_at_top_center(None);
    }

    /// Pencereyi hedef ekranın üst ortasına yerleştirir (üst boşluk ayardan).
    pub fn place_at_top_center(&self, target: Option<DisplayMonitor>) {
        if let Some(m) = target {
            *self.monitor.borrow_mut() = Some(m);
        }
        let hwnd = self.hwnd.get();
        let top = self.vm.top_offset_dip();
        let monitor = self.monitor.borrow().clone();
        match monitor {
            Some(m) if hwnd != 0 && m.bounds.width() > 100 => {
                let rect = WindowPlacementMath::top_center_with_offset(&m, NotchLayout::CANVAS_WIDTH, NotchLayout::CANVAS_HEIGHT, top);
                let scale = self.window.window().scale_factor() as f64;
                let same_dpi = (scale - m.dpi_scale_x).abs() < 0.01;
                WindowStyles::set_bounds(hwnd, rect, same_dpi);
            }
            _ => {
                use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN};
                // SAFETY: salt okunur sistem ölçüsü.
                let screen = unsafe { GetSystemMetrics(SM_CXSCREEN) } as f32;
                let scale = self.window.window().scale_factor();
                let x = (screen - NotchLayout::CANVAS_WIDTH as f32 * scale) / 2.0;
                self.window.window().set_position(slint::PhysicalPosition::new(x.round() as i32, (top as f32 * scale).round() as i32));
            }
        }
    }

    // ---- Görünüm modeli olayları ----

    fn wire_view_model(self: &Rc<Self>) {
        let vm = &self.vm;
        let w = Rc::downgrade(self);
        let on = |w: &Weak<Self>| w.clone();
        let a = on(&w);
        let id = vm.target_size_changed.subscribe(move |(tw, th)| {
            if let Some(h) = a.upgrade() {
                h.animate_to(*tw, *th);
            }
        });
        self.subs.borrow_mut().push((0, id));
        let a = on(&w);
        let id = vm.target_visibility_changed.subscribe(move |v| {
            if let Some(h) = a.upgrade() {
                h.animate_visibility(*v);
            }
        });
        self.subs.borrow_mut().push((1, id));
        let a = on(&w);
        let id = vm.panel_transition_triggered.subscribe(move |d| {
            if let Some(h) = a.upgrade() {
                h.animate_panel_transition(*d);
            }
        });
        self.subs.borrow_mut().push((2, id));
        let a = on(&w);
        let id = vm.shake_requested.subscribe(move |_| {
            if let Some(h) = a.upgrade() {
                h.on_shake();
            }
        });
        self.subs.borrow_mut().push((3, id));
        let a = on(&w);
        let id = vm.search_closed.subscribe(move |restore| {
            if let Some(h) = a.upgrade() {
                h.on_search_closed(*restore);
            }
        });
        self.subs.borrow_mut().push((4, id));
        // Hızlı başlat iletişim kutusunun sahibi: pencere geçici olarak etkinleştirilebilir olur.
        let a = on(&w);
        *vm.home.quick_launch.resolve_dialog_owner.borrow_mut() = Some(Box::new(move || {
            let h = a.upgrade()?;
            let hwnd = h.hwnd.get();
            if hwnd == 0 {
                return None;
            }
            WindowStyles::set_no_activate(hwnd, false);
            native_window::activate(hwnd);
            Some(hwnd)
        }));
    }

    fn wire_window(self: &Rc<Self>) {
        let win = &self.window;
        let w = Rc::downgrade(self);
        let a = w.clone();
        win.on_mouse_entered(move || {
            if let Some(h) = a.upgrade() {
                h.vm.controller.on_mouse_enter();
                h.vm.visibility.set_mouse_over(true);
                h.vm.visibility.set_mouse_at_edge(false);
            }
        });
        let a = w.clone();
        win.on_mouse_left(move || {
            if let Some(h) = a.upgrade() {
                h.vm.controller.on_mouse_leave();
                h.vm.visibility.set_mouse_over(false);
                h.vm.visibility.set_mouse_at_edge(false);
            }
        });
        let a = w.clone();
        win.on_notch_clicked(move || {
            if let Some(h) = a.upgrade() {
                h.on_clicked();
            }
        });
        let a = w.clone();
        win.on_wheel(move |dy| {
            if let Some(h) = a.upgrade() {
                h.on_wheel(dy);
            }
        });
        let a = w.clone();
        win.on_key(move |text| a.upgrade().is_some_and(|h| h.on_key(text.as_str())));
        let a = w.clone();
        win.on_open_settings(move || {
            if let Some(h) = a.upgrade() {
                h.vm.open_settings();
            }
        });
        let a = w;
        win.on_open_control_center(move || {
            if let Some(h) = a.upgrade() {
                h.vm.open_control_center();
            }
        });
    }

    // ---- Girdi ----

    fn osd_voice_state(&self) -> Option<(VoiceState, bool)> {
        let s = self.vm.osd.state();
        (s.is_active && s.kind == OsdType::Voice).then(|| (s.voice_state, s.is_voice_editing()))
    }

    fn call(slot: &RefCell<Option<Box<dyn Fn()>>>) {
        if let Some(f) = slot.borrow().as_ref() {
            f();
        }
    }

    fn on_key(&self, text: &str) -> bool {
        if self.osd_voice_state().is_none() {
            return false;
        }
        let enter: slint::SharedString = slint::platform::Key::Return.into();
        let escape: slint::SharedString = slint::platform::Key::Escape.into();
        if text == enter.as_str() {
            Self::call(&self.voice_confirm_requested);
            true
        } else if text == escape.as_str() {
            Self::call(&self.voice_cancel_requested);
            true
        } else {
            false
        }
    }

    fn on_clicked(&self) {
        match self.osd_voice_state() {
            Some((_, true)) => {
                self.focus_voice_confirmation();
                return;
            }
            Some((VoiceState::Completed | VoiceState::Failed, _)) => {
                Self::call(&self.voice_reply_requested);
                return;
            }
            _ => {}
        }
        self.vm.controller.on_click();
    }

    /// Yazı alanı olan sayfalarda tıklama pencereyi etkinleştirir (C#: TextBox'a tıklanınca `Activate`).
    fn on_left_button_down(&self) {
        let hwnd = self.hwnd.get();
        if hwnd == 0 {
            return;
        }
        let st = self.vm.state();
        let has_text_input = st.state == NotchState::Expanded
            && matches!(st.current_panel, NotchPanel::Focus | NotchPanel::Calendar | NotchPanel::Clipboard | NotchPanel::ControlCenter);
        drop(st);
        if has_text_input || self.osd_voice_state().is_some_and(|(_, editing)| editing) {
            WindowStyles::set_no_activate(hwnd, false);
            native_window::activate(hwnd);
        }
    }

    fn on_wheel(&self, dy: f32) {
        if self.vm.is_search_open() || dy == 0.0 {
            return;
        }
        let action = self.vm.settings.borrow().notch.wheel_action;
        if action == WheelAction::Volume {
            self.vm.control_center.adjust_volume(if dy > 0.0 { WHEEL_VOLUME_STEP_PERCENT } else { -WHEEL_VOLUME_STEP_PERCENT });
        } else if dy < 0.0 {
            self.vm.next_panel();
        } else {
            self.vm.previous_panel();
        }
    }

    fn drag_should_open(&self) -> bool {
        let s = self.vm.settings.borrow();
        let elapsed = self.drag_entered_at.get().map_or(0.0, |t| t.elapsed().as_secs_f64() * 1000.0);
        NotchOpenPolicy::drag_should_open(s.notch.open_behavior, elapsed, s.notch.open_delay_ms)
    }

    fn on_drag_over(&self, entered: bool) {
        if entered {
            self.drag_entered_at.set(Some(Instant::now()));
        }
        let open_on_shelf = {
            let st = self.vm.state();
            st.state == NotchState::Expanded && st.current_panel == NotchPanel::Shelf
        };
        if !open_on_shelf && self.drag_should_open() {
            self.vm.expand_to_shelf();
        }
    }

    fn on_drop(&self, data: DropData) {
        self.vm.shelf.handle_drop(&data);
        self.gulp();
    }

    fn on_deactivated(&self) {
        let hwnd = self.hwnd.get();
        if hwnd != 0 {
            WindowStyles::set_no_activate(hwnd, true);
        }
        if let Some(search) = self.vm.search()
            && !search.state().is_pinned
        {
            self.vm.close_search(false);
        }
    }

    // ---- Etkinleştirme ----

    /// Arama kutusuna odaklanır: pencere etkinleştirilebilir olur ve öne gelir.
    pub fn focus_search(&self) {
        let hwnd = self.hwnd.get();
        if hwnd == 0 {
            return;
        }
        WindowStyles::set_no_activate(hwnd, false);
        native_window::activate(hwnd);
        WindowStyles::set_foreground(hwnd);
        let w = self.window.as_weak();
        crate::infra::dispatch::post_local(move || {
            if let Some(win) = w.upgrade() {
                win.invoke_focus_search();
            }
        });
    }

    pub fn focus_voice_confirmation(&self) {
        let hwnd = self.hwnd.get();
        if hwnd == 0 {
            return;
        }
        WindowStyles::set_no_activate(hwnd, false);
        native_window::activate(hwnd);
        WindowStyles::set_foreground(hwnd);
        self.window.invoke_focus_window();
    }

    pub fn release_voice_confirmation(&self, restore_to: isize) {
        let hwnd = self.hwnd.get();
        if hwnd != 0 {
            WindowStyles::set_no_activate(hwnd, true);
        }
        if restore_to != 0 {
            WindowStyles::set_foreground(restore_to);
        }
    }

    fn on_search_closed(&self, restore_to: isize) {
        self.release_voice_confirmation(restore_to);
    }

    // ---- Animasyon ----

    fn gulp(&self) {
        if self.vm.reduce_motion() || !Motion::enabled() {
            return;
        }
        self.anim.borrow_mut().gulp.kick(GULP_KICK_VELOCITY);
        self.wake();
    }

    fn animate_panel_transition(&self, direction: i32) {
        let scale = Motion::scale_for(MotionKind::TabTransitions);
        {
            let mut a = self.anim.borrow_mut();
            if self.vm.reduce_motion() || scale <= 0.0 {
                a.panel_x.snap_to(0.0);
                a.panel_opacity.snap_to(1.0);
            } else {
                a.panel_time_scale = 1.0 / scale;
                a.panel_x.snap_to(if direction > 0 { 35.0 } else { -35.0 });
                a.panel_x.set_target(0.0);
                a.panel_opacity.snap_to(0.25);
                a.panel_opacity.set_target(1.0);
            }
        }
        self.wake();
    }

    fn on_shake(&self) {
        {
            let mut a = self.anim.borrow_mut();
            a.translate_y.snap_to(8.0);
            a.translate_y.set_target(0.0);
        }
        self.wake();
    }

    fn animate_visibility(&self, visible: bool) {
        let hwnd = self.hwnd.get();
        let reduce = self.vm.reduce_motion();
        {
            let mut a = self.anim.borrow_mut();
            a.visible = visible;
            let target = if visible {
                if hwnd != 0 {
                    WindowStyles::set_click_through(hwnd, false);
                    WindowStyles::reassert_topmost(hwnd);
                }
                0.0
            } else {
                let h = a.height.value();
                if h > 0.0 { -(h + 20.0) } else { -150.0 }
            };
            if reduce {
                a.translate_y.snap_to(target);
            } else {
                a.translate_y.set_target(target);
            }
        }
        self.wake();
    }

    fn animate_to(&self, width: f64, height: f64) {
        let osd_active = self.vm.osd.is_active();
        let expanded = self.vm.state().state == NotchState::Expanded;
        let radius = self.vm.state().target_corner_radius;
        {
            let mut a = self.anim.borrow_mut();
            let kind = if osd_active || (a.osd_shape && !expanded) { MotionKind::Osd } else { MotionKind::NotchOpenClose };
            a.osd_shape = osd_active;
            if self.vm.reduce_motion() || !Motion::enabled_for(kind) {
                a.width.snap_to(width);
                a.height.snap_to(height);
                a.corner.snap_to(radius);
            } else {
                retarget(&mut a.width, width);
                retarget(&mut a.height, height);
                retarget(&mut a.corner, radius);
            }
        }
        self.wake();
    }

    fn wake(&self) {
        if self.timer.running() {
            return;
        }
        self.anim.borrow_mut().last = None;
        let me = self.me.borrow().clone();
        self.timer.start(slint::TimerMode::Repeated, FRAME, move || {
            if let Some(h) = me.upgrade() {
                h.tick();
            }
        });
    }

    fn tick(&self) {
        let now = Instant::now();
        let moving = {
            let mut a = self.anim.borrow_mut();
            let dt = a.last.map_or(1.0 / 60.0, |t| (now - t).as_secs_f64());
            a.last = Some(now);
            if dt <= 0.0 {
                return;
            }
            let pdt = dt * a.panel_time_scale;
            a.width.step(dt)
                | a.height.step(dt)
                | a.corner.step(dt)
                | a.translate_y.step(dt)
                | a.panel_x.step(pdt)
                | a.panel_opacity.step(pdt)
                | a.gulp.step(dt)
        };
        self.render_frame();
        if moving {
            return;
        }
        self.timer.stop();
        let visible = self.anim.borrow().visible;
        let hwnd = self.hwnd.get();
        if !visible && hwnd != 0 {
            WindowStyles::set_click_through(hwnd, true);
        }
    }

    /// Yayların anlık değerlerini pencereye uygular: şekil yolu, bölge, kayma, sayfa geçişi.
    fn render_frame(&self) {
        let a = self.anim.borrow();
        let w = a.width.value().max(1.0);
        let gulp = a.gulp.value();
        let sx = (1.0 + GULP_STRETCH_X * gulp).min(NotchLayout::CANVAS_WIDTH / w);
        let sy = 1.0 - GULP_SQUASH_Y * gulp;
        let ew = w * sx;
        let eh = (a.height.value() * sy).max(1.0);
        let ty = a.translate_y.value();
        let spec = ShapeSpec {
            left: (NotchLayout::CANVAS_WIDTH - ew) / 2.0,
            top: ty,
            width: ew,
            height: eh,
            corner_radius: a.corner.value(),
            ear_radius: self.vm.ear_radius(),
        };
        let win = &self.window;
        win.set_shape_width(ew as f32);
        win.set_shape_height(eh as f32);
        win.set_translate_y(ty as f32);
        win.set_panel_translate_x(a.panel_x.value() as f32);
        win.set_panel_opacity(a.panel_opacity.value().clamp(0.0, 1.0) as f32);
        win.set_shape_path(path_commands(&spec).into());
        let hwnd = self.hwnd.get();
        if hwnd != 0 {
            apply_region(hwnd, &spec, win.window().scale_factor() as f64);
        }
    }

    pub fn dispose(&self) {
        self.timer.stop();
        let hwnd = self.hwnd.get();
        for id in self.hooks.borrow_mut().drain(..) {
            native_window::remove_hook(hwnd, id);
        }
        for (kind, id) in self.subs.borrow_mut().drain(..) {
            match kind {
                0 => self.vm.target_size_changed.unsubscribe(id),
                1 => self.vm.target_visibility_changed.unsubscribe(id),
                2 => self.vm.panel_transition_triggered.unsubscribe(id),
                3 => self.vm.shake_requested.unsubscribe(id),
                _ => self.vm.search_closed.unsubscribe(id),
            }
        }
        *self.vm.home.quick_launch.resolve_dialog_owner.borrow_mut() = None;
        self.drop.borrow_mut().take();
        let _ = self.window.hide();
    }
}
