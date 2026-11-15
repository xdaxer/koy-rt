use super::system_stats::SystemSnapshot;

/// Sayfa geri dönünce gösterilecek değeri seçer. Servis yalnız sayfa görünürken tutulduğundan her dönüşte
/// yeni kurulur ve ilk örneğe kadar boştur; bu aralıkta "–" yerine son bilinen değer gösterilir.
pub struct SystemSnapshotMerge;

impl SystemSnapshotMerge {
    /// `fresh` tam örnekse ([`SystemSnapshot::rates_ready`]) aynen döner.
    /// Yalnız anlık değerleri varsa (RAM, Koy belleği, ağ adı) onlar taze alınır, CPU / süreç / ağ hızı
    /// `last_known`'dan gelir. İkisinde de oran yoksa `fresh` döner (CPU henüz bilinmiyor).
    pub fn merge(fresh: SystemSnapshot, last_known: Option<&SystemSnapshot>) -> SystemSnapshot {
        let last_known = match last_known {
            Some(last) if !fresh.rates_ready && last.rates_ready => last,
            _ => return fresh,
        };

        SystemSnapshot {
            cpu_percent: last_known.cpu_percent,
            top_process_name: last_known.top_process_name.clone(),
            top_process_cpu_percent: last_known.top_process_cpu_percent,
            net_down_bytes_per_sec: last_known.net_down_bytes_per_sec,
            net_up_bytes_per_sec: last_known.net_up_bytes_per_sec,
            rates_ready: true,
            ..fresh
        }
    }

    /// Anlık değerler (RAM) okunabildi mi: hiç değer yoksa ekran "–" gösterir.
    pub fn has_instant_data(s: &SystemSnapshot) -> bool {
        s.ram_total_bytes > 0
    }
}
