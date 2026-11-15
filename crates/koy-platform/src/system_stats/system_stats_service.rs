//! CPU, RAM, ağ hızı ve Koy belleği örnekleyicisi. Zamanlayıcı yalnız `start` ile kurulur, `stop` onu tamamen kaldırır;
//! durmuşken sıfır iş. Her saniye yalnız ucuz çağrılar: GetSystemTimes, GlobalMemoryStatusEx, GetIfTable2,
//! K32GetProcessMemoryInfo (mikrosaniyeler).
//!
//! "En çok CPU kullanan süreç" PAHALI: her taramada bütün süreçler için tutamaç açılır (tek tarama on ms mertebesi).
//! Bu yüzden 3 saniyede bir yapılır (`PROCESS_SCAN_EVERY_TICKS`); iki tarama arası değer sabit kalır.
//! `updated` zamanlayıcı iş parçacığından gelir (arayüze kendi dağıtıcısıyla geçirilmeli).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use koy_core::diagnostics::{CrashError, CrashLog};
use koy_core::event::Event;
use koy_core::system_stats::{ISystemStatsService, ProcessCpuSample, SystemSnapshot, SystemStatsMath, TopProcessPicker};
use koy_core::win_path;
use windows::Win32::Foundation::{CloseHandle, FILETIME};
use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::{GetProcessTimes, GetSystemTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

use crate::diagnostics::ProcessMemory;

const INTERVAL: Duration = Duration::from_millis(1000);
const FIRST_TICK: Duration = Duration::from_millis(400);
const PROCESS_SCAN_EVERY_TICKS: i32 = 3;
const IF_TYPE_LOOPBACK: u32 = 24;
const IF_TYPE_TUNNEL: u32 = 131;

pub type NetworkNameFn = Box<dyn Fn() -> Option<String> + Send + Sync>;

struct Sampler {
    prev_idle: u64,
    prev_kernel: u64,
    prev_user: u64,
    prev_in: u64,
    prev_out: u64,
    prev_ticks: Instant,
    tick: i32,
    prev_process_cpu: HashMap<i32, f64>,
    prev_scan_ticks: Instant,
    top_name: Option<String>,
    top_percent: f64,
}

struct State {
    running: bool,
    generation: u64,
    disposed: bool,
    current: SystemSnapshot,
}

struct Inner {
    network_name: NetworkNameFn,
    state: Mutex<State>,
    wake: Condvar,
    ticking: AtomicBool,
    updated: Event<SystemSnapshot>,
}

pub struct SystemStatsService {
    inner: Arc<Inner>,
}

fn to_u64(t: FILETIME) -> u64 {
    ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64
}

fn read_cpu_times() -> (u64, u64, u64) {
    let (mut i, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
    // SAFETY: çıktılar yerel değişkenlere yazılır.
    if unsafe { GetSystemTimes(Some(&mut i), Some(&mut k), Some(&mut u)) }.is_ok() { (to_u64(i), to_u64(k), to_u64(u)) } else { (0, 0, 0) }
}

fn read_memory() -> (i64, i64, f64) {
    let mut mem = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    // SAFETY: yapı boyutu ayarlandı.
    if unsafe { GlobalMemoryStatusEx(&mut mem) }.is_err() {
        return (0, 0, 0.0);
    }
    let total = mem.ullTotalPhys as i64;
    let used = total - mem.ullAvailPhys as i64;
    (used, total, SystemStatsMath::percent(used as f64, total as f64))
}

/// Fiziksel ve çalışan arayüzlerin birikimli bayt toplamı. Sanal/tünel arayüzler (Hyper-V vEthernet, VPN) sayılmaz:
/// aynı trafik fiziksel arayüzden de geçer, çift sayım olur. Loopback hariç.
fn read_network_counters() -> (u64, u64) {
    let (mut inb, mut outb) = (0u64, 0u64);
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    // SAFETY: tablo GetIfTable2 ile ayrılır, FreeMibTable ile bırakılır; satırlar NumEntries kadar okunur.
    unsafe {
        if GetIfTable2(&mut table).is_err() || table.is_null() {
            return (0, 0);
        }
        let count = (*table).NumEntries as usize;
        let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), count);
        for row in rows {
            let hardware = row.InterfaceAndOperStatusFlags._bitfield & 0x1 != 0;
            if !hardware || row.Type == IF_TYPE_LOOPBACK || row.Type == IF_TYPE_TUNNEL {
                continue;
            }
            if row.OperStatus != IfOperStatusUp {
                continue;
            }
            inb = inb.wrapping_add(row.InOctets);
            outb = outb.wrapping_add(row.OutOctets);
        }
        FreeMibTable(table as *const _);
    }
    (inb, outb)
}

/// Süreç başına toplam CPU saniyesi (C# `Process.TotalProcessorTime`); ad uzantısız (C# `ProcessName`).
fn scan_processes() -> (HashMap<i32, f64>, Vec<ProcessCpuSample>) {
    let mut map = HashMap::new();
    let mut samples = Vec::new();
    // SAFETY: anlık görüntü ve süreç tutamaçları her yolda kapatılır.
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return (map, samples) };
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snapshot, &mut entry).is_ok();
        while ok {
            let pid = entry.th32ProcessID;
            // Korumalı/kapanmış süreç: CPU süresi okunamaz, atla.
            if let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                let (mut c, mut e, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
                if GetProcessTimes(process, &mut c, &mut e, &mut k, &mut u).is_ok() {
                    let seconds = (to_u64(k) + to_u64(u)) as f64 / 10_000_000.0;
                    let len = entry.szExeFile.iter().position(|&ch| ch == 0).unwrap_or(entry.szExeFile.len());
                    let exe = String::from_utf16_lossy(&entry.szExeFile[..len]);
                    let name = win_path::get_file_name_without_extension(&exe).to_string();
                    map.insert(pid as i32, seconds);
                    samples.push(ProcessCpuSample::new(pid as i32, name, seconds));
                }
                let _ = CloseHandle(process);
            }
            ok = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
    }
    (map, samples)
}

impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn safe_network_name(&self) -> Option<String> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (self.network_name)())) {
            Ok(n) => n,
            Err(_) => {
                CrashLog::write("SystemStatsService.NetworkName", &CrashError::exception("System.Exception", "panic", ""));
                None
            }
        }
    }

    fn instant_snapshot(&self) -> SystemSnapshot {
        let (used, total, ram) = read_memory();
        SystemSnapshot::new(0.0, None, 0.0, used, total, ram, 0, 0, self.safe_network_name().as_deref(), ProcessMemory::get_private_working_set_bytes())
            .with_rates_ready(false)
    }

    fn sample(&self, s: &mut Sampler) -> SystemSnapshot {
        let now = Instant::now();
        let elapsed = (now - s.prev_ticks).as_secs_f64();
        s.prev_ticks = now;

        let (idle, kernel, user) = read_cpu_times();
        let cpu = SystemStatsMath::cpu_percent(s.prev_idle, s.prev_kernel, s.prev_user, idle, kernel, user);
        (s.prev_idle, s.prev_kernel, s.prev_user) = (idle, kernel, user);

        let (inb, outb) = read_network_counters();
        let down = SystemStatsMath::bytes_per_second(s.prev_in, inb, elapsed);
        let up = SystemStatsMath::bytes_per_second(s.prev_out, outb, elapsed);
        (s.prev_in, s.prev_out) = (inb, outb);

        s.tick += 1;
        if s.tick % PROCESS_SCAN_EVERY_TICKS == 0 {
            let (current, samples) = scan_processes();
            let scan_elapsed = (now - s.prev_scan_ticks).as_secs_f64();
            let cpus = std::thread::available_parallelism().map_or(1, |n| n.get()) as i32;
            let top = TopProcessPicker::pick(&s.prev_process_cpu, &samples, scan_elapsed, cpus);
            s.top_percent = top.as_ref().map_or(0.0, |t| t.1);
            s.top_name = top.map(|t| t.0);
            s.prev_process_cpu = current;
            s.prev_scan_ticks = now;
        }

        let (used, total, ram) = read_memory();
        SystemSnapshot::new(
            cpu,
            s.top_name.as_deref(),
            s.top_percent,
            used,
            total,
            ram,
            down,
            up,
            self.safe_network_name().as_deref(),
            ProcessMemory::get_private_working_set_bytes(),
        )
    }

    /// Zamanlayıcı iş parçacığı: `generation` değişince (stop) biter.
    fn timer_loop(self: Arc<Self>, generation: u64, mut sampler: Sampler) {
        let mut wait = FIRST_TICK;
        loop {
            {
                let st = self.lock();
                let (st, _) = self.wake.wait_timeout_while(st, wait, |s| s.running && s.generation == generation).unwrap_or_else(|e| e.into_inner());
                if !st.running || st.generation != generation {
                    return;
                }
            }
            wait = INTERVAL;
            // Önceki tur bitmediyse atla (yavaş makinede üst üste binmesin).
            if self.ticking.swap(true, Ordering::SeqCst) {
                continue;
            }
            // Kilit dışında: süreç taraması onlarca ms sürebilir, stop arayüzü bekletmesin.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.sample(&mut sampler)));
            match result {
                Ok(snapshot) => {
                    let publish = {
                        let mut st = self.lock();
                        if st.running && st.generation == generation {
                            st.current = snapshot.clone();
                            true
                        } else {
                            false
                        }
                    };
                    if publish {
                        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.updated.invoke(&snapshot)));
                    }
                }
                Err(_) => CrashLog::write("SystemStatsService.OnTick", &CrashError::exception("System.Exception", "panic", "")),
            }
            self.ticking.store(false, Ordering::SeqCst);
        }
    }
}

impl SystemStatsService {
    /// `network_name`: bağlı ağ adını verir (INetworkService.current().name); `None` verilirse ad boş kalır.
    pub fn new(network_name: Option<NetworkNameFn>) -> Self {
        SystemStatsService {
            inner: Arc::new(Inner {
                network_name: network_name.unwrap_or_else(|| Box::new(|| None)),
                state: Mutex::new(State { running: false, generation: 0, disposed: false, current: SystemSnapshot::empty() }),
                wake: Condvar::new(),
                ticking: AtomicBool::new(false),
                updated: Event::new(),
            }),
        }
    }

    pub fn dispose(&self) {
        self.inner.lock().disposed = true;
        self.stop();
    }
}

impl Default for SystemStatsService {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Drop for SystemStatsService {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl ISystemStatsService for SystemStatsService {
    fn is_running(&self) -> bool {
        self.inner.lock().running
    }

    fn current(&self) -> SystemSnapshot {
        self.inner.lock().current.clone()
    }

    fn updated(&self) -> &Event<SystemSnapshot> {
        &self.inner.updated
    }

    fn start(&self) {
        let mut st = self.inner.lock();
        if st.disposed || st.running {
            return;
        }
        // Taban ölçüm: ilk örnekte delta anlamlı olsun.
        let (idle, kernel, user) = read_cpu_times();
        let (inb, outb) = read_network_counters();
        let now = Instant::now();
        let sampler = Sampler {
            prev_idle: idle,
            prev_kernel: kernel,
            prev_user: user,
            prev_in: inb,
            prev_out: outb,
            prev_ticks: now,
            tick: 0,
            prev_process_cpu: scan_processes().0,
            prev_scan_ticks: now,
            top_name: None,
            top_percent: 0.0,
        };
        // Anlık değerler (RAM, Koy belleği, ağ adı) hemen okunur; CPU ve hızlar iki örnek ister ve rates_ready=false kalır.
        st.current = self.inner.instant_snapshot();
        st.running = true;
        st.generation += 1;
        let generation = st.generation;
        drop(st);
        // İlk gerçek örnek bir saniye beklemesin: CPU/ağ farkı gerçek süre üzerinden hesaplanır, 400 ms yeter.
        let inner = Arc::clone(&self.inner);
        let _ = std::thread::Builder::new().name("Koy.SystemStats".into()).spawn(move || inner.timer_loop(generation, sampler));
    }

    fn stop(&self) {
        {
            let mut st = self.inner.lock();
            st.running = false;
            st.current = SystemSnapshot::empty();
        }
        self.inner.wake.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_after_start() {
        let service = SystemStatsService::new(Some(Box::new(|| Some("Test".into()))));
        service.start();
        let first = service.current();
        assert!(!first.rates_ready);
        assert!(first.ram_total_bytes > 0);
        std::thread::sleep(Duration::from_millis(700));
        let next = service.current();
        assert!(next.rates_ready);
        assert_eq!(next.network_name.as_deref(), Some("Test"));
        service.stop();
        assert!(!service.is_running());
    }

    #[test]
    fn scans_processes() {
        let (map, samples) = scan_processes();
        assert!(!map.is_empty());
        assert!(samples.iter().all(|s| !s.name.to_ascii_lowercase().ends_with(".exe")));
    }
}
