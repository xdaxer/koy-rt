use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use super::notification_diff::{NotificationDiff, NotificationSnapshot};
use super::notification_item::NotificationItem;
use crate::cancel::{CancellationToken, CancellationTokenSource};
use crate::event::Event;
use crate::time::{DateTimeOffset, SystemTimeProvider, TimeProvider, TimeSpan, Timer};

/// Okuma işleminin hatası. `Canceled` C#'taki `OperationCanceledException` (döngü sessizce biter),
/// `Failed` öteki her istisna (döngü [`NotificationPoller::FAILURE_INTERVAL`] sonra yeniden dener).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationReadError {
    Canceled,
    Failed(String),
}

impl fmt::Display for NotificationReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotificationReadError::Canceled => f.write_str("The operation was canceled."),
            NotificationReadError::Failed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for NotificationReadError {}

/// Listeyi okur. `known` `None` ise temel çizgi okumasıdır: kaynak yalnız kimlikleri döner, ayrıntı üretmez.
/// Aksi halde en azından `known` içinde olmayan ya da zamanı ilerlemiş kimliklerin ayrıntısını verir.
/// (C#: `Func<IReadOnlyDictionary<uint, DateTimeOffset>?, CancellationToken, Task<NotificationSnapshot>>`;
/// burada bloklayan çağrı, yoklama arka plan iş parçacığında yürür.)
pub type NotificationReader = Box<
    dyn Fn(Option<&HashMap<u32, DateTimeOffset>>, &CancellationToken) -> Result<NotificationSnapshot, NotificationReadError> + Send + Sync,
>;

/// Bir çalıştırma (Start..Stop arası): kendi iptal kaynağı ve bekleme zamanlayıcısı.
struct Run {
    id: u64,
    cts: CancellationTokenSource,
    timer: Option<Box<dyn Timer>>,
}

struct State {
    run: Option<Run>,
    next_run_id: u64,
}

/// Zamanlayıcı ve başlatma iş parçacığıyla paylaşılan çekirdek (`Arc` + `Weak` kalıbı, PORTING.md #10).
/// Olay (`arrived`) kilit dışında tetiklenir.
struct Shared {
    read: NotificationReader,
    clock: Arc<dyn TimeProvider>,
    arrived: Event<NotificationItem>,
    state: Mutex<State>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }

    /// Tek yoklama; yeni gelenleri `arrived` ile bildirir.
    fn poll_once(&self, diff: &mut NotificationDiff, token: &CancellationToken) -> Result<(), NotificationReadError> {
        let snapshot = (self.read)(if diff.has_baseline() { Some(diff.known()) } else { None }, token)?;
        if token.is_cancellation_requested() {
            return Ok(());
        }

        for item in diff.apply(&snapshot) {
            self.arrived.invoke(&item);
        }
        Ok(())
    }

    /// Döngünün bir turu: yokla, sonra bekleme zamanlayıcısını kur. C#'taki `while` döngüsü burada
    /// tek atımlık zamanlayıcının kendini yeniden kurmasıdır; bir yoklama bitmeden sonraki başlamaz.
    fn step(weak: &Weak<Shared>, run_id: u64, token: &CancellationToken, diff: &Arc<Mutex<NotificationDiff>>) {
        let Some(shared) = weak.upgrade() else { return };
        if token.is_cancellation_requested() {
            return;
        }

        let result = shared.poll_once(&mut diff.lock().unwrap(), token);
        let delay = match result {
            Ok(()) => NotificationPoller::INTERVAL,
            Err(NotificationReadError::Canceled) => return,
            Err(NotificationReadError::Failed(_)) => NotificationPoller::FAILURE_INTERVAL,
        };

        let mut state = shared.lock();
        let Some(run) = state.run.as_mut().filter(|r| r.id == run_id) else { return };
        if token.is_cancellation_requested() {
            return;
        }

        match run.timer.as_mut() {
            Some(timer) => {
                timer.change(delay, TimeSpan::INFINITE);
            }
            None => {
                let (weak, token, diff) = (weak.clone(), token.clone(), Arc::clone(diff));
                run.timer = Some(shared.clock.create_timer(
                    Box::new(move || Shared::step(&weak, run_id, &token, &diff)),
                    delay,
                    TimeSpan::INFINITE,
                ));
            }
        }
    }
}

/// Windows bildirim listesini sabit aralıkla yoklar ve yeni gelenleri [`NotificationPoller::arrived`] ile bildirir.
/// Paketsiz süreçte `NotificationChanged` olayı kurulamadığı için tek yol budur (bkz. AGENTS #5 istisnası).
/// Döngü tek görevdir: bir yoklama bitmeden sonraki başlamaz, iş arka planda yürür (UI iş parçacığı değil).
/// Zamanlayıcı yalnız [`NotificationPoller::start`] ile kurulur, [`NotificationPoller::stop`] ile sökülür.
///
/// C#'taki `Task.Run` + `Task.Delay(_clock)` döngüsü: ilk yoklama `start`'ın açtığı arka plan iş parçacığında,
/// sonrakiler `TimeProvider` zamanlayıcısının geri çağrısında (tek atımlık, her turdan sonra yeniden kurulur).
pub struct NotificationPoller {
    shared: Arc<Shared>,
}

impl NotificationPoller {
    /// Yoklama aralığı. 2 sn: Windows'un köşe başlığı birkaç sn görünür, bildirim kartı en çok 2 sn gecikmeyle çıkar;
    /// tek bir WinRT liste çağrısı olduğundan CPU maliyeti çok küçük. Daha sık yoklamak kazanç getirmez, daha seyrek
    /// yoklamak bildirimi geç gösterir.
    pub const INTERVAL: TimeSpan = TimeSpan::from_secs(2);

    /// Okuma hata verirse (izin kalktı, Windows servisi yanıtsız) bir sonraki deneme bu kadar sonra.
    pub const FAILURE_INTERVAL: TimeSpan = TimeSpan::from_secs(10);

    /// `clock` verilmezse `TimeProvider.System`.
    pub fn new(read: NotificationReader, clock: Option<Arc<dyn TimeProvider>>) -> Self {
        NotificationPoller {
            shared: Arc::new(Shared {
                read,
                clock: clock.unwrap_or_else(|| Arc::new(SystemTimeProvider)),
                arrived: Event::new(),
                state: Mutex::new(State { run: None, next_run_id: 0 }),
            }),
        }
    }

    /// Yeni bildirim geldi; arka plan iş parçacığından tetiklenir.
    pub fn arrived(&self) -> &Event<NotificationItem> {
        &self.shared.arrived
    }

    pub fn is_running(&self) -> bool {
        self.shared.lock().run.is_some()
    }

    pub fn start(&self) {
        let (run_id, token) = {
            let mut state = self.shared.lock();
            if state.run.is_some() {
                return;
            }

            state.next_run_id += 1;
            let id = state.next_run_id;
            let cts = CancellationTokenSource::new();
            let token = cts.token();
            state.run = Some(Run { id, cts, timer: None });
            (id, token)
        };

        // Her çalıştırmanın kendi farkı var: yeniden başlatmada ilk okuma yine temel çizgidir.
        let diff = Arc::new(Mutex::new(NotificationDiff::new()));
        let weak = Arc::downgrade(&self.shared);
        std::thread::Builder::new()
            .name("koy-notification-poll".into())
            .spawn(move || Shared::step(&weak, run_id, &token, &diff))
            .expect("bildirim yoklama iş parçacığı");
    }

    pub fn stop(&self) {
        let run = self.shared.lock().run.take();
        let Some(run) = run else { return };

        run.cts.cancel();
        // Zamanlayıcı düşürülünce sökülür (kilit dışında).
        drop(run);
    }

    /// Tek yoklama; yeni gelenleri [`NotificationPoller::arrived`] ile bildirir. Testlerde doğrudan çağrılır.
    pub fn poll_once(&self, diff: &mut NotificationDiff, token: &CancellationToken) -> Result<(), NotificationReadError> {
        self.shared.poll_once(diff, token)
    }
}

impl Drop for NotificationPoller {
    fn drop(&mut self) {
        self.stop();
    }
}
