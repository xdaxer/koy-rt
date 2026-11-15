use super::DisplayMonitor;
use crate::text;

/// Ekran düzeninin imzası: kimlik, sınırlar, DPI ve ana ekran. ÇALIŞMA ALANI bilerek yoktur: dock kendi AppBar'ıyla
/// çalışma alanını değiştirir, Gezgin de görev çubuğunu gösterip gizleyince oynatır; imzaya girseydi her çalışma alanı
/// oynaması "ekran düzeni değişti" sayılır, dock görev çubuğunu yeniden gizler (ABM_SETSTATE) ve bu Gezgin'e çalışma
/// alanını yeniden hesaplatıp döngüye girerdi (büyütülmüş pencereler sürekli yeniden boyutlanıyordu).
pub struct MonitorLayoutSignature;

impl MonitorLayoutSignature {
    pub fn compute(monitors: &[DisplayMonitor]) -> String {
        let mut list: Vec<&DisplayMonitor> = monitors.iter().collect();
        list.sort_by(|a, b| text::cmp_ignore_case(&a.id, &b.id));
        list.iter()
            .map(|m| {
                // Bounds: C# record struct ToString biçimi; IsPrimary: True/False; F2: .NET biçimi.
                format!(
                    "{};WindowRect {{ Left = {}, Top = {}, Right = {}, Bottom = {} }};{};{}",
                    m.id,
                    m.bounds.left,
                    m.bounds.top,
                    m.bounds.right,
                    m.bounds.bottom,
                    format_f2(m.dpi_scale_x),
                    if m.is_primary { "True" } else { "False" }
                )
            })
            .collect::<Vec<_>>()
            .join("|")
    }
}

/// .NET `ToString("F2")`: tam yarımda sıfırdan uzağa yuvarlar (Rust `{:.2}` yarımda çifte yuvarlar).
fn format_f2(value: f64) -> String {
    if !value.is_finite() {
        return if value.is_nan() {
            "NaN".into()
        } else if value > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    let exact = format!("{:.30}", value.abs());
    let dot = exact.find('.').unwrap();
    let tail = &exact[dot + 3..];
    let is_tie = tail.starts_with('5') && tail[1..].bytes().all(|b| b == b'0');
    let nudged = if is_tie { value + value.signum() * 1e-9 } else { value };
    format!("{:.2}", nudged)
}
