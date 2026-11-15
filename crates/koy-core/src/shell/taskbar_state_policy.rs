use chrono::{NaiveDateTime, Timelike};

use crate::text;

/// Diske yazılan görev çubuğu durumu: gizlendi mi ve gizlemeden önceki ABS_* bayrakları.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskbarSavedState {
    pub hidden: bool,
    pub previous_state: i32,
}

impl TaskbarSavedState {
    pub const fn new(hidden: bool, previous_state: i32) -> Self {
        TaskbarSavedState { hidden, previous_state }
    }
}

/// Görev çubuğunu gizleme/geri getirme kararları (saf mantık, Win32 yok).
/// Amaç: Hide() ikinci kez çağrılsa bile kullanıcının gerçek (önceki) durumu, bizim yazdığımız AUTOHIDE ile ezilmesin.
pub struct TaskbarStatePolicy;

impl TaskbarStatePolicy {
    pub const AUTO_HIDE: i32 = 0x1;
    pub const ALWAYS_ON_TOP: i32 = 0x2;

    /// Gizlemeden önceki durumu seçer. Zaten gizliyken (durum dosyası "hidden") görev çubuğunun
    /// o anki durumu bizim koyduğumuz AUTOHIDE'dır; onu "önceki durum" sayma, kayıtlıyı koru.
    pub fn choose_previous_state(saved: Option<TaskbarSavedState>, current_state: i32) -> i32 {
        match saved {
            Some(s) if s.hidden => s.previous_state,
            _ => current_state,
        }
    }

    /// Geri getirirken uygulanacak ABS_* değeri. Kayıt yoksa ya da geçersizse AlwaysOnTop (güvenli varsayılan).
    pub fn resolve_restore_state(saved: Option<TaskbarSavedState>) -> i32 {
        match saved {
            Some(s) if s.previous_state > 0 => s.previous_state,
            _ => Self::ALWAYS_ON_TOP,
        }
    }

    /// Geri yükleme sırasında ABM_SETSTATE gönderilmeli mi?
    /// Koy görev çubuğunu gerçekten gizlemişse (`saved` hidden) veya kullanıcı kurtarma amacıyla
    /// zorlamışsa (`force: true`) yazılır. Aksi halde Windows'un mevcut durumuna dokunulmaz.
    /// C#'ta `force` varsayılanı `false`.
    pub fn should_write_state(saved: Option<TaskbarSavedState>, force: bool) -> bool {
        force || matches!(saved, Some(s) if s.hidden)
    }

    /// `utc_now`: UTC zamanı (C#'ta `DateTime.UtcNow`, "O" biçimi sonuna `Z` alır).
    pub fn serialize(state: TaskbarSavedState, utc_now: NaiveDateTime) -> String {
        let mut sb = String::new();
        sb.push_str("{\"taskbarHidden\": ");
        sb.push_str(if state.hidden { "true" } else { "false" });
        sb.push_str(", \"previousState\": ");
        sb.push_str(&state.previous_state.to_string());
        sb.push_str(", \"timestamp\": \"");
        sb.push_str(&format_round_trip_utc(utc_now));
        sb.push_str("\"}");
        sb
    }

    /// Bozuk ya da eksik içerikte `None` döner (çağıran güvenli varsayılana düşer).
    pub fn parse(content: Option<&str>) -> Option<TaskbarSavedState> {
        if text::is_null_or_white_space(content) {
            return None;
        }
        let content = content?;

        let hidden = matches!(read_token(content, "\"taskbarHidden\":"), Some(h) if text::eq_ignore_case(h, "true"));

        // `NumberStyles.Integer`: baştaki/sondaki boşluk ve işaret; `read_token` zaten kırpıyor.
        let previous = read_token(content, "\"previousState\":")?.parse::<i32>().ok()?;

        Some(TaskbarSavedState { hidden, previous_state: previous })
    }
}

/// `DateTime.ToString("O", InvariantCulture)`, `DateTimeKind.Utc` için: `yyyy-MM-ddTHH:mm:ss.fffffffZ`.
fn format_round_trip_utc(t: NaiveDateTime) -> String {
    use chrono::Datelike;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:07}Z",
        t.year(),
        t.month(),
        t.day(),
        t.hour(),
        t.minute(),
        t.second(),
        // 100 ns'lik tick (`Timelike::nanosecond` sıçrama saniyesinde 1e9'u aşabilir, o yüzden mod).
        (t.nanosecond() % 1_000_000_000) / 100
    )
}

fn read_token<'a>(content: &'a str, key: &str) -> Option<&'a str> {
    let idx = text::find_ignore_case(content, key)?;

    let start = idx + key.len();
    // C# ile aynı: `end > start` değilse (anahtardan hemen sonra ayraç ya da hiç ayraç yok) kalan metnin tamamı alınır.
    let end = content[start..].find([',', '}']).map(|e| start + e);
    let token = match end {
        Some(end) if end > start => &content[start..end],
        _ => &content[start..],
    };
    Some(text::trim(token))
}
