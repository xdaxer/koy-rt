use crate::text;
use std::sync::Mutex;

/// Dosyadan okunan sonucu dosyanın son yazma zamanı ve boyutu değişmedikçe saklar;
/// büyük dosyalar panel her açıldığında yeniden okunup ayrıştırılmaz.
pub struct FileStampCache<T> {
    /// Anahtar büyük/küçük harf duyarsız (`OrdinalIgnoreCase`).
    entries: Mutex<Vec<(String, (i64, i64, T))>>,
}

impl<T> Default for FileStampCache<T> {
    fn default() -> Self {
        FileStampCache { entries: Mutex::new(Vec::new()) }
    }
}

impl<T: Clone> FileStampCache<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// `load`: yalnız damga değiştiyse ya da kayıt yoksa çağrılır.
    pub fn get_or_load(&self, key: &str, write_ticks: i64, length: i64, load: impl FnOnce() -> T) -> T {
        {
            let entries = self.entries.lock().unwrap();
            if let Some((_, (w, l, v))) = entries.iter().find(|(k, _)| text::eq_ignore_case(k, key)) {
                if *w == write_ticks && *l == length {
                    return v.clone();
                }
            }
        }

        // Ayrıştırma kilit dışında: iki panel aynı anda okursa en kötü ihtimal çift iş.
        let value = load();
        let mut entries = self.entries.lock().unwrap();
        match entries.iter_mut().find(|(k, _)| text::eq_ignore_case(k, key)) {
            Some((_, e)) => *e = (write_ticks, length, value.clone()),
            None => entries.push((key.to_string(), (write_ticks, length, value.clone()))),
        }
        value
    }

    /// Dosya artık yok: kaydı bırakır.
    pub fn remove(&self, key: &str) {
        self.entries.lock().unwrap().retain(|(k, _)| !text::eq_ignore_case(k, key));
    }
}
