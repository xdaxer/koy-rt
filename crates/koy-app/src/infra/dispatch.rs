//! Arayüz iş parçacığı yardımcıları (WPF `Dispatcher` karşılığı).
//!
//! Slint olay döngüsü arayüz iş parçacığıdır. `post` başka iş parçacığından gelen işi oraya taşır
//! (`Dispatcher.BeginInvoke`); `delay` tek atımlık zamanlayıcı kurar (`DispatcherTimer` + tek `Tick`).

use std::sync::Arc;
use std::time::Duration;

use koy_core::diagnostics::CrashLog;
use koy_core::search::PostFn;

type Work = Box<dyn FnOnce() + Send>;

/// Olay döngüsü dönmeye başlamadan gönderilen işler. `Some`: döngü henüz başlamadı (kuyruğa alınır); `None`: başladı.
/// winit, döngü çalışmadan `invoke_from_event_loop` ile gelen işi düşürebiliyordu (açılışta arka plan işinin sonucu,
/// ör. kapak rengi, hiç uygulanmıyordu). WPF Dispatcher kuyruğu döngü başlamadan da tutar.
static PENDING: std::sync::Mutex<Option<Vec<Work>>> = std::sync::Mutex::new(Some(Vec::new()));

/// İşi arayüz iş parçacığına gönderir. Döngü başlamadıysa kuyruğa alınır; döngü kapanmışsa iş sessizce düşer (C#'ta
/// kapanmış dispatcher gibi).
pub fn post(work: impl FnOnce() + Send + 'static) {
    {
        let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(queue) = pending.as_mut() {
            queue.push(Box::new(work));
            return;
        }
    }
    let _ = slint::invoke_from_event_loop(work);
}

/// Olay döngüsünün ilk turunda bekleyen işleri çalıştırır; sonrası doğrudan `invoke_from_event_loop` ile gider.
/// `run_event_loop` çağrısından hemen önce bir kez çağrılır.
pub fn start() {
    slint::Timer::single_shot(Duration::ZERO, || {
        let queued = PENDING.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap_or_default();
        for work in queued {
            work();
        }
    });
}

/// Core ve platform servislerine verilen `post` geri çağrısı.
pub fn post_fn() -> PostFn {
    Arc::new(|work: Box<dyn FnOnce() + Send>| post(work))
}

/// Arayüz iş parçacığında `delay` sonra bir kez çalışır. Dönen zamanlayıcı düşerse iş iptal olur.
#[must_use]
pub fn delay(after: Duration, work: impl FnOnce() + 'static) -> slint::Timer {
    let timer = slint::Timer::default();
    let mut work = Some(work);
    timer.start(slint::TimerMode::SingleShot, after, move || {
        if let Some(w) = work.take() {
            w();
        }
    });
    timer
}

/// `CrashLog.Guard`: paniği yakalar ve günlüğe yazar (arayüz geri çağrılarında süreç düşmesin).
pub fn guard(source: &str, action: impl FnOnce()) {
    CrashLog::guard(source, action);
}

/// Arayüz iş parçacığında, geçerli olay işlendikten sonra çalışır (`Send` gerektirmez; yalnız arayüz iş parçacığından
/// çağrılır). Pencere yordamı içinden pencereyi gizlemek gibi yeniden girişli işler için.
pub fn post_local(work: impl FnOnce() + 'static) {
    slint::Timer::single_shot(Duration::ZERO, work);
}
