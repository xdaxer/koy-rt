use std::io;
use std::path::{Path, PathBuf};

use serde::{Serialize, Serializer};

use super::calendar_event::{CalendarEvent, RawCalendarEvent};
use crate::text;

/// `EventJsonContext`: enum dönüştürücüsü yok, `Source` sayı olarak yazılır/okunur; girinti açık.
struct NumericSourceEvent<'a>(&'a CalendarEvent);

impl Serialize for NumericSourceEvent<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize_with(serializer, true)
    }
}

/// Etkinlikleri `%AppData%\Koy\events.json` dosyasında saklar (raf ve ayarlarla aynı klasör).
pub struct EventStore {
    path: PathBuf,
}

impl EventStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        EventStore { path: path.into() }
    }

    pub fn default_path() -> PathBuf {
        PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join("Koy").join("events.json")
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Dosya yoksa, okunamıyorsa ya da bozuksa boş liste; takvim açılışı asla bozuk dosyada çökmez.
    pub fn load(&self) -> Vec<CalendarEvent> {
        self.try_load().unwrap_or_default()
    }

    fn try_load(&self) -> Option<Vec<CalendarEvent>> {
        if !self.path.exists() {
            return Some(Vec::new());
        }

        let bytes = std::fs::read(&self.path).ok()?;
        let list: Option<Vec<RawCalendarEvent>> = serde_json::from_slice(&bytes).ok()?;
        let mut events = Vec::new();
        for raw in list.unwrap_or_default() {
            let event = raw.into_event(true).ok()?;
            if !text::is_null_or_white_space(Some(&event.id)) && !text::is_null_or_white_space(Some(&event.title)) {
                events.push(event);
            }
        }
        Some(events)
    }

    /// Önce geçici dosyaya yazar, sonra yerine taşır; yazma yarıda kesilirse eski dosya bozulmaz.
    pub fn save(&self, events: &[CalendarEvent]) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut temp = self.path.clone().into_os_string();
        temp.push(".tmp");
        let temp = PathBuf::from(temp);

        let rows: Vec<NumericSourceEvent<'_>> = events.iter().map(NumericSourceEvent).collect();
        let json = serde_json::to_string_pretty(&rows).map_err(io::Error::other)?;
        std::fs::write(&temp, json)?;
        std::fs::rename(&temp, &self.path)
    }
}
