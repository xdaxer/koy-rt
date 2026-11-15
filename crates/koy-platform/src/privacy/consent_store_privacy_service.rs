//! Windows ConsentStore kayıtlarını RegNotifyChangeKeyValue ile olay tabanlı izleyen servis.
//! Periyodik yoklama yapmaz, boştayken %0 CPU harcar.

use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use koy_core::event::Event;
use koy_core::privacy::{CaptureUsagePolicy, IPrivacyMonitorService, PrivacyState};
use koy_core::text;
use windows::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, REG_NOTIFY_CHANGE_LAST_SET, REG_NOTIFY_CHANGE_NAME, REG_NOTIFY_FILTER, REG_QWORD, REG_VALUE_TYPE,
    RegCloseKey, RegEnumKeyExW, RegNotifyChangeKeyValue, RegOpenKeyExW, RegQueryValueExW,
};
use windows::Win32::System::Threading::{CreateEventW, SetEvent, WaitForMultipleObjects, WaitForSingleObject, INFINITE};
use windows::core::{HSTRING, PWSTR};

const WEBCAM_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\webcam";
const MIC_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";
const CONSENT_STORE_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";

/// Okuma için açılmış, bırakılınca kapanan kayıt anahtarı.
struct Key(HKEY);

// SAFETY: kayıt tutamaçları iş parçacıkları arasında taşınabilir.
unsafe impl Send for Key {}

impl Key {
    fn open(parent: HKEY, path: &str) -> Option<Key> {
        let mut key = HKEY::default();
        // SAFETY: çıktı tutamacı yerel değişkene yazılır.
        (unsafe { RegOpenKeyExW(parent, &HSTRING::from(path), None, KEY_READ, &mut key) } == ERROR_SUCCESS).then_some(Key(key))
    }

    fn sub(&self, name: &str) -> Option<Key> {
        Key::open(self.0, name)
    }

    fn sub_key_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        let mut index = 0u32;
        loop {
            let mut buf = [0u16; 256];
            let mut len = buf.len() as u32;
            // SAFETY: arabellek `len` karakter; anahtar adı en çok 255 karakter.
            let status = unsafe { RegEnumKeyExW(self.0, index, Some(PWSTR(buf.as_mut_ptr())), &mut len, None, None, None, None) };
            if status != ERROR_SUCCESS {
                break;
            }
            names.push(String::from_utf16_lossy(&buf[..len as usize]));
            index += 1;
        }
        names
    }

    /// REG_QWORD değeri (C# `GetValue(...) is long`).
    fn get_qword(&self, name: &str) -> Option<i64> {
        let mut kind = REG_VALUE_TYPE(0);
        let mut value = 0i64;
        let mut size = 8u32;
        // SAFETY: değer 8 baytlık yerel değişkene yazılır.
        let status = unsafe {
            RegQueryValueExW(self.0, &HSTRING::from(name), None, Some(&mut kind), Some(&mut value as *mut i64 as *mut u8), Some(&mut size))
        };
        (status == ERROR_SUCCESS && kind == REG_QWORD && size == 8).then_some(value)
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: tutamaç bir kez kapatılır.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

struct EventHandle(HANDLE);

// SAFETY: olay tutamaçları iş parçacıkları arasında paylaşılabilir.
unsafe impl Send for EventHandle {}
unsafe impl Sync for EventHandle {}

impl EventHandle {
    fn new(manual_reset: bool) -> Option<EventHandle> {
        // SAFETY: adsız olay.
        unsafe { CreateEventW(None, manual_reset, false, None) }.ok().map(EventHandle)
    }
}

impl Drop for EventHandle {
    fn drop(&mut self) {
        // SAFETY: tutamaç bir kez kapatılır.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// Bir Start–Stop döngüsünün tanıtıcıları. İş parçacığı kendi Run'ını sahiplenir ve çıkarken bırakır.
struct Run {
    cancel: Arc<EventHandle>,
    cam_event: EventHandle,
    mic_event: EventHandle,
    parent_event: EventHandle,
    webcam: Option<Key>,
    mic: Option<Key>,
    parent: Option<Key>,
    cam_armed: bool,
    mic_armed: bool,
    parent_armed: bool,
}

struct Active {
    cancel: Arc<EventHandle>,
    thread: Option<JoinHandle<()>>,
}

struct State {
    run: Option<Active>,
    current_state: PrivacyState,
    disposed: bool,
}

struct Inner {
    state: Mutex<State>,
    state_changed: Event<PrivacyState>,
}

pub struct ConsentStorePrivacyService {
    inner: Arc<Inner>,
}

impl Default for ConsentStorePrivacyService {
    fn default() -> Self {
        ConsentStorePrivacyService {
            inner: Arc::new(Inner {
                state: Mutex::new(State { run: None, current_state: PrivacyState::EMPTY, disposed: false }),
                state_changed: Event::new(),
            }),
        }
    }
}

fn try_open(path: &str) -> Option<Key> {
    Key::open(HKEY_CURRENT_USER, path)
}

/// Bildirimi kurar; kurulamazsa anahtarı kapatıp `None` yapar (bir sonraki turda yeniden açılır ya da üst anahtar izlenir).
fn arm(key: &mut Option<Key>, event: &EventHandle, watch_subtree: bool, filter: REG_NOTIFY_FILTER) -> bool {
    let Some(k) = key.as_ref() else { return false };
    // SAFETY: anahtar ve olay geçerli; eşzamansız bildirim olayı işaretler.
    if unsafe { RegNotifyChangeKeyValue(k.0, watch_subtree, filter, Some(event.0), true) } == ERROR_SUCCESS {
        return true;
    }
    *key = None;
    false
}

fn check_single_key(key: &Key, key_name: &str) -> Option<Option<String>> {
    let start = key.get_qword("LastUsedTimeStart")?;
    let stop = key.get_qword("LastUsedTimeStop").unwrap_or(0);
    CaptureUsagePolicy::is_actively_using(start, stop).then(|| CaptureUsagePolicy::extract_app_name_from_key(Some(key_name)))
}

fn check_device_usage(root: Option<&Key>) -> (bool, Option<String>) {
    let Some(root) = root else { return (false, None) };
    // 1. Doğrudan paketli (UWP) alt anahtarlar.
    for name in root.sub_key_names() {
        if text::eq_ignore_case(&name, "NonPackaged") {
            continue;
        }
        if let Some(sub) = root.sub(&name)
            && let Some(app) = check_single_key(&sub, &name)
        {
            return (true, app);
        }
    }
    // 2. Paketsiz (Win32 exe) alt anahtarlar.
    if let Some(non_packaged) = root.sub("NonPackaged") {
        for name in non_packaged.sub_key_names() {
            if let Some(sub) = non_packaged.sub(&name)
                && let Some(app) = check_single_key(&sub, &name)
            {
                return (true, app);
            }
        }
    }
    (false, None)
}

fn query_current_state(run: &Run) -> PrivacyState {
    let (cam_in_use, cam_app) = check_device_usage(run.webcam.as_ref());
    let (mic_in_use, mic_app) = check_device_usage(run.mic.as_ref());
    PrivacyState::new(cam_in_use, mic_in_use, cam_app, mic_app)
}

impl Inner {
    fn publish(&self, new_state: PrivacyState) {
        {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if new_state == st.current_state {
                return;
            }
            st.current_state = new_state.clone();
        }
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.state_changed.invoke(&new_state)));
    }

    fn worker_loop(&self, mut run: Run) {
        loop {
            // Eksik anahtar (ör. kamerasız makinede webcam) yoklanmaz: üst ConsentStore anahtarında alt anahtar
            // oluşturma bildirimi beklenir, oluşunca açılır.
            if run.webcam.is_none() {
                run.webcam = try_open(WEBCAM_KEY_PATH);
            }
            if run.mic.is_none() {
                run.mic = try_open(MIC_KEY_PATH);
            }
            // RegNotifyChangeKeyValue tek seferliktir: yalnız sinyali tüketilmiş olanı yeniden kur.
            if !run.cam_armed && run.webcam.is_some() {
                run.cam_armed = arm(&mut run.webcam, &run.cam_event, true, REG_NOTIFY_CHANGE_LAST_SET);
            }
            if !run.mic_armed && run.mic.is_some() {
                run.mic_armed = arm(&mut run.mic, &run.mic_event, true, REG_NOTIFY_CHANGE_LAST_SET);
            }
            if run.webcam.is_none() || run.mic.is_none() {
                if run.parent.is_none() {
                    run.parent = try_open(CONSENT_STORE_PATH);
                }
                if !run.parent_armed && run.parent.is_some() {
                    run.parent_armed = arm(&mut run.parent, &run.parent_event, false, REG_NOTIFY_CHANGE_NAME);
                }
            }

            // Bildirimler kurulduktan SONRA oku: okuma ile kurulum arasındaki yazımlar kaçmaz.
            self.publish(query_current_state(&run));

            let handles = [run.cancel.0, run.cam_event.0, run.mic_event.0, run.parent_event.0];
            // SAFETY: tutamaçlar Run yaşadıkça geçerli.
            let index = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) }.0.wrapping_sub(WAIT_OBJECT_0.0);
            match index {
                0 => break,
                1 => run.cam_armed = false,
                2 => run.mic_armed = false,
                3 => run.parent_armed = false,
                _ => break,
            }
            // Windows başlangıç ve bitiş damgalarını birkaç milisaniye arayla yazabilir.
            // SAFETY: iptal olayı geçerli.
            if unsafe { WaitForSingleObject(run.cancel.0, 50) } == WAIT_OBJECT_0 {
                break;
            }
        }
        // Run burada düşer: anahtarlar ve olaylar iş parçacığı içinde bırakılır.
    }
}

impl ConsentStorePrivacyService {
    pub fn dispose(&self) {
        {
            let mut st = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
            if st.disposed {
                return;
            }
            st.disposed = true;
        }
        // Önce disposed, sonra stop: stop artık disposed'a bakmaz, izleyici gerçekten durur.
        self.stop();
    }
}

impl Drop for ConsentStorePrivacyService {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl IPrivacyMonitorService for ConsentStorePrivacyService {
    fn current_state(&self) -> PrivacyState {
        self.inner.state.lock().unwrap_or_else(|e| e.into_inner()).current_state.clone()
    }

    fn state_changed(&self) -> &Event<PrivacyState> {
        &self.inner.state_changed
    }

    fn start(&self) {
        let mut st = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        if st.disposed || st.run.is_some() {
            return;
        }
        let (Some(cancel), Some(cam_event), Some(mic_event), Some(parent_event)) =
            (EventHandle::new(true), EventHandle::new(false), EventHandle::new(false), EventHandle::new(false))
        else {
            return;
        };
        let cancel = Arc::new(cancel);
        let run = Run {
            cancel: Arc::clone(&cancel),
            cam_event,
            mic_event,
            parent_event,
            webcam: try_open(WEBCAM_KEY_PATH),
            mic: try_open(MIC_KEY_PATH),
            parent: None,
            cam_armed: false,
            mic_armed: false,
            parent_armed: false,
        };
        // İlk durumu hemen oku (çentik açılışta doğru noktayla başlasın).
        st.current_state = query_current_state(&run);
        let inner = Arc::clone(&self.inner);
        let thread = std::thread::Builder::new().name("Koy.PrivacyMonitor".into()).spawn(move || inner.worker_loop(run)).ok();
        st.run = Some(Active { cancel, thread });
    }

    fn stop(&self) {
        let Some(active) = self.inner.state.lock().unwrap_or_else(|e| e.into_inner()).run.take() else { return };
        // SAFETY: iptal olayı Arc ile yaşıyor.
        let _ = unsafe { SetEvent(active.cancel.0) };
        // Tanıtıcıları iş parçacığı kendisi bırakır; burada yalnız (en çok 2 sn) bekleriz.
        if let Some(t) = active.thread {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            while !t.is_finished() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            if t.is_finished() {
                let _ = t.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_reads_and_stops() {
        let service = ConsentStorePrivacyService::default();
        service.start();
        let _ = service.current_state();
        service.stop();
        service.dispose();
    }

    #[test]
    fn enumerates_consent_store() {
        let root = try_open(CONSENT_STORE_PATH);
        if let Some(root) = root {
            assert!(!root.sub_key_names().is_empty());
        }
    }
}
