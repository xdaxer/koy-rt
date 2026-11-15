//! Karşılığı: `koy/src/Core/Updates/UpdateFlowPolicy.cs`.

use std::cmp::Ordering;

use crate::text::is_null_or_white_space;

/// Güncelleme akışı, sürüm karşılaştırması ve "Neler Geldi" gösterim kararları.
/// Saf mantık, Win32 ve UI bağımlılığı yoktur.
pub struct UpdateFlowPolicy;

/// `System.Version`: 2 ila 4 bileşen; tanımsız bileşen -1 sayılır (bu yüzden `0.3` < `0.3.0`, C# ile aynı).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Version {
    major: i32,
    minor: i32,
    build: i32,
    revision: i32,
}

impl Version {
    fn compare_to(&self, other: &Version) -> i32 {
        let a = [self.major, self.minor, self.build, self.revision];
        let b = [other.major, other.minor, other.build, other.revision];
        match a.cmp(&b) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        }
    }
}

/// `int.Parse`'ın kabul ettiği boşluklar: 0x20 ve 0x09-0x0D.
fn is_number_white(c: char) -> bool {
    c == ' ' || ('\u{9}'..='\u{D}').contains(&c)
}

/// `int.TryParse(s, out _)` (NumberStyles.Integer: baştaki/sondaki boşluk ve baştaki işaret).
fn try_parse_int(s: &str) -> Option<i32> {
    let t = s.trim_matches(is_number_white);
    let (negative, digits) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let magnitude: i64 = digits.parse().ok()?;
    let value = if negative { -magnitude } else { magnitude };
    i32::try_from(value).ok()
}

/// `Version.TryParse`: bileşenler negatif olamaz.
fn try_parse_version(s: &str) -> Option<Version> {
    let parts: Vec<&str> = s.split('.').collect();
    if !(2..=4).contains(&parts.len()) {
        return None;
    }
    let mut values = [-1i32; 4];
    for (i, part) in parts.iter().enumerate() {
        let v = try_parse_int(part)?;
        if v < 0 {
            return None;
        }
        values[i] = v;
    }
    Some(Version { major: values[0], minor: values[1], build: values[2], revision: values[3] })
}

impl UpdateFlowPolicy {
    /// İki sürüm dizesini semantik olarak karşılaştırır.
    /// versionA > versionB ise pozitif, eşitse 0, küçükse negatif döner.
    pub fn compare_versions(version_a: Option<&str>, version_b: Option<&str>) -> i32 {
        let va = parse_version(version_a);
        let vb = parse_version(version_b);

        let (va, vb) = match (va, vb) {
            (None, None) => return 0,
            (None, Some(_)) => return -1,
            (Some(_), None) => return 1,
            (Some(a), Some(b)) => (a, b),
        };

        let core = va.compare_to(&vb);
        if core != 0 {
            return core;
        }

        // SemVer: çekirdek eşitse ön sürüm (0.3.0-rc.2) son sürümden (0.3.0) küçüktür.
        compare_prerelease(prerelease(version_a), prerelease(version_b))
    }

    /// Mevcut sürüme göre yeni bir sürümün daha ileri olup olmadığını denetler.
    pub fn is_newer_version(current_version: &str, available_version: &str) -> bool {
        Self::compare_versions(Some(available_version), Some(current_version)) > 0
    }

    /// Yeni sürüm sonrası "Neler Geldi" (What's New) ekranı gösterilmeli mi?
    /// İlk kurulumda (first_run_completed = false) kurulum sihirbazı çalıştığı için gösterilmez.
    /// Daha önce sürüm kaydı yoksa (eski sürümden geçiş) veya mevcut sürüm kayıtlı sürümden büyükse gösterilir.
    /// C# varsayılanı: `firstRunCompleted = true`.
    pub fn should_show_whats_new(current_version: &str, last_seen_version: Option<&str>, first_run_completed: bool) -> bool {
        if !first_run_completed {
            return false;
        }

        if is_null_or_white_space(last_seen_version) {
            return true;
        }

        Self::compare_versions(Some(current_version), last_seen_version) > 0
    }

    /// Sürüm numarasını "0.x.y" veya "v0.x.y" formatına temizler. C# varsayılanı: `prefixWithV = false`.
    pub fn format_display_version(version: Option<&str>, prefix_with_v: bool) -> String {
        let version = match version {
            Some(v) if !is_null_or_white_space(Some(v)) => v,
            _ => return String::new(),
        };

        let mut s = version.trim().trim_start_matches(['v', 'V']);
        if let Some(plus) = s.find('+') {
            s = &s[..plus];
        }

        if prefix_with_v { format!("v{s}") } else { s.to_string() }
    }
}

fn prerelease(v: Option<&str>) -> Option<&str> {
    let v = v.filter(|v| !is_null_or_white_space(Some(v)))?;

    let mut s = v.trim();
    if let Some(plus) = s.find('+') {
        s = &s[..plus];
    }
    match s.find('-') {
        Some(dash) if dash < s.len() - 1 => Some(&s[dash + 1..]),
        _ => None,
    }
}

/// `string.CompareOrdinal`: UTF-16 kod birimi sırası (Rust'ın UTF-8 bayt sırasından astral karakterlerde ayrılır).
fn compare_ordinal(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

fn compare_prerelease(a: Option<&str>, b: Option<&str>) -> i32 {
    let (a, b) = match (a, b) {
        (None, None) => return 0,
        (None, Some(_)) => return 1,
        (Some(_), None) => return -1,
        (Some(a), Some(b)) => (a, b),
    };

    let pa: Vec<&str> = a.split('.').collect();
    let pb: Vec<&str> = b.split('.').collect();
    for i in 0..pa.len().min(pb.len()) {
        let ia = try_parse_int(pa[i]);
        let ib = try_parse_int(pb[i]);
        let c = match (ia, ib) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => compare_ordinal(pa[i], pb[i]),
        };
        if c != Ordering::Equal {
            return match c {
                Ordering::Less => -1,
                _ => 1,
            };
        }
    }

    match pa.len().cmp(&pb.len()) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

fn parse_version(v: Option<&str>) -> Option<Version> {
    let v = v.filter(|v| !is_null_or_white_space(Some(v)))?;

    let mut s = v.trim().trim_start_matches(['v', 'V']);
    if let Some(plus) = s.find('+') {
        s = &s[..plus];
    }
    if let Some(dash) = s.find('-') {
        s = &s[..dash];
    }

    if let Some(parsed) = try_parse_version(s) {
        return Some(parsed);
    }

    // C# ile aynı: Version.TryParse'ın reddettiği iki bileşenli dize burada yeniden denenir
    // (aynı ayrıştırıcı olduğu için pratikte hep null döner).
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() == 2 {
        if let (Some(maj), Some(min)) = (try_parse_int(parts[0]), try_parse_int(parts[1])) {
            // `new Version(maj, min, 0)`: negatif bileşen ArgumentOutOfRangeException fırlatır.
            assert!(maj >= 0 && min >= 0, "Version bileşenleri negatif olamaz");
            return Some(Version { major: maj, minor: min, build: 0, revision: -1 });
        }
    }

    None
}
