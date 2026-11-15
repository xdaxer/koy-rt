use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

/// .NET `Guid` karşılığı (pano kimliği). JSON'da `"xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"` (biçim "D") yazılır.
/// Bayt düzeni metindeki sırayla aynıdır (Windows GUID yapısının karışık sıralaması gerekmez).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct Guid(pub [u8; 16]);

impl Guid {
    /// `Guid.Empty`.
    pub const EMPTY: Guid = Guid([0; 16]);

    /// `Guid.NewGuid()`: rastgele (sürüm 4 / varyant 1) kimlik. Kaynak: işletim sistemi tohumlu `RandomState`,
    /// zaman ve sayaç karışımı (kimlik çakışmasız olsun yeter, kriptografik değil).
    pub fn new_guid() -> Guid {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let mut bytes = [0u8; 16];
        for chunk in bytes.chunks_mut(8) {
            let mut h = RandomState::new().build_hasher();
            h.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default();
            h.write_u128(nanos);
            chunk.copy_from_slice(&h.finish().to_le_bytes());
        }
        bytes[6] = (bytes[6] & 0x0F) | 0x40;
        bytes[8] = (bytes[8] & 0x3F) | 0x80;
        Guid(bytes)
    }

    pub fn is_empty(&self) -> bool {
        *self == Guid::EMPTY
    }
}

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let b = &self.0;
        write!(
            f,
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
        )
    }
}

impl FromStr for Guid {
    type Err = String;

    /// `Guid.Parse` (tireli "D" biçimi; ayraçlı biçimler de kabul edilir).
    fn from_str(s: &str) -> Result<Guid, String> {
        let t = s.trim().trim_start_matches(['{', '(']).trim_end_matches(['}', ')']);
        let hex: String = t.chars().filter(|c| *c != '-').collect();
        if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("Geçersiz Guid: {s}"));
        }
        let mut bytes = [0u8; 16];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|e| e.to_string())?;
        }
        Ok(Guid(bytes))
    }
}

impl Serialize for Guid {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Guid {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Guid, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}
