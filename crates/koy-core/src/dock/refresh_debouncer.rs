use std::sync::{Arc, Mutex, Weak};

use crate::time::{SystemTimeProvider, TimeProvider, TimeSpan, Timer};

/// `post` geri çağrısının (ör. UI dispatcher'ı) işlediği iş.
pub type PostedWork = Box<dyn FnOnce() + Send + 'static>;

struct State {
    timer: Option<Box<dyn Timer>>,
    pending: bool,
    disposed: bool,
}

struct Shared {
    action: Arc<dyn Fn() + Send + Sync>,
    delay: TimeSpan,
    post: Option<Arc<dyn Fn(PostedWork) + Send + Sync>>,
    time_provider: Arc<dyn TimeProvider>,
    gate: Mutex<State>,
}

/// Sık gelen "yenile" isteklerini tek bir yenilemeye indirger. İlk istekten `delay` sonra eylem bir kez çalışır;
/// bu aralıktaki diğer istekler birleşir (fırtına sırasında hiç çalışmama riski yok: ilk istek süreyi belirler).
/// Eylem `post` ile (ör. UI dispatcher'ı) çalıştırılır; zamanlayıcı ThreadPool'dan tikler.
pub struct RefreshDebouncer {
    shared: Arc<Shared>,
}

impl RefreshDebouncer {
    /// `time_provider` verilmezse `TimeProvider.System` (gerçek saat).
    pub fn new(
        action: Arc<dyn Fn() + Send + Sync>,
        delay: TimeSpan,
        post: Option<Arc<dyn Fn(PostedWork) + Send + Sync>>,
        time_provider: Option<Arc<dyn TimeProvider>>,
    ) -> Self {
        let time_provider: Arc<dyn TimeProvider> = time_provider.unwrap_or_else(|| Arc::new(SystemTimeProvider));
        RefreshDebouncer {
            shared: Arc::new(Shared {
                action,
                delay,
                post,
                time_provider,
                gate: Mutex::new(State { timer: None, pending: false, disposed: false }),
            }),
        }
    }

    pub fn is_pending(&self) -> bool {
        self.shared.gate.lock().unwrap().pending
    }

    pub fn request(&self) {
        let old;
        {
            let mut g = self.shared.gate.lock().unwrap();
            if g.disposed || g.pending {
                return;
            }

            g.pending = true;
            let weak: Weak<Shared> = Arc::downgrade(&self.shared);
            let timer = self.shared.time_provider.create_timer(
                Box::new(move || {
                    if let Some(shared) = weak.upgrade() {
                        Shared::on_elapsed(&shared);
                    }
                }),
                self.shared.delay,
                TimeSpan::INFINITE,
            );
            old = g.timer.replace(timer);
        }
        // Eski zamanlayıcı kilit dışında bırakılır (kendi geri çağrısı kilit bekliyor olabilir).
        drop(old);
    }

    /// Bekleyen isteği iptal eder (ör. yenilemeyi zaten elle yaptıysak).
    pub fn cancel(&self) {
        let old;
        {
            let mut g = self.shared.gate.lock().unwrap();
            g.pending = false;
            old = g.timer.take();
        }
        drop(old);
    }

    pub fn dispose(&self) {
        let old;
        {
            let mut g = self.shared.gate.lock().unwrap();
            g.disposed = true;
            g.pending = false;
            old = g.timer.take();
        }
        drop(old);
    }
}

impl Shared {
    fn on_elapsed(this: &Arc<Shared>) {
        {
            let mut g = this.gate.lock().unwrap();
            if g.disposed || !g.pending {
                return;
            }

            g.pending = false;
        }

        match &this.post {
            None => (this.action)(),
            Some(post) => {
                let shared = Arc::clone(this);
                post(Box::new(move || {
                    {
                        let g = shared.gate.lock().unwrap();
                        if g.disposed {
                            return;
                        }
                    }
                    (shared.action)();
                }));
            }
        }
    }
}

impl Drop for RefreshDebouncer {
    fn drop(&mut self) {
        self.dispose();
    }
}
