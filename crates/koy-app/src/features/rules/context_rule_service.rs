//! Karşılığı: `koy/src/App/Features/Rules/ContextRuleService.cs`.
//!
//! Bağlam kurallarının uygulama tarafı: ön plan uygulaması değişince kuralları değerlendirir ve profili
//! `ProfileService` üzerinden uygular. Karar mantığı Core'daki `ContextProfileSwitcher` ve `RuleEvaluator` içindedir.
//! - Etkin kural yoksa hiçbir şeye abone olunmaz (ortak pencere izleyicisi bile alınmaz). Kurallar varsayılan KAPALI.
//! - Olay tabanlı: ön plan pencere değişimi. Titreşim önleme için yalnız bekleme sürerken çalışan tek atımlık zamanlayıcı.
//! - Döngü yok: kurallar rules.json'dadır; kendi uygulamamızın `applied` olayı yok sayılır; Koy'un kendi pencereleri ve
//!   çözülemeyen pencereler değerlendirilmez.
//! - Geri dönüş: kural profili uygulanmadan önce dokunduğu bölümlerin anlık görüntüsü alınır; eşleşme bitince geri uygulanır.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use koy_core::display::IMonitorService;
use koy_core::event::Subscription;
use koy_core::rules::{
    AutoRule, ContextProfileSwitcher, RuleContext, RuleDraft, RuleEditResult, RuleSource, RuleSources, RuleStore, SwitchActionKind,
    SwitchDecision,
};
use koy_core::settings::{ProfileApplyResult, ProfileLibrary, ProfileSection, SavedProfile};
use koy_core::shell::{IFullscreenDetector, IWindowTracker, WindowInfo};
use koy_platform::shell::{FullscreenDetector, WindowStyles, WindowTracker};

use crate::features::settings::profile_service::ProfileService;
use crate::infra::{dispatch, ui_ref::UiRef};
use crate::shared_services::SharedServices;

pub struct ContextRuleService {
    store: RuleStore,
    profiles: Rc<ProfileService>,
    switcher: RefCell<ContextProfileSwitcher>,
    debounce: slint::Timer,
    services: RefCell<Option<Rc<SharedServices>>>,
    tracker: RefCell<Option<(Arc<WindowTracker>, Subscription, u64)>>,
    rules: RefCell<Vec<AutoRule>>,
    needed: Cell<RuleSource>,
    snapshot: RefCell<Option<SavedProfile>>,
    fullscreen: RefCell<Option<FullscreenDetector>>,
    applying: Cell<bool>,
}

thread_local! {
    static INSTANCE: RefCell<Option<Rc<ContextRuleService>>> = const { RefCell::new(None) };
}

fn now() -> koy_core::time::DateTimeOffset {
    chrono::Utc::now().fixed_offset()
}

impl ContextRuleService {
    pub fn new(store: RuleStore, profiles: Rc<ProfileService>) -> Rc<Self> {
        Rc::new(ContextRuleService {
            store,
            profiles,
            switcher: RefCell::new(ContextProfileSwitcher::with_default_stable_for()),
            debounce: slint::Timer::default(),
            services: RefCell::new(None),
            tracker: RefCell::new(None),
            rules: RefCell::new(Vec::new()),
            needed: Cell::new(RuleSource::None),
            snapshot: RefCell::new(None),
            fullscreen: RefCell::new(None),
            applying: Cell::new(false),
        })
    }

    /// Uygulama genelindeki tek örnek (C# `ContextRuleService.Instance`).
    pub fn instance() -> Rc<ContextRuleService> {
        INSTANCE.with(|i| {
            i.borrow_mut()
                .get_or_insert_with(|| ContextRuleService::new(RuleStore::new(RuleStore::default_path()), ProfileService::instance()))
                .clone()
        })
    }

    /// Kuralların ham listesi (arayüz için).
    pub fn load_drafts(&self) -> Vec<RuleDraft> {
        self.store.load_drafts()
    }

    /// Uygulama açılışında bir kez. Etkin kural yoksa hiçbir şey kurulmaz.
    pub fn start(self: &Rc<Self>, services: Rc<SharedServices>) {
        *self.services.borrow_mut() = Some(services);
        self.reload();
    }

    /// Uygulama çıkarken: abonelik ve zamanlayıcı bırakılır. Ayarlara dokunulmaz.
    pub fn stop(&self) {
        self.unsubscribe();
        *self.services.borrow_mut() = None;
    }

    /// Listeyi düzenler, kaydeder ve değişikliği uygular. Kaydedilemezse `saved` false.
    pub fn edit(self: &Rc<Self>, edit: impl FnOnce(&[RuleDraft]) -> RuleEditResult) -> (RuleEditResult, bool) {
        let result = edit(&self.store.load_drafts());
        if !result.is_success() {
            return (result, false);
        }
        if !self.store.save(&result.rules) {
            return (result, false);
        }
        self.reload();
        (result, true)
    }

    fn reload(self: &Rc<Self>) {
        let rules = self.store.load_rules();
        self.needed.set(RuleSources::needed(&rules));
        self.switcher.borrow_mut().set_rules(&rules);
        *self.rules.borrow_mut() = rules;
        let services = self.services.borrow().clone();
        let Some(services) = services.filter(|_| self.needed.get() != RuleSource::None) else {
            // Etkin kural kalmadı: kullanıcının yeni durumu geçerli, geri dönülecek bir şey yok.
            self.unsubscribe();
            return;
        };
        if self.tracker.borrow().is_none() {
            let tracker = services.acquire_window_tracker();
            let ui = UiRef::new(self);
            let sub = tracker.foreground_window_changed().subscribe(move |info: &WindowInfo| {
                let info = info.clone();
                dispatch::post(move || {
                    if let Some(me) = ui.upgrade() {
                        me.observe(&info);
                    }
                });
            });
            let me = Rc::downgrade(self);
            let applied_id = self.profiles.applied.subscribe(move |_| {
                if let Some(m) = me.upgrade() {
                    m.on_profile_applied();
                }
            });
            tracker.start();
            *self.tracker.borrow_mut() = Some((tracker, sub, applied_id));
        }
    }

    fn unsubscribe(&self) {
        self.debounce.stop();
        self.switcher.borrow_mut().reset();
        *self.snapshot.borrow_mut() = None;
        *self.fullscreen.borrow_mut() = None;
        let Some((tracker, sub, applied_id)) = self.tracker.borrow_mut().take() else { return };
        // Abonelik yalnız kimlik taşır: olaydan açıkça çıkarılır (paylaşılan izleyici kural kapanınca uyarmasın).
        tracker.foreground_window_changed().unsubscribe(sub);
        self.profiles.applied.unsubscribe(applied_id);
        if let Some(s) = self.services.borrow().as_ref() {
            s.release_window_tracker();
        }
    }

    // Kullanıcı profili kendisi uyguladı: kural yürürlükten düşer, kullanıcının seçimi ezilmez.
    fn on_profile_applied(&self) {
        if self.applying.get() {
            return;
        }
        self.debounce.stop();
        self.switcher.borrow_mut().reset();
        *self.snapshot.borrow_mut() = None;
    }

    fn observe(self: &Rc<Self>, info: &WindowInfo) {
        // Koy'un kendi penceresi ve masaüstü/çözülemeyen pencere "eşleşme yok" demek değildir, durum korunur.
        if self.tracker.borrow().is_none() || info.handle == 0 || WindowStyles::get_window_process_id(info.handle) == std::process::id() {
            return;
        }
        let Some(process) = WindowStyles::get_window_process_name(info.handle) else { return };
        let needed = self.needed.get();
        let context = RuleContext::new(
            Some(&process),
            chrono::Local::now().time(),
            if needed.has_flag(RuleSource::DisplayCount) { count_displays() } else { 1 },
            needed.has_flag(RuleSource::Fullscreen) && self.is_fullscreen(),
        );
        let decision = self.switcher.borrow_mut().observe(&context, now());
        self.handle(decision);
    }

    fn handle(self: &Rc<Self>, decision: SwitchDecision) {
        match decision.kind {
            SwitchActionKind::Wait => {
                let me = Rc::downgrade(self);
                let delay = std::time::Duration::from_secs_f64(decision.delay.total_seconds().max(0.0));
                self.debounce.start(slint::TimerMode::SingleShot, delay, move || {
                    if let Some(m) = me.upgrade() {
                        let d = m.switcher.borrow_mut().tick(now());
                        m.handle(d);
                    }
                });
            }
            SwitchActionKind::Apply => {
                self.debounce.stop();
                if let Some(id) = decision.profile_id {
                    self.apply_rule(&id);
                }
            }
            SwitchActionKind::Revert => {
                self.debounce.stop();
                self.revert();
            }
            _ => self.debounce.stop(),
        }
    }

    fn apply_rule(&self, profile_id: &str) {
        // Profil silinmiş: kural etkisiz, durum değişmez.
        let Some(profile) = self.profiles.find(Some(profile_id)) else { return };
        // Geri dönüş anlık görüntüsü yalnız kural dışı durumdan kurala geçerken alınır.
        let mut new_snapshot = false;
        if !self.switcher.borrow().is_rule_active() {
            *self.snapshot.borrow_mut() = self.profiles.snapshot(&self.sections_touched_by_rules());
            new_snapshot = true;
        }
        if !self.apply_guarded(|| self.profiles.apply(Some(&profile.id))) {
            if new_snapshot {
                *self.snapshot.borrow_mut() = None;
            }
            return;
        }
        self.switcher.borrow_mut().note_applied(profile_id);
    }

    fn revert(&self) {
        let snapshot = self.snapshot.borrow_mut().take();
        if let Some(s) = snapshot {
            self.apply_guarded(|| self.profiles.apply_unsaved(&s));
        }
        self.switcher.borrow_mut().note_reverted();
    }

    fn apply_guarded(&self, apply: impl FnOnce() -> ProfileApplyResult) -> bool {
        self.applying.set(true);
        let ok = apply().is_success();
        self.applying.set(false);
        ok
    }

    // Etkin kuralların profillerinin dokunduğu bölümlerin birleşimi.
    fn sections_touched_by_rules(&self) -> Vec<ProfileSection> {
        let mut out: Vec<ProfileSection> = Vec::new();
        for rule in self.rules.borrow().iter().filter(|r| r.enabled) {
            if let Some(p) = self.profiles.find(Some(&rule.profile_id)) {
                for s in ProfileLibrary::sections_of(&p) {
                    if !out.contains(&s) {
                        out.push(s);
                    }
                }
            }
        }
        out
    }

    fn is_fullscreen(&self) -> bool {
        self.fullscreen.borrow_mut().get_or_insert_with(FullscreenDetector::default).is_fullscreen()
    }
}

fn count_displays() -> i32 {
    let monitors = koy_platform::display::MonitorService::default();
    let n = monitors.get_monitors().len() as i32;
    monitors.dispose();
    n
}
