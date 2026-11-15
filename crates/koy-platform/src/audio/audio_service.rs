//! Varsayılan çıkış aygıtının ses düzeyi ve sessiz durumu (Core Audio `IAudioEndpointVolume`). Dış değişiklikler
//! (klavye ses tuşları, Windows karışımı) `IAudioEndpointVolumeCallback` ile gelir; yoklama yok.

use std::sync::{Arc, Mutex, Weak};

use koy_core::audio::{AudioStateChangedEventArgs, IAudioService};
use koy_core::event::Event;
use windows::Win32::Media::Audio::Endpoints::{IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl};
use windows::Win32::Media::Audio::{AUDIO_VOLUME_NOTIFICATION_DATA, IMMDeviceEnumerator, MMDeviceEnumerator, eMultimedia, eRender};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::core::{GUID, implement};

use crate::bluetooth::support::ComApartment;

struct Endpoint {
    volume: IAudioEndpointVolume,
    callback: IAudioEndpointVolumeCallback,
}

// SAFETY: uç nokta arayüzü serbest iş parçacıklıdır (MTA'da etkinleştirilir) ve yalnız kilit altında kullanılır.
unsafe impl Send for Endpoint {}

struct State {
    volume: f32,
    is_muted: bool,
}

struct Inner {
    endpoint: Mutex<Option<Endpoint>>,
    state: Mutex<State>,
    state_changed: Event<AudioStateChangedEventArgs>,
}

impl Inner {
    fn raise(&self) {
        let args = {
            let st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            AudioStateChangedEventArgs::new(st.volume, st.is_muted)
        };
        self.state_changed.invoke(&args);
    }

    fn refresh_state(&self) {
        let read = {
            let ep = self.endpoint.lock().unwrap_or_else(|e| e.into_inner());
            let Some(ep) = ep.as_ref() else { return };
            // SAFETY: arayüz geçerli.
            unsafe { ep.volume.GetMasterVolumeLevelScalar().and_then(|l| ep.volume.GetMute().map(|m| (l, m.as_bool()))) }
        };
        if let Ok((level, muted)) = read {
            {
                let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
                st.volume = level;
                st.is_muted = muted;
            }
            self.raise();
        }
    }
}

#[implement(IAudioEndpointVolumeCallback)]
struct VolumeCallback {
    inner: Weak<Inner>,
}

impl IAudioEndpointVolumeCallback_Impl for VolumeCallback_Impl {
    fn OnNotify(&self, pnotify: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        if !pnotify.is_null()
            && let Some(inner) = self.inner.upgrade()
        {
            inner.refresh_state();
        }
        Ok(())
    }
}

pub struct AudioService {
    inner: Arc<Inner>,
}

impl Default for AudioService {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioService {
    pub fn new() -> Self {
        let inner = Arc::new(Inner {
            endpoint: Mutex::new(None),
            state: Mutex::new(State { volume: 0.0, is_muted: false }),
            state_changed: Event::new(),
        });
        let _com = ComApartment::enter_mta();
        // Ses aygıtı yoksa veya COM hatasında servis sessizce işlevsiz kalır.
        // SAFETY: COM çağrıları; arayüzler Drop ile bırakılır.
        let endpoint = unsafe {
            (|| -> windows::core::Result<Endpoint> {
                let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
                let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
                let volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
                let callback: IAudioEndpointVolumeCallback = VolumeCallback { inner: Arc::downgrade(&inner) }.into();
                volume.RegisterControlChangeNotify(&callback)?;
                Ok(Endpoint { volume, callback })
            })()
        };
        if let Ok(ep) = endpoint {
            *inner.endpoint.lock().unwrap_or_else(|e| e.into_inner()) = Some(ep);
            inner.refresh_state();
        }
        AudioService { inner }
    }

    pub fn refresh_state(&self) {
        self.inner.refresh_state();
    }

    pub fn dispose(&self) {
        if let Some(ep) = self.inner.endpoint.lock().unwrap_or_else(|e| e.into_inner()).take() {
            // SAFETY: kayıtlı geri çağrı çözülür.
            let _ = unsafe { ep.volume.UnregisterControlChangeNotify(&ep.callback) };
        }
    }
}

impl Drop for AudioService {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl IAudioService for AudioService {
    fn volume(&self) -> f32 {
        self.inner.state.lock().unwrap_or_else(|e| e.into_inner()).volume
    }

    fn is_muted(&self) -> bool {
        self.inner.state.lock().unwrap_or_else(|e| e.into_inner()).is_muted
    }

    fn state_changed(&self) -> &Event<AudioStateChangedEventArgs> {
        &self.inner.state_changed
    }

    fn set_volume(&self, volume: f32) {
        let clamped = volume.clamp(0.0, 1.0);
        let ok = {
            let ep = self.inner.endpoint.lock().unwrap_or_else(|e| e.into_inner());
            let Some(ep) = ep.as_ref() else { return };
            // SAFETY: arayüz geçerli; olay bağlamı boş GUID (C# Guid.Empty).
            unsafe { ep.volume.SetMasterVolumeLevelScalar(clamped, &GUID::zeroed()) }.is_ok()
        };
        if ok {
            self.inner.state.lock().unwrap_or_else(|e| e.into_inner()).volume = clamped;
            self.inner.raise();
        }
    }

    fn set_mute(&self, is_muted: bool) {
        let ok = {
            let ep = self.inner.endpoint.lock().unwrap_or_else(|e| e.into_inner());
            let Some(ep) = ep.as_ref() else { return };
            // SAFETY: arayüz geçerli.
            unsafe { ep.volume.SetMute(is_muted, &GUID::zeroed()) }.is_ok()
        };
        if ok {
            self.inner.state.lock().unwrap_or_else(|e| e.into_inner()).is_muted = is_muted;
            self.inner.raise();
        }
    }

    fn toggle_mute(&self) {
        self.set_mute(!self.is_muted());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_volume_without_changing_it() {
        let service = AudioService::new();
        let v = service.volume();
        assert!((0.0..=1.0).contains(&v));
        service.dispose();
    }
}
