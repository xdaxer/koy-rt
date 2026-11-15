//! Wi-Fi ve Bluetooth radyoları (Windows.Devices.Radios): durum, açma/kapatma ve durum değişikliği olayı.

use std::sync::{Arc, Condvar, Mutex, Weak};

use koy_core::diagnostics::{CrashError, CrashLog};
use koy_core::event::Event;
use koy_core::radio::{IRadioService, RadioAccess, RadioKind, RadioState, RadioStateChangedEventArgs};
use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind as WinRadioKind, RadioState as WinRadioState};
use windows::Foundation::TypedEventHandler;

use crate::calendar::open_url;

fn log(source: &str, e: &windows::core::Error) {
    CrashLog::write(source, &CrashError::exception("System.Exception", &e.message(), ""));
}

struct Bound {
    radio: Radio,
    token: i64,
}

struct State {
    access: RadioAccess,
    wifi: Option<Bound>,
    bluetooth: Option<Bound>,
    scanned: bool,
    disposed: bool,
    refreshing: bool,
}

struct Inner {
    state: Mutex<State>,
    refresh_done: Condvar,
    state_changed: Event<RadioStateChangedEventArgs>,
}

pub struct RadioService {
    inner: Arc<Inner>,
}

impl Default for RadioService {
    fn default() -> Self {
        RadioService {
            inner: Arc::new(Inner {
                state: Mutex::new(State { access: RadioAccess::Unknown, wifi: None, bluetooth: None, scanned: false, disposed: false, refreshing: false }),
                refresh_done: Condvar::new(),
                state_changed: Event::new(),
            }),
        }
    }
}

fn map_state(state: WinRadioState) -> RadioState {
    match state {
        WinRadioState::On => RadioState::On,
        WinRadioState::Off => RadioState::Off,
        WinRadioState::Disabled => RadioState::Disabled,
        _ => RadioState::Unknown,
    }
}

fn map_access(status: RadioAccessStatus) -> RadioAccess {
    match status {
        RadioAccessStatus::Allowed => RadioAccess::Allowed,
        RadioAccessStatus::DeniedByUser => RadioAccess::DeniedByUser,
        RadioAccessStatus::DeniedBySystem => RadioAccess::DeniedBySystem,
        _ => RadioAccess::Unknown,
    }
}

impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn get_state(scanned: bool, radio: Option<&Radio>) -> RadioState {
        match radio {
            None => {
                if scanned {
                    RadioState::NotPresent
                } else {
                    RadioState::Unknown
                }
            }
            // Adaptör çıkarıldıysa eski nesne hata verir.
            Some(r) => r.State().map(map_state).unwrap_or(RadioState::NotPresent),
        }
    }

    fn current(&self, kind: RadioKind) -> RadioState {
        let st = self.lock();
        let bound = if kind == RadioKind::WiFi { st.wifi.as_ref() } else { st.bluetooth.as_ref() };
        Self::get_state(st.scanned, bound.map(|b| &b.radio))
    }

    // Abonenin paniği tarama işini ya da WinRT iş parçacığını bozmasın.
    fn raise(&self, kind: RadioKind, state: RadioState) {
        let args = RadioStateChangedEventArgs::new(kind, state);
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.state_changed.invoke(&args))).is_err() {
            CrashLog::write("RadioService.StateChanged abonesi", &CrashError::exception("System.Exception", "subscriber panicked", ""));
        }
    }
}

/// Her taramada yeni WinRT nesneleri gelir; eski abonelik bırakılmazsa çıkarılan USB adaptör tutulur.
fn rebind(field: &mut Option<Bound>, next: Option<Radio>, kind: RadioKind, weak: Weak<Inner>) {
    if let Some(old) = field.take() {
        let _ = old.radio.RemoveStateChanged(old.token);
    }
    let Some(radio) = next else { return };
    // WinRT olayı iş parçacığı havuzundan gelir; adaptör çıkarılmışsa State okuması hata verebilir.
    let handler = TypedEventHandler::<Radio, windows::core::IInspectable>::new(move |sender, _| {
        if let Some(inner) = weak.upgrade() {
            let state = Inner::get_state(true, sender.as_ref());
            inner.raise(kind, state);
        }
        Ok(())
    });
    match radio.StateChanged(&handler) {
        Ok(token) => *field = Some(Bound { radio, token }),
        Err(_) => *field = Some(Bound { radio, token: 0 }),
    }
}

impl RadioService {
    fn refresh_core(&self) {
        let mut wifi: Option<Radio> = None;
        let mut bluetooth: Option<Radio> = None;
        let mut scan = || -> windows::core::Result<RadioAccess> {
            let access = map_access(Radio::RequestAccessAsync()?.join()?);
            if access == RadioAccess::Allowed {
                for radio in Radio::GetRadiosAsync()?.join()? {
                    match radio.Kind()? {
                        WinRadioKind::WiFi if wifi.is_none() => wifi = Some(radio),
                        WinRadioKind::Bluetooth if bluetooth.is_none() => bluetooth = Some(radio),
                        _ => {}
                    }
                }
            }
            Ok(access)
        };
        let access = match scan() {
            Ok(a) => a,
            Err(e) => {
                // Radyo yığını hata verirse radyo "yok" sayılır: döşeme sessizce "Kapalı" kalmak yerine nedenini yazar ve
                // Windows ayarlarını açar. Önceki taramadan kalan erişim durumu bayat kalmasın diye bilinmiyor yapılır.
                log("RadioService.RefreshCoreAsync", &e);
                wifi = None;
                bluetooth = None;
                RadioAccess::Unknown
            }
        };

        {
            let mut st = self.inner.lock();
            st.access = access;
            if st.disposed {
                return;
            }
            let weak = Arc::downgrade(&self.inner);
            rebind(&mut st.wifi, wifi, RadioKind::WiFi, weak.clone());
            rebind(&mut st.bluetooth, bluetooth, RadioKind::Bluetooth, weak);
            st.scanned = true;
        }
        self.inner.raise(RadioKind::WiFi, self.inner.current(RadioKind::WiFi));
        self.inner.raise(RadioKind::Bluetooth, self.inner.current(RadioKind::Bluetooth));
    }

    pub fn dispose(&self) {
        let mut st = self.inner.lock();
        if st.disposed {
            return;
        }
        st.disposed = true;
        let weak = Arc::downgrade(&self.inner);
        rebind(&mut st.wifi, None, RadioKind::WiFi, weak.clone());
        rebind(&mut st.bluetooth, None, RadioKind::Bluetooth, weak);
    }
}

impl Drop for RadioService {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl IRadioService for RadioService {
    fn access(&self) -> RadioAccess {
        self.inner.lock().access
    }

    fn wifi_state(&self) -> RadioState {
        self.inner.current(RadioKind::WiFi)
    }

    fn bluetooth_state(&self) -> RadioState {
        self.inner.current(RadioKind::Bluetooth)
    }

    fn state_changed(&self) -> &Event<RadioStateChangedEventArgs> {
        &self.inner.state_changed
    }

    fn refresh(&self) {
        {
            let mut st = self.inner.lock();
            if st.refreshing {
                // Aynı anda gelen çağrılar tek taramayı paylaşır.
                while st.refreshing {
                    st = self.inner.refresh_done.wait(st).unwrap_or_else(|e| e.into_inner());
                }
                return;
            }
            st.refreshing = true;
        }
        self.refresh_core();
        self.inner.lock().refreshing = false;
        self.inner.refresh_done.notify_all();
    }

    fn set_state(&self, kind: RadioKind, enable: bool) -> bool {
        // Panel açılır açılmaz tıklanırsa tarama henüz bitmemiş olabilir; tıklama yutulmasın.
        if !self.inner.lock().scanned {
            self.refresh();
        }
        let radio = {
            let st = self.inner.lock();
            let bound = if kind == RadioKind::WiFi { st.wifi.as_ref() } else { st.bluetooth.as_ref() };
            bound.map(|b| b.radio.clone())
        };
        let Some(radio) = radio else { return false };

        let result = radio.SetStateAsync(if enable { WinRadioState::On } else { WinRadioState::Off }).and_then(|op| op.join());
        match result {
            Ok(status) => {
                let ok = status == RadioAccessStatus::Allowed;
                if !ok && matches!(status, RadioAccessStatus::DeniedByUser | RadioAccessStatus::DeniedBySystem) {
                    self.inner.lock().access = map_access(status);
                }
                self.inner.raise(kind, radio.State().map(map_state).unwrap_or(RadioState::NotPresent));
                ok
            }
            Err(e) => {
                log("RadioService.SetStateAsync", &e);
                false
            }
        }
    }

    fn toggle(&self, kind: RadioKind) -> bool {
        if !self.inner.lock().scanned {
            self.refresh();
        }
        let current = self.inner.current(kind);
        self.set_state(kind, current != RadioState::On)
    }

    fn open_privacy_settings(&self) {
        open_url("ms-settings:privacy-radios");
    }

    fn open_settings(&self, kind: RadioKind) {
        // Ayarlar uygulaması kaldırılmış ya da politika ile engellenmiş olabilir: hata yutulur.
        open_url(if kind == RadioKind::WiFi { "ms-settings:network-wifi" } else { "ms-settings:bluetooth" });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_before_and_after_scan() {
        let service = RadioService::default();
        assert_eq!(service.wifi_state(), RadioState::Unknown);
        service.refresh();
        // Tarandıktan sonra Unknown kalmaz (radyo yoksa NotPresent). Sistem durumu değiştirilmez.
        assert_ne!(service.bluetooth_state(), RadioState::Unknown);
    }
}
