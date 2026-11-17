//! Çentik özelliğinin kartları: güncelleme kartı, bildirim kartları ve sesli komut göstergesi.
//! Karşılığı: `NotchFeature.cs` içindeki `TryShowUpdateCard`, `PumpNotifications` ve `UpdateVoiceOsd` bölümleri.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::Duration;

use slint::ComponentHandle;

use koy_core::cancel::CancellationTokenSource;
use koy_core::event::Subscription;
use koy_core::notch::{NotchCard, NotchCardKind, NotchState};
use koy_core::notifications::{
    INotificationListener, NotificationAccessStatus, NotificationItem, NotificationPresentation, NotificationQueue, RelativeTimeLabels,
};
use koy_core::shell::IFocusAssistService;
use koy_core::updates::UpdateCardPolicy;
use koy_core::voice::{HarnessExecutionResult, VoiceFailure, VoiceHarnessType, VoiceSessionController, VoiceSessionOptions, VoiceState};
use koy_platform::notifications::WinRtNotificationListener;
use koy_platform::shell::WindowStyles;
use koy_platform::voice::{ProcessHarnessRunnerService, VoiceTyping};

use super::notch_feature::{Inner, WindowState};
use super::notch_view_model::NotchViewModel;
use super::osd::osd_indicator_view_model::{OsdIndicatorViewModel, OsdType};
use crate::infra::dispatch;
use crate::infra::ui_ref::UiRef;
use crate::resources::strings::Strings;
use crate::update_status::UpdateStatus;

thread_local! {
    // C#'ta statik: özellik yeniden başlatılınca aynı sürüm için kart tekrar gösterilmez.
    static UPDATE_CARD_POLICY: RefCell<UpdateCardPolicy> = RefCell::new(UpdateCardPolicy::new(None));
}

type OsdHook = (Weak<OsdIndicatorViewModel>, u64, u64);
type VmWait = (Weak<NotchViewModel>, u64);

/// Kart ve sesli komut durumu.
#[derive(Default)]
pub(crate) struct CardState {
    /// Özelliğin kendisine zayıf başvuru (zamanlayıcı ve olay geri çağrıları için).
    pub me: RefCell<Weak<Inner>>,
    /// Güncelleme kartının birincil düğmesi (Ayarlar penceresinde güncelleme katmanını açar); uygulama kurar.
    pub open_update_handler: RefCell<Option<Box<dyn Fn()>>>,
    // Güncelleme kartı.
    card_osd: RefCell<Option<OsdHook>>,
    card_wait: RefCell<Option<VmWait>>,
    card_version: RefCell<Option<String>>,
    update_hooked: Cell<bool>,
    // Bildirimler.
    listener: RefCell<Option<(Arc<WinRtNotificationListener>, Option<Subscription>)>>,
    tray_sub: RefCell<Option<Subscription>>,
    queue: RefCell<Option<NotificationQueue>>,
    notification_osd: RefCell<Option<OsdHook>>,
    notification_card: RefCell<Option<NotchCard>>,
    notification_wait: RefCell<Option<VmWait>>,
    balloon_sequence: Cell<u32>,
    // Sesli komut.
    voice: RefCell<Option<(Arc<VoiceSessionController>, Subscription)>>,
    // Seçenek sağlayıcısının okuduğu ses ayarları; her tetiklemede güncel ayardan yenilenir (C#'ta ayar nesnesi canlıdır).
    voice_settings: RefCell<Option<Arc<std::sync::Mutex<koy_core::settings::VoiceSettings>>>>,
    voice_prior_foreground: Cell<isize>,
    voice_window: RefCell<Option<Weak<NotchViewModel>>>,
    voice_input_sub: RefCell<Option<(Weak<NotchViewModel>, u64)>>,
    voice_typing: RefCell<Option<CancellationTokenSource>>,
}

fn with_queue<R>(me: &Inner, f: impl FnOnce(&mut NotificationQueue) -> R) -> R {
    let mut q = me.cards.queue.borrow_mut();
    f(q.get_or_insert_with(|| NotificationQueue::new(None)))
}

/// Ana ekrandaki (yoksa ilk) çentik.
fn primary(me: &Inner) -> Option<Rc<NotchViewModel>> {
    let windows = me.windows.borrow();
    windows.iter().find(|w| w.monitor().is_primary).or_else(|| windows.first()).map(|w| Rc::clone(&w.vm))
}

fn window_of(me: &Inner, vm: &Rc<NotchViewModel>) -> Option<usize> {
    me.windows.borrow().iter().position(|w| Rc::ptr_eq(&w.vm, vm))
}

/// `vm` açık değilken (durum Expanded'dan çıkınca) `then` bir kez çalışır.
fn wait_for_collapse(slot: &RefCell<Option<VmWait>>, vm: &Rc<NotchViewModel>, then: impl Fn() + 'static) {
    if slot.borrow().as_ref().and_then(|(w, _)| w.upgrade()).is_some_and(|w| Rc::ptr_eq(&w, vm)) {
        return;
    }
    stop_waiting(slot);
    let wv = Rc::downgrade(vm);
    let id = vm.changed.subscribe(move || {
        if wv.upgrade().is_some_and(|v| v.state().state != NotchState::Expanded) {
            // Abonelik kendi içinden kaldırılamaz: iş bir sonraki tura bırakılır.
            let then_once = &then;
            then_once();
        }
    });
    *slot.borrow_mut() = Some((Rc::downgrade(vm), id));
}

fn stop_waiting(slot: &RefCell<Option<VmWait>>) {
    if let Some((w, id)) = slot.borrow_mut().take()
        && let Some(vm) = w.upgrade()
    {
        vm.changed.unsubscribe(id);
    }
}

pub(super) fn start(me: &Rc<Inner>) {
    *me.cards.me.borrow_mut() = Rc::downgrade(me);
    start_notifications(me);
    if !me.cards.update_hooked.replace(true) {
        let w = Rc::downgrade(me);
        UpdateStatus::on_changed(move || {
            if let Some(me) = w.upgrade() {
                let w = Rc::downgrade(&me);
                dispatch::post_local(move || {
                    if let Some(me) = w.upgrade() {
                        try_show_update_card(&me);
                    }
                });
            }
        });
    }
    let w = Rc::downgrade(me);
    dispatch::post_local(move || {
        if let Some(me) = w.upgrade() {
            try_show_update_card(&me);
        }
    });
}

pub(super) fn stop(me: &Inner) {
    stop_waiting(&me.cards.card_wait);
    detach_card_osd(me);
    stop_notifications(me);
    stop_voice_typing(me);
    detach_voice_input(me);
    if let Some((v, sub)) = me.cards.voice.borrow_mut().take() {
        v.state_changed.unsubscribe(sub);
        v.dispose();
    }
    super::voice_reply_window::close_current();
}

// ---- Güncelleme kartı ----

fn try_show_update_card(me: &Rc<Inner>) {
    if me.stopped.get() {
        return;
    }
    let Some(vm) = primary(me) else { return };
    let version = UpdateStatus::available_version();
    let has_update = UpdateStatus::has_available_update();
    let expanded = vm.state().state == NotchState::Expanded;
    let show = UPDATE_CARD_POLICY.with(|p| p.borrow().should_show(has_update, version.as_deref(), expanded));
    if !show {
        if expanded && has_update && version.as_deref().is_some_and(|v| !v.trim().is_empty()) {
            let w = Rc::downgrade(me);
            wait_for_collapse(&me.cards.card_wait, &vm, move || {
                if let Some(me) = w.upgrade() {
                    let w = Rc::downgrade(&me);
                    dispatch::post_local(move || {
                        if let Some(me) = w.upgrade() {
                            stop_waiting(&me.cards.card_wait);
                            try_show_update_card(&me);
                        }
                    });
                }
            });
        }
        return;
    }
    stop_waiting(&me.cards.card_wait);
    let version = version.unwrap_or_default();
    let mut card = NotchCard::new(
        NotchCardKind::Update,
        Strings::format("UpdateCardTitle", &[&version]),
        Strings::get("UpdateCardSubtitle"),
    );
    card.primary_action_label = Strings::get("UpdateCardAction");
    detach_card_osd(me);
    let osd = Rc::clone(&vm.osd);
    let w = Rc::downgrade(me);
    let a = osd.primary_action_requested.subscribe(move |card| {
        if card.kind == NotchCardKind::Update
            && let Some(me) = w.upgrade()
            && let Some(f) = me.cards.open_update_handler.borrow().as_ref()
        {
            f();
        }
    });
    let w = Rc::downgrade(me);
    let d = osd.dismiss_requested.subscribe(move |card| {
        if card.kind == NotchCardKind::Update
            && let Some(me) = w.upgrade()
            && let Some(v) = me.cards.card_version.borrow().as_ref()
        {
            UPDATE_CARD_POLICY.with(|p| p.borrow_mut().dismiss(v));
        }
    });
    *me.cards.card_osd.borrow_mut() = Some((Rc::downgrade(&osd), a, d));
    let lifetime = Duration::from_millis(UpdateCardPolicy::AUTO_DISMISS_AFTER.total_milliseconds() as u64);
    if osd.show_card(card, lifetime) {
        UPDATE_CARD_POLICY.with(|p| p.borrow_mut().mark_shown(&version));
        *me.cards.card_version.borrow_mut() = Some(version);
    } else {
        detach_card_osd(me);
    }
}

fn detach_card_osd(me: &Inner) {
    if let Some((w, a, d)) = me.cards.card_osd.borrow_mut().take()
        && let Some(osd) = w.upgrade()
    {
        osd.primary_action_requested.unsubscribe(a);
        osd.dismiss_requested.unsubscribe(d);
    }
}

// ---- Bildirimler ----

fn start_notifications(me: &Rc<Inner>) {
    if !me.settings.borrow().notch.show_notifications || me.windows.borrow().is_empty() || me.cards.listener.borrow().is_some() {
        return;
    }
    // Tepsi balonları (arayüz iş parçacığında yükselir).
    let ui = UiRef::new(me);
    let sub = me.services.tray_balloon_received().subscribe(move |balloon| {
        let ui = ui.clone();
        let balloon = balloon.clone();
        dispatch::post(move || {
            if let Some(me) = ui.upgrade() {
                on_tray_balloon(&me, &balloon);
            }
        });
    });
    *me.cards.tray_sub.borrow_mut() = Some(sub);
    let listener = me.services.acquire_notification_listener();
    *me.cards.listener.borrow_mut() = Some((Arc::clone(&listener), None));
    // Erişim isteği kullanıcıya sorabilir: arka planda.
    let ui = UiRef::new(me);
    std::thread::spawn(move || {
        let mut status = listener.access_status();
        if status == NotificationAccessStatus::Unspecified {
            status = listener.request_access();
        }
        dispatch::post(move || {
            let Some(me) = ui.upgrade() else { return };
            super::notification_access::raise_changed();
            let same = me.cards.listener.borrow().as_ref().is_some_and(|(l, _)| Arc::ptr_eq(l, &listener));
            if status != NotificationAccessStatus::Allowed || me.stopped.get() || !same {
                return;
            }
            let ui = UiRef::new(&me);
            let sub = listener.notification_arrived().subscribe(move |item| {
                let ui = ui.clone();
                let item = item.clone();
                dispatch::post(move || {
                    if let Some(me) = ui.upgrade() {
                        on_notification_arrived(&me, item);
                    }
                });
            });
            listener.start();
            if let Some((_, s)) = me.cards.listener.borrow_mut().as_mut() {
                *s = Some(sub);
            }
        });
    });
}

fn do_not_disturb(me: &Inner) -> bool {
    me.svc.borrow().focus_assist.as_ref().is_some_and(|f| f.is_do_not_disturb())
}

fn on_notification_arrived(me: &Rc<Inner>, item: NotificationItem) {
    if me.stopped.get() || me.cards.listener.borrow().is_none() {
        return;
    }
    let dnd = do_not_disturb(me);
    if with_queue(me, |q| q.enqueue(item, dnd)) {
        pump_notifications(me);
    }
}

fn on_tray_balloon(me: &Rc<Inner>, balloon: &koy_core::tray::TrayBalloonNotification) {
    if me.stopped.get() || !me.settings.borrow().notch.show_notifications || me.windows.borrow().is_empty() {
        return;
    }
    let seq = me.cards.balloon_sequence.get().wrapping_add(1);
    me.cards.balloon_sequence.set(seq);
    let item = NotificationPresentation::from_tray_balloon(balloon, 0x8000_0000u32 | seq);
    let dnd = do_not_disturb(me);
    if with_queue(me, |q| q.enqueue(item, dnd)) {
        pump_notifications(me);
    }
}

fn relative_time_labels() -> RelativeTimeLabels {
    RelativeTimeLabels::new(
        Strings::get("NotificationTimeNow"),
        Strings::get("NotificationTimeMinutesAgo"),
        Strings::get("NotificationTimeHoursAgo"),
    )
}

fn pump_notifications(me: &Rc<Inner>) {
    if me.stopped.get() || with_queue(me, |q| q.current().is_some()) {
        return;
    }
    let Some(vm) = primary(me) else { return };
    hook_notification_osd(me, &vm.osd);
    if vm.osd.is_active() {
        return;
    }
    if vm.state().state == NotchState::Expanded {
        if with_queue(me, |q| q.pending_count()) > 0 {
            let w = Rc::downgrade(me);
            wait_for_collapse(&me.cards.notification_wait, &vm, move || {
                if let Some(me) = w.upgrade() {
                    let w = Rc::downgrade(&me);
                    dispatch::post_local(move || {
                        if let Some(me) = w.upgrade() {
                            stop_waiting(&me.cards.notification_wait);
                            pump_notifications(&me);
                        }
                    });
                }
            });
        }
        return;
    }
    stop_waiting(&me.cards.notification_wait);
    let Some(item) = with_queue(me, |q| q.try_take_next(false)) else { return };
    let hide_body = me.settings.borrow().notch.notifications_hide_body;
    let card = NotificationPresentation::to_card(&item, chrono::Local::now().fixed_offset(), hide_body, &relative_time_labels());
    *me.cards.notification_card.borrow_mut() = Some(card.clone());
    let lifetime = Duration::from_millis(NotificationQueue::CARD_DURATION.total_milliseconds() as u64);
    if !vm.osd.show_card(card, lifetime) {
        finish_notification_card(me);
    }
}

fn hook_notification_osd(me: &Rc<Inner>, osd: &Rc<OsdIndicatorViewModel>) {
    let same = me.cards.notification_osd.borrow().as_ref().and_then(|(w, ..)| w.upgrade()).is_some_and(|o| Rc::ptr_eq(&o, osd));
    if same {
        return;
    }
    unhook_notification_osd(me);
    let w = Rc::downgrade(me);
    let dismissed = osd.dismissed.subscribe(move || {
        if let Some(me) = w.upgrade() {
            finish_notification_card(&me);
            let w = Rc::downgrade(&me);
            dispatch::post_local(move || {
                if let Some(me) = w.upgrade() {
                    pump_notifications(&me);
                }
            });
        }
    });
    let w = Rc::downgrade(me);
    let wo = Rc::downgrade(osd);
    let activated = osd.activated.subscribe(move || {
        let (Some(me), Some(osd)) = (w.upgrade(), wo.upgrade()) else { return };
        if with_queue(&me, |q| q.current().is_none()) {
            return;
        }
        let ours = me.cards.notification_card.borrow().clone();
        let Some(ours) = ours else { return };
        let st = osd.state();
        // Bizim kartımız gösteriliyorsa iş bitmedi; başka bir gösterge kartın yerini aldıysa kart tamamlanır.
        if st.kind == OsdType::Card && st.card.as_ref() == Some(&ours) {
            return;
        }
        drop(st);
        finish_notification_card(&me);
    });
    *me.cards.notification_osd.borrow_mut() = Some((Rc::downgrade(osd), dismissed, activated));
}

fn unhook_notification_osd(me: &Inner) {
    let hook = me.cards.notification_osd.borrow_mut().take();
    if let Some((w, d, a)) = hook {
        if let Some(osd) = w.upgrade() {
            osd.dismissed.unsubscribe(d);
            osd.activated.unsubscribe(a);
        }
        finish_notification_card(me);
    }
}

fn finish_notification_card(me: &Inner) {
    *me.cards.notification_card.borrow_mut() = None;
    with_queue(me, |q| q.complete());
}

fn stop_notifications(me: &Inner) {
    if let Some(sub) = me.cards.tray_sub.borrow_mut().take() {
        me.services.tray_balloon_received().unsubscribe(sub);
    }
    stop_waiting(&me.cards.notification_wait);
    unhook_notification_osd(me);
    with_queue(me, |q| q.clear());
    let listener = me.cards.listener.borrow_mut().take();
    if let Some((l, sub)) = listener {
        if let Some(sub) = sub {
            l.notification_arrived().unsubscribe(sub);
        }
        me.services.release_notification_listener();
    }
}

// ---- Sesli komut ----

fn ensure_voice_controller(me: &Rc<Inner>) -> Arc<VoiceSessionController> {
    if let Some((v, _)) = me.cards.voice.borrow().as_ref() {
        return Arc::clone(v);
    }
    let settings = Arc::new(std::sync::Mutex::new(me.settings.borrow().voice.clone()));
    *me.cards.voice_settings.borrow_mut() = Some(Arc::clone(&settings));
    let snapshot = Arc::clone(&settings);
    let controller = Arc::new(VoiceSessionController::new(Arc::new(ProcessHarnessRunnerService), move || {
        let v = snapshot.lock().unwrap_or_else(|e| e.into_inner()).clone();
        VoiceSessionOptions {
            harness_type: v.harness,
            working_directory: v.working_directory.clone(),
            timeout_seconds: v.timeout_seconds,
            custom_executable: Some(v.custom_command.clone()).filter(|c| !c.trim().is_empty()),
            claude_permission: v.claude_permission,
            model: Some(v.model.clone()).filter(|m| !m.trim().is_empty()),
            ..Default::default()
        }
    }));
    let ui = UiRef::new(me);
    let sub = controller.state_changed.subscribe(move |state| {
        let ui = ui.clone();
        let state = *state;
        dispatch::post(move || {
            if let Some(me) = ui.upgrade() {
                update_voice_osd(&me, state);
            }
        });
    });
    *me.cards.voice.borrow_mut() = Some((Arc::clone(&controller), sub));
    controller
}

pub(super) fn toggle_voice_command(me: &Rc<Inner>) {
    let controller = ensure_voice_controller(me);
    if let Some(v) = me.cards.voice_settings.borrow().as_ref() {
        *v.lock().unwrap_or_else(|e| e.into_inner()) = me.settings.borrow().voice.clone();
    }
    std::thread::spawn(move || controller.trigger_shortcut());
}

pub(super) fn confirm_voice_command(me: &Rc<Inner>) {
    if let Some((v, _)) = me.cards.voice.borrow().as_ref() {
        let v = Arc::clone(v);
        std::thread::spawn(move || v.confirm());
    }
}

pub(super) fn cancel_voice_command(me: &Rc<Inner>) {
    if let Some((v, _)) = me.cards.voice.borrow().as_ref() {
        v.cancel();
    }
}

fn harness_name(h: VoiceHarnessType) -> String {
    match h {
        VoiceHarnessType::Agy => "Antigravity".into(),
        VoiceHarnessType::Custom => Strings::get("VoiceHarnessCustom"),
        VoiceHarnessType::Claude => "Claude".into(),
        VoiceHarnessType::Codex => "Codex".into(),
    }
}

fn first_line(text: &str) -> Option<String> {
    text.split('\n').map(str::trim).find(|l| !l.is_empty()).map(str::to_string)
}

fn failure_key(f: VoiceFailure) -> &'static str {
    match f {
        VoiceFailure::None | VoiceFailure::Unexpected => "VoiceErrorUnexpected",
        VoiceFailure::InvalidCommand => "VoiceErrorInvalidCommand",
        VoiceFailure::InvalidModel => "VoiceErrorInvalidModel",
        VoiceFailure::ScriptNotSupported => "VoiceErrorScriptNotSupported",
        VoiceFailure::CommandNotFound => "VoiceErrorCommandNotFound",
        VoiceFailure::Timeout => "VoiceErrorTimeout",
        VoiceFailure::ExitCode => "VoiceErrorExitCode",
        VoiceFailure::StartFailed => "VoiceErrorStartFailed",
    }
}

fn failure_text(c: &VoiceSessionController) -> String {
    if c.failure() == VoiceFailure::ExitCode
        && let Some(r) = c.last_result()
        && let Some(line) = first_line(&r.error).or_else(|| first_line(&r.output))
    {
        return line;
    }
    Strings::format(failure_key(c.failure()), &[&c.failure_detail()])
}

fn status_text(me: &Inner, c: &VoiceSessionController, state: VoiceState) -> String {
    match state {
        VoiceState::Editing => Strings::get("VoiceEditing"),
        VoiceState::Running => Strings::format("VoiceRunning", &[&harness_name(me.settings.borrow().voice.harness)]),
        VoiceState::Completed => Strings::get("VoiceCompleted"),
        VoiceState::Failed => failure_text(c),
        VoiceState::Idle => String::new(),
    }
}

pub(super) fn voice_reply_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("Koy").join("voice-reply.txt")
}

fn save_voice_reply(me: &Inner, prompt: &str, r: &HarnessExecutionResult) -> bool {
    if r.output.trim().is_empty() && r.error.trim().is_empty() {
        return false;
    }
    let mut text = format!(
        "> {prompt}\n({}, {}, {:.1} s)\n\n{}\n",
        harness_name(me.settings.borrow().voice.harness),
        r.exit_code,
        r.duration.total_seconds(),
        r.output
    );
    if !r.error.trim().is_empty() {
        text.push_str(&format!("\n[stderr]\n{}\n", r.error));
    }
    let path = voice_reply_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(&path, text) {
        Ok(()) => true,
        Err(e) => {
            koy_core::diagnostics::CrashLog::write(
                "Sesli komut yanıtı",
                &koy_core::diagnostics::CrashError::exception("IOException", &e.to_string(), ""),
            );
            false
        }
    }
}

fn show_voice_reply_card(me: &Inner, target: &WindowState, prompt: &str, r: &HarnessExecutionResult) {
    let body = if r.error.trim().is_empty() {
        r.output.clone()
    } else if r.output.trim().is_empty() {
        r.error.clone()
    } else {
        format!("{}\n\n{}", r.output, r.error)
    };
    let info = format!("{} · {:.1} s", harness_name(me.settings.borrow().voice.harness), r.duration.total_seconds());
    // Çentiğin altında, ortada (pencerenin üst kenarı + 72 DIP).
    let win = target.host.window.window();
    let pos = win.position().to_logical(win.scale_factor());
    let (cx, top) = (pos.x, pos.y);
    super::voice_reply_window::show_reply(cx as f64 + 320.0, top as f64 + 72.0, prompt, &info, &body, Box::new(open_voice_reply_file));
}

pub(super) fn open_voice_reply(me: &Rc<Inner>) {
    if super::voice_reply_window::focus_current() {
        return;
    }
    open_voice_reply_file();
    for w in me.windows.borrow().iter() {
        if w.vm.osd.kind() == OsdType::Voice {
            w.vm.osd.dismiss();
        }
    }
}

fn open_voice_reply_file() {
    let path = voice_reply_path();
    if !path.exists() {
        return;
    }
    let shell = koy_platform::shell::ShellActionService;
    if let Err(e) = koy_core::shell::IShellActionService::launch(&shell, &path.to_string_lossy(), None) {
        koy_core::diagnostics::CrashLog::write(
            "Sesli komut yanıtını açma",
            &koy_core::diagnostics::CrashError::exception("Win32Exception", &e.to_string(), ""),
        );
    }
}

fn detach_voice_input(me: &Inner) {
    if let Some((w, id)) = me.cards.voice_input_sub.borrow_mut().take()
        && let Some(vm) = w.upgrade()
    {
        vm.osd.voice_input_changed.unsubscribe(id);
    }
}

fn update_voice_osd(me: &Rc<Inner>, state: VoiceState) {
    let Some(controller) = me.cards.voice.borrow().as_ref().map(|(v, _)| Arc::clone(v)) else { return };
    if state == VoiceState::Editing {
        super::voice_reply_window::close_current();
        me.cards.voice_prior_foreground.set(WindowStyles::get_foreground());
        let target = me.pick_for_shortcut().map(|i| Rc::downgrade(&me.windows.borrow()[i].vm));
        *me.cards.voice_window.borrow_mut() = target;
    }
    let Some(vm) = me.cards.voice_window.borrow().as_ref().and_then(Weak::upgrade) else { return };
    let Some(index) = window_of(me, &vm) else { return };
    let host = Rc::clone(&me.windows.borrow()[index].host);
    let prior = me.cards.voice_prior_foreground.get();
    if state == VoiceState::Idle {
        stop_voice_typing(me);
        detach_voice_input(me);
        if vm.osd.kind() == OsdType::Voice {
            vm.osd.dismiss();
        }
        host.release_voice_confirmation(prior);
        me.cards.voice_prior_foreground.set(0);
        *me.cards.voice_window.borrow_mut() = None;
        return;
    }
    let mut text = status_text(me, &controller, state);
    let mut prompt = if state == VoiceState::Editing { String::new() } else { controller.prompt() };
    let mut detail = (state == VoiceState::Failed).then(|| text.clone());
    let mut has_reply = false;
    if matches!(state, VoiceState::Completed | VoiceState::Failed)
        && let Some(result) = controller.last_result()
        && save_voice_reply(me, &controller.prompt(), &result)
    {
        has_reply = true;
        show_voice_reply_card(me, &me.windows.borrow()[index], &controller.prompt(), &result);
        if state == VoiceState::Completed {
            prompt = first_line(&result.output).unwrap_or_else(|| Strings::get("VoiceCompleted"));
            text = Strings::get("VoiceReplyOpen");
            detail = Some(if result.output.chars().count() > 600 {
                format!("{}…", result.output.chars().take(600).collect::<String>())
            } else {
                result.output.clone()
            });
        }
    }
    vm.osd.show_voice(state, &text, &prompt, detail.as_deref(), has_reply);
    if state == VoiceState::Editing {
        detach_voice_input(me);
        let c = Arc::clone(&controller);
        let id = vm.osd.voice_input_changed.subscribe(move |t| c.update_prompt(t));
        *me.cards.voice_input_sub.borrow_mut() = Some((Rc::downgrade(&vm), id));
        host.focus_voice_confirmation();
        start_voice_typing(me);
        return;
    }
    stop_voice_typing(me);
    detach_voice_input(me);
    host.release_voice_confirmation(prior);
    if matches!(state, VoiceState::Completed | VoiceState::Failed) {
        me.cards.voice_prior_foreground.set(0);
        *me.cards.voice_window.borrow_mut() = None;
    }
}

/// Windows sesle yazmayı (Win+H) açar; açılırken odak kaçarsa kutu iki kez yeniden odaklanır.
fn start_voice_typing(me: &Rc<Inner>) {
    stop_voice_typing(me);
    let cts = CancellationTokenSource::new();
    let token = cts.token();
    *me.cards.voice_typing.borrow_mut() = Some(cts);
    let ui = UiRef::new(me);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(120));
        if token.is_cancellation_requested() {
            return;
        }
        let still_editing = {
            let (tx, rx) = std::sync::mpsc::channel();
            let ui2 = ui.clone();
            dispatch::post(move || {
                let editing = ui2
                    .upgrade()
                    .and_then(|me| me.cards.voice.borrow().as_ref().map(|(v, _)| v.state() == VoiceState::Editing))
                    .unwrap_or(false);
                let _ = tx.send(editing);
            });
            rx.recv_timeout(Duration::from_secs(2)).unwrap_or(false)
        };
        if !still_editing {
            return;
        }
        let _ = VoiceTyping::open(Duration::from_secs(2), &token);
        for delay in [400u64, 900] {
            std::thread::sleep(Duration::from_millis(delay));
            if token.is_cancellation_requested() {
                return;
            }
            let ui = ui.clone();
            dispatch::post(move || {
                let Some(me) = ui.upgrade() else { return };
                let editing = me.cards.voice.borrow().as_ref().is_some_and(|(v, _)| v.state() == VoiceState::Editing);
                let vm = me.cards.voice_window.borrow().as_ref().and_then(Weak::upgrade);
                if let (true, Some(vm)) = (editing, vm)
                    && let Some(i) = window_of(&me, &vm)
                {
                    let host = Rc::clone(&me.windows.borrow()[i].host);
                    host.focus_voice_confirmation();
                }
            });
        }
    });
}

fn stop_voice_typing(me: &Inner) {
    if let Some(cts) = me.cards.voice_typing.borrow_mut().take() {
        cts.cancel();
    }
}
