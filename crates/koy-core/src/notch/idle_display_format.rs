use chrono::NaiveDateTime;

use crate::clock::{format_date_time, ClockFormatPolicy, Culture, DayOfWeek};
use crate::text;

/// Halka modundaki boşta metinleri ve halka geometrisi. XAML yalnız bağlanır; biçim ve çizim burada, testli.
pub struct IdleDisplayFormat;

impl IdleDisplayFormat {
    /// Halkanın kutusu (DIP).
    pub const RING_SIZE: f64 = 18.0;

    /// Halkanın yarıçapı: kalınlık 2,5 olduğundan 18 px kutuya taşmadan sığar (9 - 2,5 / 2 - 0,5).
    pub const RING_RADIUS: f64 = 7.25;

    /// Halka kalınlığı (DIP).
    pub const RING_THICKNESS: f64 = 2.5;

    /// Bu yüzde ve altında, şarj olmuyorsa halka düşük pil rengine geçer.
    pub const LOW_BATTERY_PERCENT: i32 = 20;

    /// Kısa gün adı ve saat: "Sal 19:21" / "Tue 7:21 PM". Desen `ClockFormatPolicy::resolve_time_pattern`'den gelir.
    /// C# varsayılanı `time_pattern = "HH:mm"`.
    pub fn day_time(now: NaiveDateTime, culture: &Culture, time_pattern: &str) -> String {
        let day = DayOfWeek::of(now.date()).index() as usize;
        let time = format_date_time(now, None, time_pattern, culture).expect("geçersiz saat deseni (FormatException)");
        format!("{} {}", culture.abbreviated_day_names[day], time)
    }

    /// Peek'te saatin yanındaki kısa tarih, dilin gün-ay sırasıyla: "18 Kas" / "Nov 18". Geçerli özel biçim verilirse
    /// o kullanılır (`ClockFormatPolicy::is_valid_custom_date_format`); geçersizse yerleşik biçim.
    pub fn short_date(now: NaiveDateTime, culture: &Culture, custom_format: Option<&str>) -> String {
        if ClockFormatPolicy::is_valid_custom_date_format(custom_format) {
            return format_date_time(now, None, custom_format.unwrap(), culture).expect("geçersiz tarih biçimi (FormatException)");
        }
        let pattern = culture.month_day_pattern.replace("MMMM", "MMM");
        format_date_time(now, None, &pattern, culture).expect("geçersiz tarih deseni (FormatException)")
    }

    /// Yüzde, dilin yazımıyla: "%43" (Türkçe) / "43%" (İngilizce).
    pub fn percent(percent: i32, culture: &Culture) -> String {
        let n = text::clamp(percent, 0, 100).to_string();
        let sign = "%";
        // `NumberFormat.PercentPositivePattern`: Culture'da yok; gömülü kültürlerin değerleri
        // (tr-TR "%n" = 2, de-DE "n %" = 0, diğerleri "n%" = 1).
        let pattern = percent_positive_pattern(culture);
        match pattern {
            0 => format!("{n} {sign}"),
            2 => format!("{sign}{n}"),
            3 => format!("{sign} {n}"),
            _ => format!("{n}{sign}"),
        }
    }

    /// Halka düşük pil renginde mi: şarj olmuyor ve yüzde eşikte ya da altında.
    pub fn is_low(percent: i32, charging: bool) -> bool {
        !charging && percent <= Self::LOW_BATTERY_PERCENT
    }

    /// Halkanın dolu kısmının yay sonundaki nokta. Yay en üstten (saat 12) başlar ve saat yönünde ilerler.
    pub fn arc_point(percent: f64, radius: f64, center: f64) -> (f64, f64) {
        let angle = text::clamp(percent, 0.0, 100.0) / 100.0 * 2.0 * std::f64::consts::PI;
        (center + radius * angle.sin(), center - radius * angle.cos())
    }

    /// Halkanın dolu kısmı için yol verisi (WPF / SVG yol söz dizimi). %0'da boş metin (çizilecek yay yok); %100'de tek
    /// yay tam daire çizemediği için iki yarım yay. C# varsayılanları: `size = RING_SIZE`, `radius = RING_RADIUS`.
    pub fn arc_path(percent: i32, size: f64, radius: f64) -> String {
        let pct = text::clamp(percent, 0, 100);
        if pct == 0 {
            return String::new();
        }

        let c = size / 2.0;
        let top = c - radius;
        let r = f(radius);
        if pct == 100 {
            return format!("M {},{} A {r},{r} 0 1 1 {},{} A {r},{r} 0 1 1 {},{}", f(c), f(top), f(c), f(c + radius), f(c), f(top));
        }

        let (x, y) = Self::arc_point(pct as f64, radius, c);
        let large = if pct > 50 { 1 } else { 0 };
        format!("M {},{} A {r},{r} 0 {large} 1 {},{}", f(c), f(top), f(x), f(y))
    }
}

/// `Math.Round(value, 3).ToString("0.###", InvariantCulture)`.
fn f(value: f64) -> String {
    let rounded = text::round_digits(value, 3);
    let s = format!("{rounded:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" || s == "-0" { "0".to_string() } else { s.to_string() }
}

fn percent_positive_pattern(culture: &Culture) -> i32 {
    match culture.name.to_ascii_lowercase().as_str() {
        n if n.starts_with("tr") => 2,
        n if n.starts_with("de") => 0,
        _ => 1,
    }
}
