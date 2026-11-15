//! Karşılığı: `tests/Core.Tests/JumpList/CustomDestinationsBuilder.cs`.
//!
//! Testler için tamamen sentetik `customDestinations-ms` üretir. Gerçek kullanıcı dosyası hiçbir testte kullanılmaz.
//! ShellLink akışı yalnız yapıyı taşır (başlık, isteğe bağlı kimlik listesi / bağlantı bilgisi, dizeler, uç blok);
//! içeriği anlamlı bir kısayol değildir.

pub const SHELL_LINK_CLSID: [u8; 16] =
    [0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46];

pub const FOOTER: u32 = 0xBABFFBAB;

#[derive(Default)]
pub struct CustomDestinationsBuilder {
    body: Vec<u8>,
    categories: i32,
}

impl CustomDestinationsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn custom(mut self, name: &str, items: &[Vec<u8>]) -> Self {
        self.write_int(0);
        let units: Vec<u16> = name.encode_utf16().collect();
        self.write_short(units.len() as i16);
        for u in &units {
            self.body.extend_from_slice(&u.to_le_bytes());
        }
        self.write_items(items);
        self
    }

    pub fn known(mut self, id: i32) -> Self {
        self.write_int(1);
        self.write_int(id);
        self.write_uint(FOOTER);
        self.categories += 1;
        self
    }

    pub fn tasks(mut self, items: &[Vec<u8>]) -> Self {
        self.write_int(2);
        self.write_items(items);
        self
    }

    /// Kategori ekleyen, sahipliği almayan sürüm (döngülerde kullanılır).
    pub fn add_custom(&mut self, name: &str, items: &[Vec<u8>]) {
        let taken = std::mem::take(self);
        *self = taken.custom(name, items);
    }

    fn write_items(&mut self, items: &[Vec<u8>]) {
        self.write_int(items.len() as i32);
        for item in items {
            self.body.extend_from_slice(&SHELL_LINK_CLSID);
            self.body.extend_from_slice(item);
        }

        self.write_uint(FOOTER);
        self.categories += 1;
    }

    pub fn build(&self) -> Vec<u8> {
        self.build_with(2, None)
    }

    pub fn build_with(&self, version: i32, category_count: Option<i32>) -> Vec<u8> {
        let mut ms = Vec::new();
        ms.extend_from_slice(&version.to_le_bytes());
        ms.extend_from_slice(&category_count.unwrap_or(self.categories).to_le_bytes());
        ms.extend_from_slice(&0i32.to_le_bytes());
        ms.extend_from_slice(&self.body);
        ms
    }

    fn write_int(&mut self, value: i32) {
        self.body.extend_from_slice(&value.to_le_bytes());
    }

    fn write_short(&mut self, value: i16) {
        self.body.extend_from_slice(&value.to_le_bytes());
    }

    fn write_uint(&mut self, value: u32) {
        self.body.extend_from_slice(&value.to_le_bytes());
    }
}

/// `Link(...)` parametreleri. `string_flags`: 0x04 ad, 0x08 göreli yol, 0x10 klasör, 0x20 bağımsız değişken, 0x40 simge.
pub struct LinkSpec<'a> {
    pub strings: Option<Vec<&'a str>>,
    pub string_flags: u32,
    pub unicode: bool,
    pub id_list_bytes: i32,
    pub link_info_bytes: i32,
    pub extra_blocks: i32,
}

impl Default for LinkSpec<'_> {
    fn default() -> Self {
        LinkSpec { strings: None, string_flags: 0x20, unicode: true, id_list_bytes: -1, link_info_bytes: -1, extra_blocks: 0 }
    }
}

/// Varsayılan bağlantı: `Link()`.
pub fn link() -> Vec<u8> {
    link_spec(LinkSpec::default())
}

/// `Link(["a", ...])`: yalnız dizeler.
pub fn link_strings(strings: &[&str]) -> Vec<u8> {
    link_spec(LinkSpec { strings: Some(strings.to_vec()), ..Default::default() })
}

/// MS-SHLLINK biçiminde sentetik akış.
pub fn link_spec(spec: LinkSpec) -> Vec<u8> {
    let mut ms: Vec<u8> = Vec::new();
    let mut flags = spec.string_flags | if spec.unicode { 0x80 } else { 0 };
    if spec.id_list_bytes >= 0 {
        flags |= 0x01;
    }
    if spec.link_info_bytes >= 0 {
        flags |= 0x02;
    }

    let mut header = [0u8; 0x4C];
    header[0..4].copy_from_slice(&0x4Cu32.to_le_bytes());
    header[4..20].copy_from_slice(&SHELL_LINK_CLSID);
    header[0x14..0x18].copy_from_slice(&flags.to_le_bytes());
    ms.extend_from_slice(&header);

    if spec.id_list_bytes >= 0 {
        ms.extend_from_slice(&(spec.id_list_bytes as u16).to_le_bytes());
        ms.extend(std::iter::repeat_n(0u8, spec.id_list_bytes as usize));
    }

    if spec.link_info_bytes >= 0 {
        let size = spec.link_info_bytes + 4;
        ms.extend_from_slice(&(size as u32).to_le_bytes());
        ms.extend(std::iter::repeat_n(0u8, spec.link_info_bytes as usize));
    }

    let values: Vec<&str> = spec.strings.unwrap_or_else(|| vec!["--yeni-pencere"]);
    let mut index = 0;
    for bit in [0x04u32, 0x08, 0x10, 0x20, 0x40] {
        if spec.string_flags & bit == 0 {
            continue;
        }
        let text = if index < values.len() { values[index] } else { "" };
        index += 1;
        let units: Vec<u16> = text.encode_utf16().collect();
        ms.extend_from_slice(&(units.len() as u16).to_le_bytes());
        if spec.unicode {
            for u in &units {
                ms.extend_from_slice(&u.to_le_bytes());
            }
        } else {
            // Encoding.ASCII: ASCII dışı karakterler '?' olur.
            ms.extend(text.chars().map(|c| if c.is_ascii() { c as u8 } else { b'?' }));
        }
    }

    for _ in 0..spec.extra_blocks {
        ms.extend_from_slice(&12u32.to_le_bytes()); // boyut, imza, 4 bayt veri
        ms.extend_from_slice(&0xA0000001u32.to_le_bytes());
        ms.extend_from_slice(&[0u8; 4]);
    }

    ms.extend_from_slice(&0u32.to_le_bytes()); // uç blok
    ms
}
