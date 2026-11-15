//! C# `System.Guid` karşılığı (yalnız görev kimliği için gereken kadar): 128 bit değer, "D" biçiminde metin.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// `Guid`: 16 bayt. Metin biçimi `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx` (küçük harf), `System.Text.Json` ile aynı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Guid(pub u128);

impl Guid {
    /// `Guid.Empty`.
    pub const EMPTY: Guid = Guid(0);

    /// `Guid.NewGuid()`: rastgele (sürüm 4, varyant 1) kimlik; rastgelelik `crate::random` üzerinden.
    pub fn new_guid() -> Guid {
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&crate::random::get_bytes(16));
        bytes[6] = (bytes[6] & 0x0F) | 0x40;
        bytes[8] = (bytes[8] & 0x3F) | 0x80;
        Guid(u128::from_be_bytes(bytes))
    }

    /// `ToString("N")`: 32 küçük onaltılık hane, tiresiz.
    pub fn to_string_n(&self) -> String {
        format!("{:032x}", self.0)
    }
}

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = format!("{:032x}", self.0);
        write!(f, "{}-{}-{}-{}-{}", &n[0..8], &n[8..12], &n[12..16], &n[16..20], &n[20..32])
    }
}

/// `Guid.TryParse` ("D" biçimi: 8-4-4-4-12; büyük/küçük harf fark etmez).
impl FromStr for Guid {
    type Err = ();

    fn from_str(s: &str) -> Result<Guid, ()> {
        let b = s.as_bytes();
        if b.len() != 36 {
            return Err(());
        }
        let mut hex = String::with_capacity(32);
        for (i, &c) in b.iter().enumerate() {
            if matches!(i, 8 | 13 | 18 | 23) {
                if c != b'-' {
                    return Err(());
                }
            } else if c.is_ascii_hexdigit() {
                hex.push(c as char);
            } else {
                return Err(());
            }
        }
        u128::from_str_radix(&hex, 16).map(Guid).map_err(|_| ())
    }
}

impl Serialize for Guid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Guid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Guid, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse::<Guid>().map_err(|_| serde::de::Error::custom("The JSON value is not a valid Guid."))
    }
}
