//! Karşılığı: `JumpList/JumpListAppId.cs`.

use std::sync::OnceLock;

use crate::text;

const POLYNOMIAL: u64 = 0x92C64265D32139A4;

fn table() -> &'static [u64; 256] {
    static TABLE: OnceLock<[u64; 256]> = OnceLock::new();
    TABLE.get_or_init(build_table)
}

/// AppID'den atlama listesi dosya adını üretir. Windows, `%APPDATA%\Microsoft\Windows\Recent\CustomDestinations` ve
/// `AutomaticDestinations` altında dosya adı olarak AppID'nin büyük harfe çevrilmiş UTF-16LE hâlinin CRC-64'ünü kullanır:
/// polinom 0x92C64265D32139A4 (yansıtılmış), başlangıç değeri tümü 1, son XOR yok.
pub struct JumpListAppId;

impl JumpListAppId {
    /// AppID'nin dosya adı karması: küçük harfli onaltılık, **başında sıfır doldurma yok** (Windows `%x` kullanır,
    /// diskte 15 haneli adlar var). Boş AppID için `None` döner (bir dosyaya eşlenemez).
    pub fn hash_name(app_id: Option<&str>) -> Option<String> {
        if text::is_null_or_white_space(app_id) {
            return None;
        }

        // Encoding.Unicode.GetBytes: UTF-16LE.
        let upper = text::to_upper_invariant(app_id?);
        let bytes: Vec<u8> = upper.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        Some(format!("{:x}", Self::crc64(&bytes)))
    }

    pub fn crc64(data: &[u8]) -> u64 {
        let table = table();
        let mut crc = u64::MAX;
        for &b in data {
            crc = table[(crc as u8 ^ b) as usize] ^ (crc >> 8);
        }
        crc
    }
}

fn build_table() -> [u64; 256] {
    let mut table = [0u64; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut c = i as u64;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ POLYNOMIAL } else { c >> 1 };
        }
        *slot = c;
    }
    table
}
