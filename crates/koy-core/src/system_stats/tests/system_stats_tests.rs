use std::collections::HashMap;

use crate::clock::Culture;
use crate::system_stats::*;
use crate::units::NetworkSpeedUnit;

// ---- SystemStatsMathTests ----

/// xUnit `Assert.Equal(expected, actual, precision)`: 6 ondalık basamağa yuvarlayıp karşılaştırır.
fn assert_equal_precision(expected: f64, actual: f64, precision: i32) {
    let p = 10f64.powi(precision);
    assert_eq!((expected * p).round(), (actual * p).round(), "beklenen {expected}, gelen {actual}");
}

#[test]
fn cpu_is_busy_share_where_kernel_includes_idle() {
    // delta: kernel 800 (idle 600 dahil), user 200 -> toplam 1000, boşta 600, meşgul 400
    assert_equal_precision(40.0, SystemStatsMath::cpu_percent(0, 0, 0, 600, 800, 200), 6);
}

#[test]
fn cpu_is_zero_without_elapsed_time_or_when_counters_go_back() {
    assert_eq!(SystemStatsMath::cpu_percent(10, 10, 10, 10, 10, 10), 0.0);
    assert_eq!(SystemStatsMath::cpu_percent(100, 100, 100, 50, 50, 50), 0.0);
}

#[test]
fn cpu_is_clamped_to_0_100() {
    assert_eq!(SystemStatsMath::cpu_percent(0, 0, 0, 0, 100, 0), 100.0);
    assert_eq!(SystemStatsMath::cpu_percent(0, 0, 0, 100, 100, 0), 0.0);
}

#[test]
fn bytes_per_second_divides_by_elapsed() {
    assert_eq!(SystemStatsMath::bytes_per_second(1000, 3000, 2.0), 1000);
}

#[test]
fn bytes_per_second_ignores_counter_reset_and_bad_time() {
    assert_eq!(SystemStatsMath::bytes_per_second(5000, 100, 1.0), 0);
    assert_eq!(SystemStatsMath::bytes_per_second(0, 100, 0.0), 0);
}

#[test]
fn percent_handles_zero_total() {
    assert_eq!(SystemStatsMath::percent(5.0, 0.0), 0.0);
    assert_eq!(SystemStatsMath::percent(8.0, 16.0), 50.0);
}

#[test]
fn process_cpu_divides_by_cores() {
    // 1 sn'de 1 çekirdek tam dolu, 4 çekirdek -> %25
    assert_equal_precision(25.0, SystemStatsMath::process_cpu_percent(1.0, 1.0, 4), 6);
    assert_eq!(SystemStatsMath::process_cpu_percent(-1.0, 1.0, 4), 0.0);
}

// ---- TopProcessPickerTests ----

#[test]
fn picks_largest_delta_and_ignores_idle_and_new_processes() {
    let previous: HashMap<i32, f64> = HashMap::from([(10, 5.0), (20, 5.0), (0, 0.0)]);
    let now = vec![
        ProcessCpuSample::new(0, "Idle", 999.0),
        ProcessCpuSample::new(10, "a", 5.5),
        ProcessCpuSample::new(20, "b", 7.0),
        ProcessCpuSample::new(30, "yeni", 500.0),
    ];

    let top = TopProcessPicker::pick(&previous, &now, 3.0, 2);

    let (name, percent) = top.expect("en çok CPU kullanan süreç bulunmalı");
    assert_eq!(name, "b");
    assert_equal_precision(2.0 * 100.0 / (3.0 * 2.0), percent, 6);
}

#[test]
fn returns_null_when_nothing_used_cpu() {
    let previous: HashMap<i32, f64> = HashMap::from([(10, 5.0)]);
    assert!(TopProcessPicker::pick(&previous, &[ProcessCpuSample::new(10, "a", 5.0)], 3.0, 4).is_none());
}

// ---- SystemFormatTests ----

fn tr() -> Culture {
    Culture::tr_tr()
}

fn en() -> Culture {
    Culture::en_us()
}

#[test]
fn speed() {
    let cases: [(i64, &str); 8] = [
        (0, "0 B/sn"),
        (512, "512 B/sn"),
        (360 * 1024, "360 KB/sn"),
        (2621440, "2,5 MB/sn"),
        (2 * 1024 * 1024, "2 MB/sn"),
        (1536, "1,5 KB/sn"),
        (1024i64 * 1024 * 1024 * 3, "3 GB/sn"),
        (-5, "0 B/sn"),
    ];
    for (bytes, expected) in cases {
        assert_eq!(SystemFormat::speed(bytes, &tr()), expected, "bytes = {bytes}");
    }
}

#[test]
fn speed_rounding_rolls_over_to_next_unit() {
    assert_eq!(SystemFormat::speed(1024 * 1024 - 1, &tr()), "1 MB/sn");
}

#[test]
fn speed_in_bits_uses_decimal_prefixes() {
    let cases: [(i64, &str); 6] = [
        (0, "0 b/sn"),
        (100, "800 b/sn"),
        (45_000, "360 kb/sn"),
        (312_500, "2,5 Mb/sn"),
        (150_000_000, "1,2 Gb/sn"),
        (-5, "0 b/sn"),
    ];
    for (bytes, expected) in cases {
        assert_eq!(SystemFormat::speed_with_unit(bytes, NetworkSpeedUnit::Bits, &tr()), expected, "bytes = {bytes}");
    }
}

#[test]
fn speed_in_bits_rolls_over_and_uses_language_suffix() {
    assert_eq!(SystemFormat::speed_with_unit(124_999, NetworkSpeedUnit::Bits, &en()), "1 Mb/s");
    assert_eq!(SystemFormat::speed_with_unit(187, NetworkSpeedUnit::Bits, &en()), "1.5 kb/s");
}

#[test]
fn speed_in_bytes_unit_matches_the_binary_formatter() {
    assert_eq!(
        SystemFormat::speed(2621440, &tr()),
        SystemFormat::speed_with_unit(2621440, NetworkSpeedUnit::Bytes, &tr())
    );
}

#[test]
fn memory_uses_one_decimal_for_used_and_trims_whole_total() {
    const GB: i64 = 1024 * 1024 * 1024;
    assert_eq!(SystemFormat::memory((9.2 * GB as f64) as i64, 16 * GB, &tr()), "9,2 / 16 GB");
    assert_eq!(SystemFormat::memory(9 * GB, (15.8 * GB as f64) as i64, &tr()), "9,0 / 15,8 GB");
}

#[test]
fn size() {
    let cases: [(i64, &str); 3] = [(0, "0 B"), (150i64 * 1024 * 1024, "150 MB"), (1503238554, "1,4 GB")];
    for (bytes, expected) in cases {
        assert_eq!(SystemFormat::size(bytes, &tr()), expected, "bytes = {bytes}");
    }
}

#[test]
fn percent() {
    let cases: [(f64, &str); 4] = [(42.4, "%42"), (42.5, "%43"), (-3.0, "%0"), (140.0, "%100")];
    for (value, expected) in cases {
        assert_eq!(SystemFormat::percent(value, &tr()), expected, "value = {value}");
    }
}

#[test]
fn english_uses_decimal_point_seconds_and_trailing_percent() {
    const GB: i64 = 1024 * 1024 * 1024;
    assert_eq!(SystemFormat::speed(2621440, &en()), "2.5 MB/s");
    assert_eq!(SystemFormat::speed(2 * 1024 * 1024, &en()), "2 MB/s");
    assert_eq!(SystemFormat::size(1503238554, &en()), "1.4 GB");
    assert_eq!(SystemFormat::memory((9.2 * GB as f64) as i64, 16 * GB, &en()), "9.2 / 16 GB");
    assert_eq!(SystemFormat::percent(42.4, &en()), "42%");
}
