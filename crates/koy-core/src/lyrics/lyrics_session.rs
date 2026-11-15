//! Karşılığı: `Lyrics/LyricsSession.cs`.

use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;

use super::cancellation::{CancellationTokenSource, LyricsError, OperationCanceled};
use super::lyric_line::LyricLine;
use super::lyrics_provider::{ILyricsProvider, LyricsLoadResult, LyricsLoadStatus};
use super::lyrics_query::LyricsQuery;
use super::lyrics_timeline::LyricsTimeline;
use crate::diagnostics::{CrashError, CrashLog};
use crate::event::Event;
use crate::time::TimeSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LyricsState {
    Idle,
    Loading,
    Ready,
    NotFound,
    Failed,
}

/// `LyricsSession.LoadAsync`'in döndürdüğü `Task`'ın karşılığı: yükleme işini yürüten iş parçacığı.
/// `wait()` C#'taki `await load` gibi biter; beklemeden bırakmak `_ = session.LoadAsync(...)` demektir (iş sürer).
pub struct LoadTask {
    handle: Option<JoinHandle<()>>,
}

impl LoadTask {
    fn completed() -> Self {
        LoadTask { handle: None }
    }

    /// Yükleme bitene (ya da iptal / eskimiş sonuç olarak bırakılana) kadar bekler.
    pub fn wait(self) {
        if let Some(handle) = self.handle {
            let _ = handle.join();
        }
    }
}

type ErrorHandler = Box<dyn Fn(&str, &LyricsError) + Send + Sync>;

struct Data {
    state: LyricsState,
    lines: Vec<LyricLine>,
    active_index: i32,
    /// Süren yükleme: (kimlik, kaynak). C#'taki `_load` ve `ReferenceEquals` denetiminin karşılığı kimliktir.
    load: Option<(u64, CancellationTokenSource)>,
    next_load_id: u64,
    key: Option<String>,
}

struct Inner {
    provider: Arc<dyn ILyricsProvider>,
    on_error: ErrorHandler,
    data: Mutex<Data>,
    changed: Event<()>,
}

/// Bir görünümün söz durumu: hangi parça yüklü, hangi satır etkin. Arayüz iş parçacığından çağrılır; dosya ve ağ işi
/// arka plana alınır. Parça hızlı değişince eski istek iptal edilir, geç gelen eski sonuç yeni parçanın üzerine yazmaz.
///
/// Eşzamansızlık: C#'ta `await Task.Run(...)` sonrası devam, arayüz bağlamında çalışır. Burada yükleme bir iş
/// parçacığında yürür ve sonucu aynı kilit altında uygular; `changed` olayı kilit bırakıldıktan sonra tetiklenir
/// (işleyici o iş parçacığında çalışır).
pub struct LyricsSession {
    inner: Arc<Inner>,
}

impl LyricsSession {
    /// `new LyricsSession(provider)`: hata günlüğü `CrashLog.Write`.
    pub fn new(provider: Arc<dyn ILyricsProvider>) -> Self {
        Self::with_error_handler(provider, |source, error| {
            let crash = CrashError::from_error(error.as_ref(), "System.Exception", "");
            CrashLog::write(source, &crash);
        })
    }

    /// `new LyricsSession(provider, onError)`.
    pub fn with_error_handler(
        provider: Arc<dyn ILyricsProvider>,
        on_error: impl Fn(&str, &LyricsError) + Send + Sync + 'static,
    ) -> Self {
        LyricsSession {
            inner: Arc::new(Inner {
                provider,
                on_error: Box::new(on_error),
                data: Mutex::new(Data {
                    state: LyricsState::Idle,
                    lines: Vec::new(),
                    active_index: -1,
                    load: None,
                    next_load_id: 0,
                    key: None,
                }),
                changed: Event::new(),
            }),
        }
    }

    pub fn state(&self) -> LyricsState {
        self.inner.lock().state
    }

    pub fn lines(&self) -> Vec<LyricLine> {
        self.inner.lock().lines.clone()
    }

    pub fn active_index(&self) -> i32 {
        self.inner.lock().active_index
    }

    /// Durum ya da satır listesi değişti (etkin satır değişimi bunu tetiklemez).
    pub fn changed(&self) -> &Event<()> {
        &self.inner.changed
    }

    /// Parçayı yükler. Aynı parça zaten yüklü ya da yükleniyorsa bir şey yapmaz; hata sonrası yeniden dener.
    /// Durum `Loading`'e hemen (çağıran iş parçacığında) geçer; yükleme arka planda sürer.
    pub fn load(&self, query: Option<&LyricsQuery>) -> LoadTask {
        let Some(query) = query else {
            self.clear();
            return LoadTask::completed();
        };

        let (id, source) = {
            let mut data = self.inner.lock();
            let key = query.key();
            if data.key.as_deref() == Some(key.as_str())
                && matches!(data.state, LyricsState::Loading | LyricsState::Ready | LyricsState::NotFound)
            {
                return LoadTask::completed();
            }

            cancel_load(&mut data);
            let source = CancellationTokenSource::new();
            data.next_load_id += 1;
            let id = data.next_load_id;
            data.load = Some((id, source.clone()));
            data.key = Some(key);
            apply(&mut data, LyricsState::Loading, Vec::new());
            (id, source)
        };
        self.inner.changed.invoke(&());

        let inner = Arc::clone(&self.inner);
        let query = query.clone();
        let handle = std::thread::spawn(move || inner.run_load(query, id, source));
        LoadTask { handle: Some(handle) }
    }

    /// Görünüm gizlendi: süren yükleme iptal edilir, biten sonuç saklanır (geri açılınca yeniden indirilmez).
    pub fn suspend(&self) {
        {
            let mut data = self.inner.lock();
            if data.state != LyricsState::Loading {
                return;
            }

            cancel_load(&mut data);
            data.key = None;
            apply(&mut data, LyricsState::Idle, Vec::new());
        }
        self.inner.changed.invoke(&());
    }

    pub fn clear(&self) {
        {
            let mut data = self.inner.lock();
            cancel_load(&mut data);
            data.key = None;
            apply(&mut data, LyricsState::Idle, Vec::new());
        }
        self.inner.changed.invoke(&());
    }

    /// Etkin satırı konuma göre günceller. Değiştiyse true.
    pub fn update_position(&self, position: TimeSpan) -> bool {
        let mut data = self.inner.lock();
        let index = LyricsTimeline::index_at(&data.lines, position);
        if index == data.active_index {
            return false;
        }

        data.active_index = index;
        true
    }
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, Data> {
        self.data.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `LoadAsync`'in `await` sonrası kısmı (arka plan iş parçacığında).
    fn run_load(&self, query: LyricsQuery, id: u64, source: CancellationTokenSource) {
        let token = source.token();
        // Task.Run(..., token): iptal önceden istenmişse iş hiç başlamaz (OperationCanceledException).
        let outcome: Result<LyricsLoadResult, LyricsError> = if token.is_cancellation_requested() {
            Err(Box::new(OperationCanceled))
        } else {
            match panic::catch_unwind(AssertUnwindSafe(|| self.provider.load(&query, &token))) {
                Ok(r) => r,
                // Panik, C#'taki istisna gibi Failed durumuna çevrilir (yoksa oturum sonsuza dek Loading kalırdı).
                Err(payload) => {
                    let message = if let Some(s) = payload.downcast_ref::<&str>() {
                        (*s).to_string()
                    } else if let Some(s) = payload.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "panic".to_string()
                    };
                    Err(message.into())
                }
            }
        };

        let result = match outcome {
            Ok(r) => r,
            Err(e) if e.is::<OperationCanceled>() && source.is_cancellation_requested() => return,
            Err(e) => {
                (self.on_error)("Şarkı sözü yüklenemedi", &e);
                LyricsLoadResult::failed()
            }
        };

        {
            let mut data = self.lock();
            // Bu sırada başka parça başladıysa sonuç artık geçersiz.
            if data.load.as_ref().map(|l| l.0) != Some(id) {
                return;
            }

            data.load = None;
            let state = match result.status {
                LyricsLoadStatus::Found => LyricsState::Ready,
                LyricsLoadStatus::NotFound => LyricsState::NotFound,
                _ => LyricsState::Failed,
            };
            apply(&mut data, state, result.lines);
        }
        self.changed.invoke(&());
    }
}

// Kaynak önce boşaltılır: iptal geri çağrıları eşzamanlı çalışabilir, bu istek artık "güncel" sayılmamalı.
fn cancel_load(data: &mut Data) {
    if let Some((_, source)) = data.load.take() {
        source.cancel();
    }
}

fn apply(data: &mut Data, state: LyricsState, lines: Vec<LyricLine>) {
    data.state = state;
    data.lines = lines;
    data.active_index = -1;
}
