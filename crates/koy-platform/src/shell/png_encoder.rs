//! RGBA pikselleri en küçük geçerli PNG'ye yazar. Platform arayüz çatısına bağımlı olmadığı için saydamlığı koruyan bir
//! biçimi burada kendimiz üretiyoruz.
//!
//! Sapma: C# `ZLibStream(CompressionLevel.Fastest)` kullanır; yeni crate eklememek için burada zlib akışı sıkıştırmasız
//! (stored) deflate bloklarıyla yazılır. Çıktı geçerli PNG'dir, piksel verisi aynıdır; yalnız dosya biraz büyüktür
//! (48 px simgede ~9 KB).

use std::sync::OnceLock;

fn crc_table() -> &'static [u32; 256] {
    static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0u32; 256];
        for (n, slot) in table.iter_mut().enumerate() {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *slot = c;
        }
        table
    })
}

fn crc(mut crc: u32, data: &[u8]) -> u32 {
    let table = crc_table();
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc
}

fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let c = crc(crc(0xFFFF_FFFF, kind), data) ^ 0xFFFF_FFFF;
    out.extend_from_slice(&c.to_be_bytes());
}

/// zlib çerçeveli, sıkıştırmasız deflate akışı (RFC 1950/1951).
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / 65535 * 5 + 11);
    out.extend_from_slice(&[0x78, 0x01]);
    let mut chunks = data.chunks(65535).peekable();
    if chunks.peek().is_none() {
        out.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(chunk) = chunks.next() {
        let last = chunks.peek().is_none();
        out.push(u8::from(last));
        let len = chunk.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(chunk);
    }
    // Adler-32.
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    out.extend_from_slice(&((b << 16) | a).to_be_bytes());
    out
}

pub struct PngEncoder;

impl PngEncoder {
    pub fn encode(rgba: &[u8], width: i32, height: i32) -> Vec<u8> {
        let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut header = [0u8; 13];
        header[..4].copy_from_slice(&width.to_be_bytes());
        header[4..8].copy_from_slice(&height.to_be_bytes());
        header[8] = 8; // bit derinliği
        header[9] = 6; // RGBA
        write_chunk(&mut out, b"IHDR", &header);

        let stride = (width * 4) as usize;
        let mut raw = Vec::with_capacity((stride + 1) * height as usize);
        for y in 0..height as usize {
            raw.push(0); // satır filtresi: yok
            raw.extend_from_slice(&rgba[y * stride..(y + 1) * stride]);
        }
        write_chunk(&mut out, b"IDAT", &zlib_stored(&raw));
        write_chunk(&mut out, b"IEND", &[]);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_valid_png_structure() {
        let png = PngEncoder::encode(&[255, 0, 0, 128], 1, 1);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
        // IEND CRC'si sabittir.
        assert_eq!(&png[png.len() - 4..], &[0xAE, 0x42, 0x60, 0x82]);
    }

    #[test]
    fn zlib_stream_has_adler() {
        let z = zlib_stored(b"abc");
        assert_eq!(&z[z.len() - 4..], &0x024d_0127u32.to_be_bytes());
    }
}
