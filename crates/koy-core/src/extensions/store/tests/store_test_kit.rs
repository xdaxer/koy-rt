//! Karşılığı: `Extensions/Store/StoreTestKit.cs`: mağaza testlerinin ortak yardımcıları (geçici klasör, bellekte zip,
//! örnek paket ve indeks).
//!
//! C# testleri zip'i `ZipArchive` ile kurar; burada küçük bir zip yazıcısı vardır ([`build_zip`]). Girdiler varsayılan
//! olarak "stored" (sıkıştırmasız) yazılır; [`ZipFile::deflated`] test için yazılmış en küçük DEFLATE yazıcısıyla
//! (sabit Huffman, harf + tekrar eşleşmesi) sıkıştırır, böylece kurulum yolu gerçek deflate verisiyle de sınanır.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use chrono::TimeZone;
use sha2::{Digest, Sha256};

use crate::extensions::store::*;
use crate::settings::{AppSettings, ProfileSection, SettingsProfileCodec};

pub(super) const PACKAGE_URL: &str = "https://raw.githubusercontent.com/koyhq/koy/main/store/packages/p.koyext";
pub(super) const PREVIEW_URL: &str = "https://raw.githubusercontent.com/koyhq/koy/main/store/previews/p.png";

pub(super) const THEME_JSON: &str = "{ \"KoyBgWindow\": \"#112233\" }";

pub(super) fn gif() -> Vec<u8> {
    let mut bytes = b"GIF89a".to_vec();
    bytes.extend_from_slice(&[1, 0, 1, 0, 0, 0, 0]);
    bytes
}

pub(super) fn png() -> Vec<u8> {
    vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0]
}

pub(super) fn profile_json() -> String {
    SettingsProfileCodec::export(&AppSettings::default(), "Ornek", &[ProfileSection::Appearance])
}

pub(super) fn manifest(id: &str, kind: &str, version: &str, extra: Option<&str>) -> String {
    format!(
        "{{ \"id\": \"{id}\", \"name\": \"Ad {id}\", \"version\": \"{version}\", \"author\": \"Yazar\", \"kind\": \"{kind}\", \"license\": \"MIT\", \"capabilities\": []{} }}",
        extra.unwrap_or("")
    )
}

// ---------------------------------------------------------------- zip yazıcısı

/// Zip'e yazılacak bir dosya.
pub(super) struct ZipFile {
    pub name: String,
    pub data: Vec<u8>,
    pub deflate: bool,
    /// Merkezi dizinde yazılacak açılmış boyut (yalan söyleyen başlık testleri); `None` = gerçek boyut.
    pub declared_length: Option<u32>,
}

impl ZipFile {
    pub fn new(name: &str, data: impl Into<Vec<u8>>) -> ZipFile {
        ZipFile { name: name.to_string(), data: data.into(), deflate: false, declared_length: None }
    }

    pub fn deflated(mut self) -> ZipFile {
        self.deflate = true;
        self
    }

    pub fn declared(mut self, length: u32) -> ZipFile {
        self.declared_length = Some(length);
        self
    }
}

/// `F(name, text)` / `F(name, bytes)`.
pub(super) fn f(name: &str, data: impl Into<Vec<u8>>) -> ZipFile {
    ZipFile::new(name, data)
}

fn crc32(data: &[u8]) -> u32 {
    // Büyük (zip bombası) verilerde CRC hesaplanmaz: okuyucu CRC doğrulamaz (C# `ZipArchive` de doğrulamaz).
    if data.len() > 1024 * 1024 {
        return 0;
    }

    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *slot = c;
    }

    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc = table[((crc ^ u32::from(b)) & 0xFF) as usize] ^ (crc >> 8);
    }
    !crc
}

/// `zip.Position = 0` ile geri sarılmış bellek akışı gibi: bayt dizisinden okunur/aranır.
pub(super) fn zip(files: Vec<ZipFile>) -> Cursor<Vec<u8>> {
    Cursor::new(build_zip(&files))
}

pub(super) fn build_zip(files: &[ZipFile]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();

    for file in files {
        let (method, stored): (u16, Vec<u8>) = if file.deflate { (8, deflate_fixed(&file.data)) } else { (0, file.data.clone()) };
        let crc = crc32(&file.data);
        let uncompressed = file.declared_length.unwrap_or(file.data.len() as u32);
        let offset = out.len() as u32;
        let name = file.name.as_bytes();

        // Yerel başlık.
        out.extend_from_slice(&0x0403_4B50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0x0800u16.to_le_bytes());
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // saat + tarih
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(stored.len() as u32).to_le_bytes());
        out.extend_from_slice(&(file.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(&stored);

        // Merkezi dizin girdisi.
        central.extend_from_slice(&0x0201_4B50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0x0800u16.to_le_bytes());
        central.extend_from_slice(&method.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(stored.len() as u32).to_le_bytes());
        central.extend_from_slice(&uncompressed.to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // ek alan
        central.extend_from_slice(&0u16.to_le_bytes()); // yorum
        central.extend_from_slice(&0u16.to_le_bytes()); // disk
        central.extend_from_slice(&0u16.to_le_bytes()); // iç öznitelik
        central.extend_from_slice(&0u32.to_le_bytes()); // dış öznitelik
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
    }

    let central_offset = out.len() as u32;
    let central_size = central.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4B50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&central_size.to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

// ---------------------------------------------------------------- en küçük DEFLATE yazıcısı

struct BitWriter {
    bytes: Vec<u8>,
    buffer: u64,
    count: u32,
}

impl BitWriter {
    fn put(&mut self, value: u32, bits: u32) {
        self.buffer |= u64::from(value) << self.count;
        self.count += bits;
        while self.count >= 8 {
            self.bytes.push(self.buffer as u8);
            self.buffer >>= 8;
            self.count -= 8;
        }
    }

    // Huffman kodları en üst bit önce.
    fn put_code(&mut self, code: u32, bits: u32) {
        for i in (0..bits).rev() {
            self.put((code >> i) & 1, 1);
        }
    }

    fn literal_length(&mut self, symbol: u32) {
        match symbol {
            0..=143 => self.put_code(0x30 + symbol, 8),
            144..=255 => self.put_code(0x190 + (symbol - 144), 9),
            256..=279 => self.put_code(symbol - 256, 7),
            _ => self.put_code(0xC0 + (symbol - 280), 8),
        }
    }
}

/// Tek sabit-Huffman bloğu: harfler, aynı baytın tekrarları (uzaklık 1, uzunluk ≤ 258) eşleşme olarak.
pub(super) fn deflate_fixed(data: &[u8]) -> Vec<u8> {
    const LENGTH_BASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
    const LENGTH_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];

    let mut w = BitWriter { bytes: Vec::new(), buffer: 0, count: 0 };
    w.put(1, 1);
    w.put(1, 2);
    let mut i = 0;
    while i < data.len() {
        let byte = data[i];
        let mut run = 1;
        while i + run < data.len() && data[i + run] == byte && run < 1 + 258 {
            run += 1;
        }

        w.literal_length(u32::from(byte));
        i += 1;
        let mut repeat = run - 1;
        while repeat >= 3 {
            let length = repeat.min(258);
            let li = LENGTH_BASE.iter().rposition(|&b| usize::from(b) <= length).unwrap_or(0);
            w.literal_length(257 + li as u32);
            w.put((length - usize::from(LENGTH_BASE[li])) as u32, u32::from(LENGTH_EXTRA[li]));
            w.put_code(0, 5); // uzaklık 1
            i += length;
            repeat -= length;
        }
    }
    w.literal_length(256);
    if w.count > 0 {
        w.bytes.push(w.buffer as u8);
    }
    w.bytes
}

// ---------------------------------------------------------------- kayıt ve öğe üreticileri

pub(super) fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

/// `Item(id, kind, version, ...)`: diğer alanlar C# varsayılanları (ad "Ornek", yazar "Koy", özet "Ozet", ...).
pub(super) fn item(id: &str, kind: ExtensionKind, version: &str) -> StoreItem {
    StoreItem {
        id: id.to_string(),
        name: "Ornek".to_string(),
        kind,
        version: version.to_string(),
        author: "Koy".to_string(),
        license: "MIT".to_string(),
        summary: LocalizedText::plain("Ozet"),
        description: LocalizedText::plain(""),
        homepage: None,
        updated: chrono::NaiveDate::from_ymd_opt(2026, 11, 12).unwrap(),
        downloads: 0,
        package: StorePackage { url: PACKAGE_URL.to_string(), sha256: "a".repeat(64), size: 1000 },
        preview: None,
    }
}

/// `Item()` varsayılanları: `koyhq.ornek`, Theme, 1.0.0.
pub(super) fn default_item() -> StoreItem {
    item("koyhq.ornek", ExtensionKind::Theme, "1.0.0")
}

/// `Installed(id, kind, version, applied, payload)`.
pub(super) fn installed(id: &str, kind: ExtensionKind, version: &str, applied: bool, payload: &str) -> InstalledEntry {
    InstalledEntry {
        id: id.to_string(),
        version: version.to_string(),
        kind,
        name: format!("Ad {id}"),
        author: "Yazar".to_string(),
        license: "MIT".to_string(),
        installed_at: chrono::FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap(),
        source: ExtensionSource::Store,
        applied,
        payload_file: payload.to_string(),
    }
}

pub(super) fn registry(entries: Vec<InstalledEntry>) -> InstalledRegistry {
    InstalledRegistry::try_build(entries).expect("kayıt kurulabilmeli")
}

/// Tek öğeli geçerli indeks JSON'u; `extra` öğenin ham metnine eklenir.
pub(super) fn item_json(id: &str, kind: &str, extra: &str) -> String {
    format!(
        "{{ \"id\": \"{id}\", \"name\": \"Ornek\", \"kind\": \"{kind}\", \"version\": \"1.0.0\", \"author\": \"Koy\", \"license\": \"GPL-3.0-or-later\", \"summary\": \"Ozet\", \"updated\": \"2026-11-12\", \"package\": {{ \"url\": \"{PACKAGE_URL}\", \"sha256\": \"{}\", \"size\": 1234 }}{extra} }}",
        "a".repeat(64)
    )
}

/// `ItemJson()` varsayılanları.
pub(super) fn default_item_json() -> String {
    item_json("koyhq.ornek", "theme", "")
}

pub(super) fn index_json(items: &[String]) -> String {
    format!(
        "{{ \"format\": \"koy-store-index\", \"formatVersion\": 1, \"generated\": \"2026-11-12T00:00:00Z\", \"items\": [{}] }}",
        items.join(",")
    )
}

// ---------------------------------------------------------------- geçici klasör ve depo kökü

pub(super) struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new() -> TempDir {
        let id: String = crate::random::get_bytes(16).iter().map(|b| format!("{b:02x}")).collect();
        let path = std::env::temp_dir().join(format!("koy-store-{id}"));
        std::fs::create_dir_all(&path).expect("geçici klasör");
        TempDir { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn combine(&self, part: &str) -> PathBuf {
        self.path.join(part)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// C# deposunun kökü (`store/index.json` içeren klasör). C# testi `Koy.slnx` arar; Rust çalışma alanı
/// `koy-rt/` yanında `koy/` klasörü bulunur. Bulunamazsa `None` (depoya bağlı testler atlanır).
pub(super) fn repo_root() -> Option<PathBuf> {
    let start = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for ancestor in start.ancestors() {
        for candidate in [ancestor.to_path_buf(), ancestor.join("koy")] {
            if candidate.join("store").join("index.json").is_file() {
                return Some(candidate);
            }
        }
    }

    None
}
