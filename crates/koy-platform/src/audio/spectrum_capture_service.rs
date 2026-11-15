//! Varsayılan çıkış aygıtında çalan sesi (WASAPI loopback) okuyup müzik kartının çubuklarını hesaplar.
//! Yalnız `start` ile `stop` arasında çalışır (çentik açık, Ana görünür, müzik çalıyor); o sırada ayrı bir iş parçacığı
//! saniyede ~30 kez paketleri boşaltır. Ses inip çubuklar sıfırlanınca olay kesilir, arayüz boşuna çizilmez. Ses
//! kaydedilmez, diske ya da ağa hiçbir şey gitmez.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use koy_core::audio::SpectrumAnalyzer;
use koy_core::diagnostics::{CrashError, CrashLog};
use koy_core::event::Event;
use windows::Win32::Media::Audio::{
    AUDCLNT_SHAREMODE_SHARED, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator, WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
    eConsole, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree};
use windows::core::GUID;

use crate::bluetooth::support::ComApartment;

pub const BAND_COUNT: i32 = 24;

const FRAME_INTERVAL: Duration = Duration::from_millis(33);
const BUFFER_DURATION_100NS: i64 = 2_000_000; // 200 ms paylaşımlı arabellek
const LOOPBACK_FLAG: u32 = 0x0002_0000; // AUDCLNT_STREAMFLAGS_LOOPBACK
const SILENT_FLAG: u32 = 0x2; // AUDCLNT_BUFFERFLAGS_SILENT
const FORMAT_PCM: u16 = 1;
const FORMAT_FLOAT: u16 = 3;
const FORMAT_EXTENSIBLE: u16 = 0xFFFE;
const SUBTYPE_FLOAT: GUID = GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);

#[derive(Clone, Copy)]
struct SampleFormat {
    sample_rate: i32,
    channels: i32,
    bits_per_sample: i32,
    is_float: bool,
}

impl SampleFormat {
    fn block_align(&self) -> usize {
        (self.channels * self.bits_per_sample / 8) as usize
    }
}

struct Inner {
    stop_requested: AtomicBool,
    levels_updated: Event<Vec<f32>>,
}

pub struct SpectrumCaptureService {
    inner: Arc<Inner>,
    thread: Mutex<Option<JoinHandle<()>>>,
    disposed: AtomicBool,
}

impl Default for SpectrumCaptureService {
    fn default() -> Self {
        SpectrumCaptureService {
            inner: Arc::new(Inner { stop_requested: AtomicBool::new(false), levels_updated: Event::new() }),
            thread: Mutex::new(None),
            disposed: AtomicBool::new(false),
        }
    }
}

impl SpectrumCaptureService {
    /// Yeni çubuk düzeyleri (0..1, `BAND_COUNT` tane). Yakalama iş parçacığından çağrılır.
    pub fn levels_updated(&self) -> &Event<Vec<f32>> {
        &self.inner.levels_updated
    }

    pub fn is_running(&self) -> bool {
        self.thread.lock().unwrap_or_else(|e| e.into_inner()).is_some()
    }

    pub fn start(&self) {
        assert!(!self.disposed.load(Ordering::SeqCst), "Cannot access a disposed object.");
        let mut slot = self.thread.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_some() {
            return;
        }
        self.inner.stop_requested.store(false, Ordering::SeqCst);
        let inner = Arc::clone(&self.inner);
        *slot = std::thread::Builder::new().name("Koy müzik çubukları".into()).spawn(move || capture_loop(&inner)).ok();
    }

    pub fn stop(&self) {
        let thread = self.thread.lock().unwrap_or_else(|e| e.into_inner()).take();
        self.inner.stop_requested.store(true, Ordering::SeqCst);
        // Döngü en geç bir kare (33 ms) içinde çıkar; arayüzü uzun bekletmemek için sınırlı beklenir.
        if let Some(t) = thread
            && t.thread().id() != std::thread::current().id()
        {
            let deadline = Instant::now() + Duration::from_millis(500);
            while !t.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            if t.is_finished() {
                let _ = t.join();
            }
        }
    }

    pub fn dispose(&self) {
        if self.disposed.swap(true, Ordering::SeqCst) {
            return;
        }
        self.stop();
        self.inner.levels_updated.clear();
    }
}

impl Drop for SpectrumCaptureService {
    fn drop(&mut self) {
        self.dispose();
    }
}

fn capture_loop(inner: &Inner) {
    let _com = ComApartment::enter_mta();
    let mut levels = vec![0f32; BAND_COUNT as usize];
    if let Err(e) = run(inner, &mut levels) {
        // Aygıt çıkarıldı, biçim desteklenmiyor, COM hatası: çubuklar durur, uygulama çalışmaya devam eder.
        CrashLog::write("Müzik çubukları: ses yakalama", &CrashError::exception("System.Exception", &e.message(), ""));
    }
    levels.iter_mut().for_each(|l| *l = 0.0);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| inner.levels_updated.invoke(&levels)));
}

fn run(inner: &Inner, levels: &mut Vec<f32>) -> windows::core::Result<()> {
    let Some((client, capture, format)) = try_open()? else { return Ok(()) };
    let mut analyzer = SpectrumAnalyzer::new(BAND_COUNT);
    let mut mono = vec![0f32; 4096];
    // SAFETY: istemci başlatıldı.
    unsafe { client.Start()? };
    let mut was_silent = true;
    let mut last_frame = Instant::now();

    while !inner.stop_requested.load(Ordering::SeqCst) {
        std::thread::sleep(FRAME_INTERVAL);
        let received = drain(&capture, format, &mut analyzer, &mut mono)?;

        // Hiç ses çalınmıyorsa ses motoru loopback paketi göndermez: geçen süre kadar sessizlik eklenir.
        let now = Instant::now();
        if received == 0 {
            let silence = (now - last_frame).as_secs_f64() * format.sample_rate as f64;
            analyzer.add_silence((SpectrumAnalyzer::FFT_SIZE as f64).min(silence) as i32);
        }
        last_frame = now;

        analyzer.update(format.sample_rate);
        let silent = analyzer.is_silent();
        if silent && was_silent {
            continue;
        }
        was_silent = silent;
        levels.copy_from_slice(analyzer.levels());
        inner.levels_updated.invoke(levels);
    }
    // SAFETY: istemci başlatılmıştı.
    unsafe { client.Stop()? };
    Ok(())
}

fn try_open() -> windows::core::Result<Option<(IAudioClient, IAudioCaptureClient, SampleFormat)>> {
    // SAFETY: COM çağrıları; karışım biçimi CoTaskMemFree ile serbest bırakılır.
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let mix = client.GetMixFormat()?;
        if mix.is_null() {
            return Ok(None);
        }
        let format = read_format(mix);
        let init = match format {
            Some(_) => client.Initialize(AUDCLNT_SHAREMODE_SHARED, LOOPBACK_FLAG, BUFFER_DURATION_100NS, 0, mix, None),
            None => Ok(()),
        };
        CoTaskMemFree(Some(mix as *const _));
        let Some(format) = format else { return Ok(None) };
        init?;
        let capture: IAudioCaptureClient = client.GetService()?;
        Ok(Some((client, capture, format)))
    }
}

unsafe fn read_format(mix: *const WAVEFORMATEX) -> Option<SampleFormat> {
    // SAFETY: mix GetMixFormat'tan gelen geçerli yapı; genişletilmiş alan cbSize ile denetlenir.
    unsafe {
        let m = std::ptr::read_unaligned(mix);
        let tag = m.wFormatTag;
        let mut is_float = tag == FORMAT_FLOAT;
        if tag == FORMAT_EXTENSIBLE && m.cbSize >= 22 {
            let ext = std::ptr::read_unaligned(mix as *const WAVEFORMATEXTENSIBLE);
            let sub = ext.SubFormat;
            is_float = sub == SUBTYPE_FLOAT;
        } else if tag != FORMAT_FLOAT && tag != FORMAT_PCM && tag != FORMAT_EXTENSIBLE {
            return None;
        }
        let bits = m.wBitsPerSample as i32;
        if m.nChannels == 0 || m.nSamplesPerSec == 0 {
            return None;
        }
        if if is_float { bits != 32 } else { !matches!(bits, 16 | 24 | 32) } {
            return None;
        }
        Some(SampleFormat { sample_rate: m.nSamplesPerSec as i32, channels: m.nChannels as i32, bits_per_sample: bits, is_float })
    }
}

// Bekleyen bütün paketleri tek kanala indirip analizöre verir; okunan çerçeve sayısını döner.
fn drain(capture: &IAudioCaptureClient, format: SampleFormat, analyzer: &mut SpectrumAnalyzer, mono: &mut Vec<f32>) -> windows::core::Result<i32> {
    let mut total = 0;
    // SAFETY: WASAPI arabelleği GetBuffer/ReleaseBuffer arasında geçerli; okunan bayt frames * block_align.
    unsafe {
        let mut packet = capture.GetNextPacketSize()?;
        while packet > 0 {
            let mut data: *mut u8 = std::ptr::null_mut();
            let (mut frames, mut flags) = (0u32, 0u32);
            capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
            if frames > 0 {
                if mono.len() < frames as usize {
                    mono.resize(frames as usize, 0.0);
                }
                let span = &mut mono[..frames as usize];
                if flags & SILENT_FLAG != 0 || data.is_null() {
                    span.fill(0.0);
                } else {
                    to_mono(std::slice::from_raw_parts(data, frames as usize * format.block_align()), format, span);
                }
                analyzer.add_samples(span);
                total += frames as i32;
            }
            capture.ReleaseBuffer(frames)?;
            packet = capture.GetNextPacketSize()?;
        }
    }
    Ok(total)
}

fn to_mono(data: &[u8], format: SampleFormat, mono: &mut [f32]) {
    let channels = format.channels as usize;
    let bytes = (format.bits_per_sample / 8) as usize;
    let stride = format.block_align();
    let inv = 1f32 / channels as f32;
    for (frame, out) in mono.iter_mut().enumerate() {
        let mut sum = 0f32;
        for c in 0..channels {
            let p = &data[frame * stride + c * bytes..];
            sum += if format.is_float {
                f32::from_le_bytes([p[0], p[1], p[2], p[3]])
            } else {
                match bytes {
                    2 => i16::from_le_bytes([p[0], p[1]]) as f32 / 32768f32,
                    3 => (((p[0] as i32) << 8) | ((p[1] as i32) << 16) | ((p[2] as i32) << 24)) as f32 / 2147483648f32,
                    _ => i32::from_le_bytes([p[0], p[1], p[2], p[3]]) as f32 / 2147483648f32,
                }
            };
        }
        *out = sum * inv;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_stereo_pcm16_to_mono() {
        let format = SampleFormat { sample_rate: 48000, channels: 2, bits_per_sample: 16, is_float: false };
        let mut data = Vec::new();
        for (l, r) in [(16384i16, 0i16), (-32768, -32768)] {
            data.extend_from_slice(&l.to_le_bytes());
            data.extend_from_slice(&r.to_le_bytes());
        }
        let mut mono = [0f32; 2];
        to_mono(&data, format, &mut mono);
        assert_eq!(mono, [0.25, -1.0]);
    }

    #[test]
    fn starts_and_stops_capture() {
        let service = SpectrumCaptureService::default();
        service.start();
        std::thread::sleep(Duration::from_millis(100));
        service.stop();
        assert!(!service.is_running());
    }
}
