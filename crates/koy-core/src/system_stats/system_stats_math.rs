use std::collections::HashMap;

use crate::text;

/// Örnekleme hesapları (saf, WinAPI yok).
pub struct SystemStatsMath;

impl SystemStatsMath {
    /// GetSystemTimes değerleri (100 ns birim, birikimli). Not: kernel süresi boşta süresini DE içerir,
    /// bu yüzden toplam = kernel + user, meşgul = toplam - boşta. Delta sıfır/negatifse 0.
    pub fn cpu_percent(prev_idle: u64, prev_kernel: u64, prev_user: u64, idle: u64, kernel: u64, user: u64) -> f64 {
        if idle < prev_idle || kernel < prev_kernel || user < prev_user {
            return 0.0;
        }

        // C# `ulong` toplamı denetimsizdir (taşarsa sarar).
        let total = (kernel - prev_kernel).wrapping_add(user - prev_user);
        if total == 0 {
            return 0.0;
        }

        let idle_delta = idle - prev_idle;
        let busy = if total > idle_delta { total - idle_delta } else { 0 };
        text::clamp(busy as f64 * 100.0 / total as f64, 0.0, 100.0)
    }

    /// Bayt sayacından hız (bayt/sn). Sayaç geri giderse (arayüz sıfırlandı, bağdaştırıcı çıktı) 0 döner,
    /// devasa sıçrama göstermez. Geçen süre sıfır/negatifse 0.
    pub fn bytes_per_second(previous: u64, current: u64, elapsed_seconds: f64) -> i64 {
        if elapsed_seconds <= 0.0 || current < previous {
            return 0;
        }
        ((current - previous) as f64 / elapsed_seconds) as i64
    }

    pub fn percent(used: f64, total: f64) -> f64 {
        if total <= 0.0 { 0.0 } else { text::clamp(used * 100.0 / total, 0.0, 100.0) }
    }

    /// Bir sürecin toplam işlemcideki payı: CPU süresi farkı / (geçen süre x çekirdek sayısı). Süre birimi aynı olmalı.
    pub fn process_cpu_percent(cpu_seconds_delta: f64, elapsed_seconds: f64, processor_count: i32) -> f64 {
        if cpu_seconds_delta <= 0.0 || elapsed_seconds <= 0.0 || processor_count <= 0 {
            return 0.0;
        }
        text::clamp(cpu_seconds_delta * 100.0 / (elapsed_seconds * processor_count as f64), 0.0, 100.0)
    }
}

/// Bir süreç taramasındaki tek süreç: kimlik, ad, birikimli CPU süresi (saniye).
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessCpuSample {
    pub pid: i32,
    pub name: String,
    pub cpu_seconds: f64,
}

impl ProcessCpuSample {
    pub fn new(pid: i32, name: impl Into<String>, cpu_seconds: f64) -> Self {
        ProcessCpuSample { pid, name: name.into(), cpu_seconds }
    }
}

/// "En çok CPU kullanan süreç" seçimi: iki tarama arasındaki CPU süresi farkına göre (saf).
pub struct TopProcessPicker;

impl TopProcessPicker {
    /// `previous` (pid -> CPU saniyesi) ile `current` arasında en çok artan süreç.
    /// Önceki taramada olmayan süreç (yeni açıldı) yok sayılır: tüm ömür boyu CPU'su tek seferde sıçrama yapar.
    /// pid 0 (Idle) hariç. Hiç aday yoksa `None`; aksi halde `(ad, yüzde)`.
    pub fn pick(
        previous: &HashMap<i32, f64>,
        current: &[ProcessCpuSample],
        elapsed_seconds: f64,
        processor_count: i32,
    ) -> Option<(String, f64)> {
        let mut best_name: Option<&str> = None;
        let mut best_percent = 0.0;
        for sample in current {
            let Some(&before) = previous.get(&sample.pid).filter(|_| sample.pid != 0) else { continue };
            let percent = SystemStatsMath::process_cpu_percent(sample.cpu_seconds - before, elapsed_seconds, processor_count);
            if percent > best_percent {
                best_percent = percent;
                best_name = Some(&sample.name);
            }
        }

        best_name.map(|name| (name.to_owned(), best_percent))
    }
}
