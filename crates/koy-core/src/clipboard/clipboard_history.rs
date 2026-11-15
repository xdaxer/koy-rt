use super::clipboard_display::take_utf16;
use super::{ClipboardClassifier, ClipboardFilter, ClipboardItem, ClipboardKind, Guid};
use crate::text;
use crate::time::DateTimeOffset;
use std::sync::Mutex;

/// C#'taki `Func<DateTimeOffset>` saat kaynağı.
pub type NowFn = Box<dyn Fn() -> DateTimeOffset + Send + Sync>;

/// Pano geçmişinin saf bellek mantığı (Win32 yok). İş parçacığı güvenli: Win32 dinleyicisi arka planda ekler,
/// arayüz okur. En yeni başta. Sabitlenenler sınıra sayılmaz ve otomatik silinmez.
pub struct ClipboardHistory {
    now: NowFn,
    items: Mutex<Vec<ClipboardItem>>,
}

impl ClipboardHistory {
    pub const MAX_UNPINNED: i32 = 50;
    pub const MAX_TEXT_LENGTH: i32 = 10_000;

    /// `now` verilmezse `DateTimeOffset.Now` (yerel saat).
    pub fn new(now: Option<NowFn>) -> Self {
        ClipboardHistory {
            now: now.unwrap_or_else(|| Box::new(|| chrono::Local::now().fixed_offset())),
            items: Mutex::new(Vec::new()),
        }
    }

    fn now(&self) -> DateTimeOffset {
        (self.now)()
    }

    /// Metni ekler. Boş/yalnız boşluk yok sayılır (None döner). Aynı metin varsa o kayıt başa taşınır (sabit durumu
    /// korunur). Uzun metin [`Self::MAX_TEXT_LENGTH`] karaktere (UTF-16 kod birimi) kesilir. Sınır aşılırsa en eski
    /// sabitlenmemiş silinir.
    pub fn add(&self, text: Option<&str>) -> Option<ClipboardItem> {
        if text::is_null_or_white_space(text) {
            return None;
        }
        let mut text = text.unwrap_or("").to_string();
        if text.encode_utf16().count() > Self::MAX_TEXT_LENGTH as usize {
            text = take_utf16(&text, Self::MAX_TEXT_LENGTH as usize);
        }

        let mut items = self.items.lock().unwrap();
        let index = items.iter().position(|i| i.text == text);
        let item = match index {
            Some(i) => ClipboardItem { copied_at: self.now(), ..items[i].clone() },
            None => {
                let kind = ClipboardClassifier::classify(Some(&text));
                ClipboardItem::new(Guid::new_guid(), text, kind, false, self.now())
            }
        };

        if let Some(i) = index {
            items.remove(i);
        }
        items.insert(0, item.clone());
        Self::trim_locked(&mut items);
        Some(item)
    }

    /// Süzgeç + arama (büyük/küçük harf duyarsız, kültürden bağımsız). En yeni başta.
    pub fn query(&self, filter: ClipboardFilter, search: Option<&str>) -> Vec<ClipboardItem> {
        let needle = if text::is_null_or_white_space(search) { None } else { search.map(str::trim) };
        let items = self.items.lock().unwrap();
        items
            .iter()
            .filter(|i| Self::matches(i, filter))
            .filter(|i| needle.is_none_or(|n| text::contains_ignore_case(&i.text, n)))
            .cloned()
            .collect()
    }

    pub fn get(&self, id: Guid) -> Option<ClipboardItem> {
        self.items.lock().unwrap().iter().find(|i| i.id == id).cloned()
    }

    /// Sabitle / çöz. Değişiklik olduysa true. Çözünce sınır aşılmışsa en eskiler budanır.
    pub fn set_pinned(&self, id: Guid, pinned: bool) -> bool {
        let mut items = self.items.lock().unwrap();
        let Some(index) = items.iter().position(|i| i.id == id) else { return false };
        if items[index].pinned == pinned {
            return false;
        }
        items[index] = ClipboardItem { pinned, ..items[index].clone() };
        Self::trim_locked(&mut items);
        true
    }

    pub fn remove(&self, id: Guid) -> bool {
        let mut items = self.items.lock().unwrap();
        let before = items.len();
        items.retain(|i| i.id != id);
        items.len() < before
    }

    /// Sabitlenmeyenlerin hepsini siler; sabitler kalır. Silinen sayısını döner.
    pub fn clear(&self) -> i32 {
        let mut items = self.items.lock().unwrap();
        let before = items.len();
        items.retain(|i| i.pinned);
        (before - items.len()) as i32
    }

    /// Sabit öğeler (diske yazılacak olan), en yeni başta.
    pub fn pinned_snapshot(&self) -> Vec<ClipboardItem> {
        self.items.lock().unwrap().iter().filter(|i| i.pinned).cloned().collect()
    }

    /// Diskten gelen sabit öğeleri sona ekler (zaten varsa atlar). Metin sınırı yeniden uygulanır.
    pub fn load_pinned(&self, pinned: impl IntoIterator<Item = ClipboardItem>) {
        let mut items = self.items.lock().unwrap();
        for p in pinned {
            if text::is_null_or_white_space(Some(&p.text)) {
                continue;
            }
            let text = if p.text.encode_utf16().count() > Self::MAX_TEXT_LENGTH as usize {
                take_utf16(&p.text, Self::MAX_TEXT_LENGTH as usize)
            } else {
                p.text.clone()
            };
            if items.iter().any(|i| i.id == p.id || i.text == text) {
                continue;
            }
            let id = if p.id == Guid::EMPTY { Guid::new_guid() } else { p.id };
            let kind = ClipboardClassifier::classify(Some(&text));
            items.push(ClipboardItem::new(id, text, kind, true, p.copied_at));
        }
    }

    fn matches(item: &ClipboardItem, filter: ClipboardFilter) -> bool {
        match filter {
            ClipboardFilter::All => true,
            ClipboardFilter::Pinned => item.pinned,
            ClipboardFilter::Link => item.kind == ClipboardKind::Link,
            ClipboardFilter::Color => item.kind == ClipboardKind::Color,
            ClipboardFilter::Code => item.kind == ClipboardKind::Code,
        }
    }

    fn trim_locked(items: &mut Vec<ClipboardItem>) {
        let mut unpinned = 0;
        let mut i = 0;
        while i < items.len() {
            if items[i].pinned {
                i += 1;
                continue;
            }
            unpinned += 1;
            if unpinned > Self::MAX_UNPINNED {
                items.remove(i);
            } else {
                i += 1;
            }
        }
    }
}
