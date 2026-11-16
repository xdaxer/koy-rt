//! CPU, bellek, ağ ve Koy belleği; hem Sistem sayfası hem Ana'daki küçük kartlar bunu paylaşır.
//! Boşta %0 CPU: servis yalnız `set_active(true)` ile alınır (çentik açık ve Ana ya da Sistem görünürken), false olunca
//! olaya abonelik kalkar ve servis bırakılır. Olaylar arka plan iş parçacığından gelir; arayüz iş parçacığına geçirilir.
//! Karşılığı: `Features/SystemInfo/SystemStatsViewModel.cs`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use koy_core::event::Subscription;
use koy_core::system_stats::{ISystemStatsService, SystemFormat, SystemSnapshot, SystemSnapshotMerge};
use koy_core::units::NetworkSpeedUnit;

use crate::infra::dispatch;
use crate::infra::observable::Notifier;
use crate::infra::ui_ref::UiRef;
use crate::resources::strings::{self, Strings};

const NO_VALUE: &str = "–";

#[derive(Debug, Clone, Default)]
pub struct SystemStatsState {
    pub cpu_progress: f64,
    pub cpu_text: String,
    /// "En çok: Chrome"; süreç henüz bilinmiyorsa boş.
    pub top_process_text: String,
    pub ram_progress: f64,
    pub ram_text: String,
    /// "9,2 / 16 GB".
    pub memory_detail_text: String,
    pub network_name: String,
    pub download_text: String,
    pub upload_text: String,
    /// Koy'un özel çalışma kümesi ("64 MB").
    pub koy_memory_text: String,
}

pub type AcquireStats = Box<dyn Fn() -> Arc<dyn ISystemStatsService>>;

pub struct SystemStatsViewModel {
    acquire: AcquireStats,
    release: Box<dyn Fn()>,
    network_unit: NetworkSpeedUnit,
    service: RefCell<Option<(Arc<dyn ISystemStatsService>, Subscription)>>,
    disposed: Cell<bool>,
    // Servis her görünüşte yeniden kurulur ve ilk örneğe kadar boştur; son tam örnek burada kalır.
    last_known: RefCell<Option<SystemSnapshot>>,
    state: RefCell<SystemStatsState>,
    pub changed: Notifier,
}

impl SystemStatsViewModel {
    /// `acquire`: servisi alır (SharedServices); `release`: bırakır. `network_unit`: ağ hızı birimi.
    pub fn new(acquire: AcquireStats, release: Box<dyn Fn()>, network_unit: NetworkSpeedUnit) -> Rc<Self> {
        let vm = Rc::new(SystemStatsViewModel {
            acquire,
            release,
            network_unit,
            service: RefCell::new(None),
            disposed: Cell::new(false),
            last_known: RefCell::new(None),
            state: RefCell::new(SystemStatsState::default()),
            changed: Notifier::default(),
        });
        vm.apply(&SystemSnapshot::empty());
        vm
    }

    pub fn state(&self) -> std::cell::Ref<'_, SystemStatsState> {
        self.state.borrow()
    }

    /// Servis şu an alınmış mı.
    pub fn is_active(&self) -> bool {
        self.service.borrow().is_some()
    }

    /// Görünürlüğe göre servisi alır ya da bırakır (idempotent).
    pub fn set_active(self: &Rc<Self>, active: bool) {
        if self.disposed.get() || active == self.is_active() {
            return;
        }
        if active {
            let service = (self.acquire)();
            let me = UiRef::new(self);
            let sub = service.updated().subscribe(move |s: &SystemSnapshot| {
                let s = s.clone();
                dispatch::post(move || {
                    // Bırakıldıktan sonra kuyruktaki olay gelirse yok sayılır.
                    if let Some(vm) = me.upgrade()
                        && !vm.disposed.get()
                        && vm.is_active()
                    {
                        vm.apply(&s);
                        vm.changed.notify();
                    }
                });
            });
            let current = service.current();
            *self.service.borrow_mut() = Some((service, sub));
            self.apply(&current);
            self.changed.notify();
        } else {
            self.deactivate();
        }
    }

    fn deactivate(&self) {
        let Some((service, sub)) = self.service.borrow_mut().take() else { return };
        service.updated().unsubscribe(sub);
        drop(service);
        (self.release)();
    }

    fn apply(&self, raw: &SystemSnapshot) {
        let s = SystemSnapshotMerge::merge(raw.clone(), self.last_known.borrow().as_ref());
        if raw.rates_ready {
            *self.last_known.borrow_mut() = Some(raw.clone());
        }
        // "–" yalnız hiç değer yokken.
        let has_data = SystemSnapshotMerge::has_instant_data(&s);
        let has_rates = has_data && s.rates_ready;
        let c = strings::culture();
        let mut st = self.state.borrow_mut();
        st.cpu_progress = s.cpu_percent.clamp(0.0, 100.0) / 100.0;
        st.cpu_text = if has_rates { SystemFormat::percent(s.cpu_percent, &c) } else { NO_VALUE.into() };
        st.top_process_text = match s.top_process_name.as_deref() {
            Some(n) if !n.trim().is_empty() => Strings::format("SystemTopProcess", &[n]),
            _ => String::new(),
        };
        st.ram_progress = s.ram_percent.clamp(0.0, 100.0) / 100.0;
        st.ram_text = if has_data { SystemFormat::percent(s.ram_percent, &c) } else { NO_VALUE.into() };
        st.memory_detail_text = if has_data { SystemFormat::memory(s.ram_used_bytes, s.ram_total_bytes, &c) } else { String::new() };
        st.network_name = match s.network_name.as_deref() {
            Some(n) if !n.trim().is_empty() => n.to_string(),
            _ => Strings::get("SystemNoNetwork"),
        };
        let speed = |v: i64| if has_rates { SystemFormat::speed_with_unit(v, self.network_unit, &c) } else { NO_VALUE.into() };
        st.download_text = format!("↓ {}", speed(s.net_down_bytes_per_sec));
        st.upload_text = format!("↑ {}", speed(s.net_up_bytes_per_sec));
        st.koy_memory_text = if s.koy_memory_bytes > 0 { SystemFormat::size(s.koy_memory_bytes, &c) } else { NO_VALUE.into() };
    }

    pub fn dispose(&self) {
        if self.disposed.get() {
            return;
        }
        self.deactivate();
        self.disposed.set(true);
    }
}
