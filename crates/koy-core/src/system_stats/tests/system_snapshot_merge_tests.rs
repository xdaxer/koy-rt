use crate::system_stats::*;

const GB: i64 = 1 << 30;
const MB: i64 = 1 << 20;

fn full(cpu: f64) -> SystemSnapshot {
    SystemSnapshot::new(cpu, Some("Chrome"), 11.0, 9 * GB, 16 * GB, 58.0, 2_000_000, 300_000, Some("Ev"), 64 * MB)
}

// Servis yeni kurulmuş: yalnız anlık değerler.
fn instant(used_bytes: i64) -> SystemSnapshot {
    SystemSnapshot::new(0.0, None, 0.0, used_bytes, 16 * GB, 62.5, 0, 0, Some("Ev"), 66 * MB).with_rates_ready(false)
}

#[test]
fn full_sample_is_returned_as_is() {
    let fresh = full(40.0);

    // C# `Assert.Same`: Rust'ta değer taşınır; aynı değer döner.
    assert_eq!(SystemSnapshotMerge::merge(fresh.clone(), Some(&full(10.0))), fresh);
}

#[test]
fn instant_sample_takes_fresh_ram_but_keeps_last_cpu_and_rates() {
    let merged = SystemSnapshotMerge::merge(instant(10 * GB), Some(&full(23.0)));

    assert_eq!(merged.ram_used_bytes, 10 * GB);
    assert_eq!(merged.ram_percent, 62.5);
    assert_eq!(merged.koy_memory_bytes, 66 * MB);
    assert_eq!(merged.cpu_percent, 23.0);
    assert_eq!(merged.top_process_name.as_deref(), Some("Chrome"));
    assert_eq!(merged.net_down_bytes_per_sec, 2_000_000);
    assert!(merged.rates_ready);
}

#[test]
fn instant_sample_without_history_stays_unready() {
    let fresh = instant(10 * GB);

    let merged = SystemSnapshotMerge::merge(fresh.clone(), None);

    assert!(!merged.rates_ready);
    assert_eq!(merged, fresh);
}

#[test]
fn instant_sample_does_not_borrow_from_unready_history() {
    let merged = SystemSnapshotMerge::merge(instant(10 * GB), Some(&SystemSnapshot::empty()));

    assert!(!merged.rates_ready);
    assert_eq!(merged.cpu_percent, 0.0);
}

#[test]
fn empty_has_no_rates_and_no_instant_data() {
    assert!(!SystemSnapshot::empty().rates_ready);
    assert!(!SystemSnapshotMerge::has_instant_data(&SystemSnapshot::empty()));
    assert!(SystemSnapshotMerge::has_instant_data(&instant(10 * GB)));
}
