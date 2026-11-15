use super::notification_item::NotificationItem;
use crate::notch::{NotchCard, NotchCardKind};
use crate::text;
use crate::time::{DateTimeOffset, TimeSpan, span_between};
use crate::tray::TrayBalloonNotification;

/// Göreli zaman metinleri; Core resx'e erişemez, App çevrilmiş halini verir. Biçimler `{0}` sayısını alır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelativeTimeLabels {
    pub now: String,
    pub minutes_ago_format: String,
    pub hours_ago_format: String,
}

impl RelativeTimeLabels {
    pub fn new(now: impl Into<String>, minutes_ago_format: impl Into<String>, hours_ago_format: impl Into<String>) -> Self {
        RelativeTimeLabels {
            now: now.into(),
            minutes_ago_format: minutes_ago_format.into(),
            hours_ago_format: hours_ago_format.into(),
        }
    }
}

/// Bildirimi çentik kartına çevirir: üst satır "UYGULAMA · şimdi", başlık, gövde. Uzun metinler kısaltılır
/// (kart 380×72 sabit; tek satır, ucu "…"). Gizlilik ayarı açıkken gövde hiç karta girmez.
pub struct NotificationPresentation;

impl NotificationPresentation {
    pub const MAX_TITLE_LENGTH: usize = 60;
    pub const MAX_BODY_LENGTH: usize = 90;
    pub const MAX_APP_NAME_LENGTH: usize = 30;

    /// Tepsi balon bildirimini çentik bildirim kuyruğuna uygun öğeye çevirir.
    pub fn from_tray_balloon(balloon: &TrayBalloonNotification, id: u32) -> NotificationItem {
        NotificationItem {
            id,
            app_name: balloon.app_name.clone(),
            title: balloon.title.clone(),
            body: balloon.body.clone(),
            time: balloon.time,
            icon: balloon.icon.clone(),
            fallback_icon: None,
        }
    }

    pub fn to_card(item: &NotificationItem, now: DateTimeOffset, hide_body: bool, labels: &RelativeTimeLabels) -> NotchCard {
        let app = Self::truncate(Some(&item.app_name), Self::MAX_APP_NAME_LENGTH);
        let mut title = Self::truncate(Some(&item.title), Self::MAX_TITLE_LENGTH);
        let mut body = if hide_body { String::new() } else { Self::truncate(Some(&item.body), Self::MAX_BODY_LENGTH) };

        // Başlık boşsa (yalnız gövdeli bildirim) gövde başlığın yerine geçer; satır boş kalmasın.
        if title.is_empty() && !body.is_empty() {
            title = std::mem::take(&mut body);
        }

        let header = if app.is_empty() {
            Self::format_age(item.time, now, labels)
        } else {
            format!("{app} · {}", Self::format_age(item.time, now, labels))
        };

        let mut card = NotchCard::new(NotchCardKind::Notification, title, body);
        card.app_name = app;
        card.timestamp = Some(item.time);
        card.icon = item.icon.clone();
        card.fallback_icon = item.fallback_icon.clone();
        card.header = header;
        card
    }

    /// "şimdi" (1 dk altı), "{n} dk önce" (1 saat altı), "{n} sa önce".
    pub fn format_age(time: DateTimeOffset, now: DateTimeOffset, labels: &RelativeTimeLabels) -> String {
        let age = span_between(now, time);
        if age < TimeSpan::from_mins(1) {
            return labels.now.clone();
        }

        if age < TimeSpan::from_hours(1) {
            return format_count(&labels.minutes_ago_format, age.total_minutes() as i32);
        }

        format_count(&labels.hours_ago_format, age.total_hours() as i32)
    }

    /// Satır sonlarını ve art arda boşlukları tek boşluğa indirir, uzunsa sondan "…" ile keser.
    /// Uzunluklar C#'taki gibi UTF-16 birimi cinsindendir.
    pub fn truncate(text: Option<&str>, max_length: usize) -> String {
        let text = match text {
            Some(t) if !text::is_null_or_white_space(Some(t)) => t,
            _ => return String::new(),
        };

        let mut normalized = String::new();
        let mut last_was_space = false;
        for ch in text::trim(text).chars() {
            if ch.is_whitespace() || ch.is_control() {
                if !last_was_space {
                    normalized.push(' ');
                }
                last_was_space = true;
                continue;
            }

            last_was_space = false;
            normalized.push(ch);
        }

        let units: Vec<u16> = normalized.encode_utf16().collect();
        if units.len() <= max_length {
            return normalized;
        }

        // Vekil çiftin ortasından kesme.
        let mut cut = max_length as isize - 1;
        if cut > 0 && is_high_surrogate(units[cut as usize - 1]) {
            cut -= 1;
        }
        // C# `normalized[..cut]`: negatif kesim ArgumentOutOfRangeException fırlatır.
        let cut = usize::try_from(cut).expect("kesim konumu negatif olamaz (maxLength >= 1 olmalı)");
        let head = String::from_utf16_lossy(&units[..cut]);
        format!("{}…", head.trim_end())
    }
}

fn is_high_surrogate(unit: u16) -> bool {
    (0xD800..=0xDBFF).contains(&unit)
}

/// `string.Format(CultureInfo.InvariantCulture, format, n)`: `{0}` yerine sayı, `{{` / `}}` kaçışları.
fn format_count(format: &str, n: i32) -> String {
    let mut out = String::new();
    let mut chars = format.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                // `{0}` (ya da `{0:...}` / `{0,n}` biçimleri resx'te kullanılmaz; yalnız `{0}` desteklenir).
                let mut spec = String::new();
                for s in chars.by_ref() {
                    if s == '}' {
                        break;
                    }
                    spec.push(s);
                }
                if spec == "0" {
                    out.push_str(&n.to_string());
                } else {
                    panic!("geçersiz biçim belirtimi: {{{spec}}} (FormatException)");
                }
            }
            _ => out.push(c),
        }
    }
    out
}
