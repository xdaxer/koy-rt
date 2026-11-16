//! Ana sayfadaki müzik kartının sözleri. Yükleme ve zamanlayıcı yalnız Ana sayfa görünürken çalışır (AGENTS #5).
//! Zamanlayıcı saniyede bir yoklamaz, bir sonraki satırın zamanına kurulur.
//! Karşılığı: `Features/Music/LyricsViewModel.cs`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, FixedOffset, Local};
use koy_core::event::Subscription;
use koy_core::lyrics::{ILyricsProvider, LyricsQuery, LyricsSession, LyricsState, LyricsTimeline};
use koy_core::media::{IMediaService, MediaInfo, PlaybackClock};
use koy_core::time::TimeSpan;

use crate::infra::dispatch;
use crate::infra::observable::{LocalEvent, Notifier};
use crate::infra::ui_ref::UiRef;

// Oynatıcı parça bilgisini parça parça bildirir; hepsi gelmeden aramak yanlış sorgu olurdu.
const LOAD_DELAY: Duration = Duration::from_millis(350);
// Parça değişiminden bu kadar eski zaman çizelgesi önceki parçanındır, süresi sorguya katılmaz.
const FRESH_TIMELINE_SECS: i64 = 5;
const MIN_LINE_DELAY: Duration = Duration::from_millis(30);
const SEEK_SETTLE: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone)]
pub struct LyricLineItem {
    pub time: TimeSpan,
    pub text: String,
    pub is_active: bool,
}

pub type SharedMedia = Arc<dyn IMediaService>;

pub struct LyricsViewModel {
    media: SharedMedia,
    session: Option<LyricsSession>,
    session_sub: Cell<Option<Subscription>>,
    media_subs: RefCell<Option<(Subscription, Subscription)>>,
    load_timer: RefCell<Option<slint::Timer>>,
    line_timer: RefCell<Option<slint::Timer>>,
    panel_active: Cell<bool>,
    allowed: Cell<bool>,
    running: Cell<bool>,
    disposed: Cell<bool>,
    track_identity: RefCell<Option<String>>,
    track_changed_at: Cell<Option<DateTime<FixedOffset>>>,
    lines: RefCell<Vec<LyricLineItem>>,
    active_index: Cell<i32>,
    state: Cell<LyricsState>,
    is_available: Cell<bool>,
    is_enabled: Cell<bool>,
    pub changed: Notifier,
    /// Kullanıcı karttaki düğmeyle sözleri açtı ya da kapattı; ayar kalıcı olarak kaydedilmeli.
    pub enabled_toggled: LocalEvent<bool>,
}

impl LyricsViewModel {
    pub fn new(media: SharedMedia, provider: Option<Arc<dyn ILyricsProvider>>) -> Rc<Self> {
        let vm = Rc::new(LyricsViewModel {
            media,
            session: provider.map(LyricsSession::new),
            session_sub: Cell::new(None),
            media_subs: RefCell::new(None),
            load_timer: RefCell::new(None),
            line_timer: RefCell::new(None),
            panel_active: Cell::new(false),
            allowed: Cell::new(false),
            running: Cell::new(false),
            disposed: Cell::new(false),
            track_identity: RefCell::new(None),
            track_changed_at: Cell::new(None),
            lines: RefCell::new(Vec::new()),
            active_index: Cell::new(-1),
            state: Cell::new(LyricsState::Idle),
            is_available: Cell::new(false),
            is_enabled: Cell::new(false),
            changed: Notifier::default(),
            enabled_toggled: LocalEvent::default(),
        });
        if let Some(session) = &vm.session {
            let me = UiRef::new(&vm);
            // Oturum olayı yükleme iş parçacığından gelir.
            let sub = session.changed().subscribe(move |_| {
                dispatch::post(move || {
                    if let Some(vm) = me.upgrade() {
                        vm.on_session_changed();
                    }
                });
            });
            vm.session_sub.set(Some(sub));
        }
        vm
    }

    pub fn lines(&self) -> std::cell::Ref<'_, Vec<LyricLineItem>> {
        self.lines.borrow()
    }

    pub fn active_index(&self) -> i32 {
        self.active_index.get()
    }

    pub fn state(&self) -> LyricsState {
        self.state.get()
    }

    pub fn is_enabled(&self) -> bool {
        self.is_enabled.get()
    }

    /// Kaynak zinciri kurulduysa kartta sözler düğmesi gösterilir.
    pub fn can_toggle(&self) -> bool {
        self.session.is_some()
    }

    /// Kartta sözler mi var. Yüklenirken ya da söz yoksa kartta ses çubukları kalır.
    pub fn is_shown(&self) -> bool {
        self.is_available.get() && self.state.get() == LyricsState::Ready
    }

    /// Çentik ayarındaki "şarkı sözleri" seçeneği.
    pub fn set_allowed(self: &Rc<Self>, allowed: bool) {
        self.allowed.set(allowed);
        self.is_enabled.set(allowed);
        self.is_available.set(allowed && self.session.is_some());
        self.update_running();
        self.changed.notify();
    }

    /// Çentik açık ve Ana sekmesi görünür mü.
    pub fn set_panel_active(self: &Rc<Self>, active: bool) {
        self.panel_active.set(active);
        self.update_running();
    }

    pub fn toggle_enabled(self: &Rc<Self>) {
        if self.session.is_none() {
            return;
        }
        self.set_allowed(!self.allowed.get());
        self.enabled_toggled.invoke(&self.allowed.get());
    }

    pub fn seek_to_line(self: &Rc<Self>, index: usize) {
        let Some(session) = &self.session else { return };
        let Some(time) = self.lines.borrow().get(index).map(|l| l.time) else { return };
        session.update_position(time);
        self.apply_active_line();
        // Oynatıcının yeni konumu bildirmesi gecikebilir: eski konumla vurgu geri sıçramasın.
        self.start_line_timer(SEEK_SETTLE);
        let start = self.media.timeline().map(|t| t.start_time).unwrap_or(TimeSpan::ZERO);
        let media = Arc::clone(&self.media);
        std::thread::spawn(move || media.seek(start + time));
        self.changed.notify();
    }

    fn start_line_timer(self: &Rc<Self>, after: Duration) {
        let weak = Rc::downgrade(self);
        *self.line_timer.borrow_mut() = Some(dispatch::delay(after, move || {
            if let Some(vm) = weak.upgrade() {
                *vm.line_timer.borrow_mut() = None;
                vm.sync();
            }
        }));
    }

    fn update_running(self: &Rc<Self>) {
        let want = !self.disposed.get() && self.is_available.get() && self.panel_active.get();
        if want == self.running.get() {
            return;
        }
        self.running.set(want);
        if want {
            let me = UiRef::new(self);
            let s1 = self.media.changed().subscribe(move |info: &Option<MediaInfo>| {
                let info = info.clone();
                dispatch::post(move || {
                    if let Some(vm) = me.upgrade() {
                        vm.on_media_changed(info);
                    }
                });
            });
            let s2 = self.media.timeline_changed().subscribe(move |_| {
                dispatch::post(move || {
                    // Parça yeni değiştiyse konum eski sözlere uygulanmasın.
                    if let Some(vm) = me.upgrade()
                        && vm.running.get()
                        && vm.load_timer.borrow().is_none()
                    {
                        vm.sync();
                    }
                });
            });
            *self.media_subs.borrow_mut() = Some((s1, s2));
            *self.track_identity.borrow_mut() = identity(self.media.current().as_ref());
            self.track_changed_at.set(None);
            if let Some(s) = &self.session {
                let _ = s.load(self.build_query().as_ref());
            }
            self.sync();
        } else {
            if let Some((a, b)) = self.media_subs.borrow_mut().take() {
                self.media.changed().unsubscribe(a);
                self.media.timeline_changed().unsubscribe(b);
            }
            *self.load_timer.borrow_mut() = None;
            *self.line_timer.borrow_mut() = None;
            if let Some(s) = &self.session {
                s.suspend();
            }
        }
    }

    fn on_media_changed(self: &Rc<Self>, info: Option<MediaInfo>) {
        if !self.running.get() {
            return;
        }
        let id = identity(info.as_ref());
        if id != *self.track_identity.borrow() {
            *self.track_identity.borrow_mut() = id;
            self.track_changed_at.set(Some(Local::now().fixed_offset()));
            *self.line_timer.borrow_mut() = None;
            let weak = Rc::downgrade(self);
            *self.load_timer.borrow_mut() = Some(dispatch::delay(LOAD_DELAY, move || {
                if let Some(vm) = weak.upgrade() {
                    *vm.load_timer.borrow_mut() = None;
                    if vm.running.get()
                        && let Some(s) = &vm.session
                    {
                        let _ = s.load(vm.build_query().as_ref());
                    }
                }
            }));
            return;
        }
        self.sync();
    }

    fn on_session_changed(self: &Rc<Self>) {
        let Some(session) = &self.session else { return };
        self.active_index.set(-1);
        *self.lines.borrow_mut() = session
            .lines()
            .into_iter()
            .map(|l| LyricLineItem { time: l.time, text: if l.text.is_empty() { "♪".into() } else { l.text }, is_active: false })
            .collect();
        self.state.set(session.state());
        self.sync();
        self.changed.notify();
    }

    fn build_query(&self) -> Option<LyricsQuery> {
        let info = self.media.current()?;
        let duration = match (self.media.timeline(), self.track_changed_at.get()) {
            (Some(t), changed_at) => {
                // C#: _trackChangedAt - timeline.LastUpdated <= 5 sn (MinValue başlangıçta hep doğru).
                let fresh = changed_at.is_none_or(|c| (c - t.last_updated).num_milliseconds() <= FRESH_TIMELINE_SECS * 1000);
                if fresh { PlaybackClock::duration(&t) } else { TimeSpan::ZERO }
            }
            _ => TimeSpan::ZERO,
        };
        LyricsQuery::create(Some(&info.title), Some(&info.artist), duration)
    }

    fn sync(self: &Rc<Self>) {
        *self.line_timer.borrow_mut() = None;
        let Some(session) = &self.session else { return };
        if !self.running.get() || session.state() != LyricsState::Ready {
            return;
        }
        let Some(timeline) = self.media.timeline() else { return };
        let is_playing = self.media.current().is_some_and(|c| c.is_playing);
        let position = PlaybackClock::position_at(&timeline, is_playing, Local::now().fixed_offset());
        if session.update_position(position) {
            self.apply_active_line();
            self.changed.notify();
        }
        if !is_playing {
            return;
        }
        let Some(delay) = LyricsTimeline::until_next_change(&session.lines(), position) else { return };
        let delay = Duration::from_millis(delay.total_milliseconds().max(0.0) as u64).max(MIN_LINE_DELAY);
        self.start_line_timer(delay);
    }

    fn apply_active_line(&self) {
        let Some(session) = &self.session else { return };
        let index = session.active_index();
        if index == self.active_index.get() {
            return;
        }
        let mut lines = self.lines.borrow_mut();
        for (i, l) in lines.iter_mut().enumerate() {
            l.is_active = i as i32 == index;
        }
        self.active_index.set(if index >= 0 && (index as usize) < lines.len() { index } else { -1 });
    }

    pub fn dispose(self: &Rc<Self>) {
        if self.disposed.replace(true) {
            return;
        }
        self.update_running();
        if let (Some(s), Some(sub)) = (&self.session, self.session_sub.take()) {
            s.changed().unsubscribe(sub);
        }
    }
}

fn identity(info: Option<&MediaInfo>) -> Option<String> {
    info.map(|i| format!("{}\n{}", i.title, i.artist))
}
