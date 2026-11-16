//! Arayüz iş parçacığı yardımcıları (WPF `Dispatcher` karşılığı).
//!
//! Slint olay döngüsü arayüz iş parçacığıdır. `post` başka iş parçacığından gelen işi oraya taşır
//! (`Dispatcher.BeginInvoke`); `delay` tek atımlık zamanlayıcı kurar (`DispatcherTimer` + tek `Tick`).

use std::sync::Arc;
use std::time::Duration;

use koy_core::diagnostics::CrashLog;
use koy_core::search::PostFn;

/// İşi arayüz iş parçacığına gönderir. Olay döngüsü kapanmışsa iş sessizce düşer (C#'ta kapanmış dispatcher gibi).
pub fn post(work: impl FnOnce() + Send + 'static) {
    let _ = slint::invoke_from_event_loop(work);
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
