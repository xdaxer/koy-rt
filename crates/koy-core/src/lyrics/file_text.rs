//! `File.ReadAllText` karşılığı (C# çerçevesinden geldiği için kendi dosyası yok).

use std::io;
use std::path::Path;

/// `File.ReadAllText(path)`: BOM'a göre UTF-8 / UTF-16 LE / UTF-16 BE / UTF-32 LE / UTF-32 BE seçer (BOM atılır),
/// BOM yoksa UTF-8 okur; geçersiz baytlar U+FFFD olur (.NET ile aynı, istisna yok).
pub fn read_all_text(path: impl AsRef<Path>) -> io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(decode(&bytes))
}

fn decode(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) {
        return decode_utf32(&bytes[4..], false);
    }
    if bytes.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
        return decode_utf32(&bytes[4..], true);
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return decode_utf16(&bytes[2..], false);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return decode_utf16(&bytes[2..], true);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn decode_utf16(bytes: &[u8], big_endian: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|b| if big_endian { u16::from_be_bytes([b[0], b[1]]) } else { u16::from_le_bytes([b[0], b[1]]) })
        .collect();
    String::from_utf16_lossy(&units)
}

fn decode_utf32(bytes: &[u8], big_endian: bool) -> String {
    bytes
        .chunks_exact(4)
        .map(|b| {
            let v = if big_endian { u32::from_be_bytes([b[0], b[1], b[2], b[3]]) } else { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) };
            char::from_u32(v).unwrap_or('\u{FFFD}')
        })
        .collect()
}
