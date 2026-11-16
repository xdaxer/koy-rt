//! Etkinlik defterinin tek örneği: bütün monitörlerdeki çentikler aynı listeyi görür. Dosya ilk ihtiyaçta okunur
//! (takvim ya da ana görünüm ilk açılınca), açılışta disk işi yapılmaz. Yerel değişiklik hemen diske yazılır; harici
//! Google/Outlook etkinlikleri çevrimdışı önbellekten yüklenir ve istendiğinde eşitlenir. Yalnız arayüz iş
//! parçacığından kullanılır. Karşılığı: `Features/Calendar/SharedEvents.cs` ve `BuiltInCalendarClients.cs`.

use std::cell::{Cell, RefCell, RefMut};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{Duration, Local, NaiveDate, NaiveTime, Utc};
use koy_core::calendar::{
    CalendarCacheStore, CalendarEvent, CalendarEventSource, CalendarOAuthConfig, CalendarProvider, CalendarSettings, EventBook, EventStore,
    ICalendarSyncService, OAuthBuiltInClients, OAuthClient,
};
use koy_core::cancel::CancellationToken;
use koy_platform::calendar::CalendarSyncService;

use crate::infra::dispatch;
use crate::infra::observable::Notifier;
use crate::infra::ui_ref::UiRef;

/// Bitişi bu kadar günden eskiyse yerel etkinlik silinir (liste şişmesin).
const KEEP_DAYS: i64 = 60;

/// Sürüme gömülü OAuth istemcileri: derlemede `KOY_BUILD_GOOGLE_CLIENT_ID`, `KOY_BUILD_GOOGLE_CLIENT_SECRET` ve
/// `KOY_BUILD_OUTLOOK_CLIENT_ID` ortam değişkenlerinden okunur; yerel derlemede boştur.
pub fn built_in_clients() -> OAuthBuiltInClients {
    OAuthBuiltInClients {
        google_client_id: option_env!("KOY_BUILD_GOOGLE_CLIENT_ID").unwrap_or("").trim().to_string(),
        google_client_secret: option_env!("KOY_BUILD_GOOGLE_CLIENT_SECRET").unwrap_or("").trim().to_string(),
        outlook_client_id: option_env!("KOY_BUILD_OUTLOOK_CLIENT_ID").unwrap_or("").trim().to_string(),
    }
}

/// Ayarlardaki kimlik, ortam değişkeni ya da yerleşik istemciden kullanılacak olanı seçer.
pub fn resolve_client(provider: CalendarProvider, settings: &CalendarSettings) -> OAuthClient {
    CalendarOAuthConfig::resolve_client(provider, settings, Some(&built_in_clients()), None)
}

struct BookFlags {
    local: Arc<AtomicBool>,
    any: Arc<AtomicBool>,
}

pub struct SharedEvents {
    store: EventStore,
    cache_store: CalendarCacheStore,
    sync: Arc<CalendarSyncService>,
    book: RefCell<Option<EventBook>>,
    flags: BookFlags,
    syncing: Cell<bool>,
    /// Etkinlik eklendi, silindi ya da harici takvim eşitlendi.
    pub changed: Notifier,
}

impl SharedEvents {
    pub fn new() -> Rc<Self> {
        Rc::new(SharedEvents {
            store: EventStore::new(EventStore::default_path()),
            cache_store: CalendarCacheStore::new(None),
            sync: Arc::new(CalendarSyncService::new(None, Some(CalendarCacheStore::new(None)))),
            book: RefCell::new(None),
            flags: BookFlags { local: Arc::new(AtomicBool::new(false)), any: Arc::new(AtomicBool::new(false)) },
            syncing: Cell::new(false),
            changed: Notifier::default(),
        })
    }

    /// Defter (ilk erişimde diskten okunur).
    pub fn book(&self) -> RefMut<'_, EventBook> {
        let mut slot = self.book.borrow_mut();
        if slot.is_none() {
            let mut book = EventBook::new(Some(self.store.load()));
            book.prune_before(Local::now().date_naive() - Duration::days(KEEP_DAYS));
            Self::load_external_cache_into(&self.cache_store, &mut book);
            let local = Arc::clone(&self.flags.local);
            book.local_changed().subscribe(move |_| local.store(true, Ordering::Relaxed));
            let any = Arc::clone(&self.flags.any);
            book.changed().subscribe(move |_| any.store(true, Ordering::Relaxed));
            *slot = Some(book);
        }
        RefMut::map(slot, |b| b.as_mut().expect("defter kuruldu"))
    }

    /// Defterdeki bütün etkinlikler (kopya).
    pub fn events(&self) -> Vec<CalendarEvent> {
        self.book().events().to_vec()
    }

    /// Defter değişti mi: yerel değişiklik diske yazılır, sonra abonelere haber verilir (C# olay işleyicileri).
    fn after_mutation(&self) {
        if self.flags.local.swap(false, Ordering::Relaxed) {
            let local = self.book().local_events().to_vec();
            if let Err(e) = self.store.save(&local) {
                // Disk dolu ya da dosya kilitli: etkinlik bu oturumda bellekte kalır, uygulama çökmez.
                koy_core::diagnostics::CrashLog::write("Takvim etkinlikleri kaydedilemedi", &koy_core::diagnostics::CrashError::from_error(&e, "IOException", ""));
            }
        }
        if self.flags.any.swap(false, Ordering::Relaxed) {
            self.changed.notify();
        }
    }

    pub fn add(&self, title: &str, day: NaiveDate, time: NaiveTime) -> Option<CalendarEvent> {
        let r = self.book().add(Some(title), day, time);
        self.after_mutation();
        r
    }

    pub fn remove(&self, id: &str) -> bool {
        let r = self.book().remove(id);
        self.after_mutation();
        r
    }

    /// Önbellekteki son bilinen harici etkinlikleri deftere yeniden yükler.
    pub fn reload_external_from_cache(&self) {
        if self.book.borrow().is_none() {
            return;
        }
        {
            let mut book = self.book();
            book.clear_external(None);
            Self::load_external_cache_into(&self.cache_store, &mut book);
        }
        self.after_mutation();
    }

    fn load_external_cache_into(cache: &CalendarCacheStore, book: &mut EventBook) {
        let snap = cache.load();
        if snap.events.is_empty() {
            return;
        }
        for source in [CalendarEventSource::Google, CalendarEventSource::Outlook] {
            let list: Vec<CalendarEvent> = snap.events.iter().filter(|e| e.source == source).cloned().collect();
            if !list.is_empty() {
                book.replace_external(source, &list);
            }
        }
    }

    /// Bağlı Google / Outlook hesaplarını salt okunur eşitler (arka planda); çevrimdışıysa önbellekle devam eder.
    pub fn sync_external(self: &Rc<Self>, settings: CalendarSettings) {
        if self.syncing.get() || (!settings.google_enabled && !settings.outlook_enabled) {
            return;
        }
        self.syncing.set(true);
        let sync = Arc::clone(&self.sync);
        let me = UiRef::new(self);
        std::thread::spawn(move || {
            let now = Utc::now().fixed_offset();
            let (from, to) = (now - Duration::days(14), now + Duration::days(60));
            let mut results = Vec::new();
            for (enabled, provider, source) in [
                (settings.google_enabled, CalendarProvider::Google, CalendarEventSource::Google),
                (settings.outlook_enabled, CalendarProvider::Outlook, CalendarEventSource::Outlook),
            ] {
                if enabled && sync.has_token(provider) {
                    let client = resolve_client(provider, &settings);
                    let result = sync.sync(provider, &client, from, to, &CancellationToken::none());
                    if !result.events.is_empty() || result.success {
                        results.push((source, result.events));
                    }
                }
            }
            dispatch::post(move || {
                let Some(me) = me.upgrade() else { return };
                for (source, events) in results {
                    me.book().replace_external(source, &events);
                    me.after_mutation();
                }
                me.syncing.set(false);
            });
        });
    }
}
