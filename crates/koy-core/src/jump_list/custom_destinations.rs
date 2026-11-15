//! Karşılığı: `JumpList/CustomDestinations.cs`.

use super::jump_list_limits::JumpListLimits;

/// `customDestinations-ms` içindeki kategori türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CustomDestinationsCategoryKind {
    /// Uygulamanın adlandırdığı kategori (tür 0).
    Custom,
    /// Bilinen kategori: Windows'un son / sık kullanılanları (tür 1); öğesi yoktur.
    Known,
    /// Görevler (tür 2).
    Tasks,
}

/// Bilinen kategori kimliği (dosyadaki değer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JumpListKnownCategory {
    Frequent = 1,
    Recent = 2,
}

/// Dosyadaki bir kategori. `items` her öğenin ShellLink (MS-SHLLINK) akışıdır; baştaki 16 baytlık CLSID
/// atılmıştır, `IPersistStream.Load` doğrudan bu baytlarla çağrılır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomDestinationsCategory {
    pub kind: CustomDestinationsCategoryKind,
    pub name: Option<String>,
    pub known: Option<JumpListKnownCategory>,
    pub items: Vec<Vec<u8>>,
}

/// Okunmuş dosya. Bozuk ya da bilinmeyen biçim `empty()` olur (kategori yok).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomDestinationsFile {
    pub categories: Vec<CustomDestinationsCategory>,
    /// Dosya yapısı geçerli okundu. Geçerli ama gösterilecek kategorisi olmayan dosya (yalnız tanımadığımız bilinen
    /// kategori) mümkündür: uygulamanın bir listesi vardır ve son / sık kullanılanları istememiş sayılır.
    pub is_valid: bool,
}

impl CustomDestinationsFile {
    /// `CustomDestinationsFile.Empty`: okunamayan (yok, bozuk, bilinmeyen biçim) dosya.
    pub fn empty() -> Self {
        CustomDestinationsFile { categories: Vec::new(), is_valid: false }
    }

    /// `new CustomDestinationsFile(categories, isValid: true)`.
    pub fn new(categories: Vec<CustomDestinationsCategory>) -> Self {
        CustomDestinationsFile { categories, is_valid: true }
    }

    /// Okunmuş gösterilebilir kategori yok.
    pub fn is_empty(&self) -> bool {
        self.categories.is_empty()
    }

    pub fn declares(&self, known: JumpListKnownCategory) -> bool {
        self.categories.iter().any(|c| c.kind == CustomDestinationsCategoryKind::Known && c.known == Some(known))
    }
}

const HEADER_BYTES: usize = 12;
const CATEGORY_FOOTER: u32 = 0xBABFFBAB;
const CLSID_BYTES: usize = 16;
const LINK_HEADER_BYTES: usize = 0x4C;

// CLSID_ShellLink {00021401-0000-0000-C000-000000000046}, bellekteki (GUID) bayt sırası.
pub(crate) const SHELL_LINK_CLSID: [u8; 16] =
    [0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46];

/// `customDestinations-ms` okuyucusu (sürüm 2). Dosya güvenilmeyen ikili veridir: boyut, kategori / öğe sayısı ve
/// dize uzunluğu için üst sınır vardır (`JumpListLimits`); bozuk, kısaltılmış, bilinmeyen sürümlü ya da
/// sınırı aşan girdi panik atmadan `CustomDestinationsFile::empty()` döner, kısmen okunmuş veri
/// **döndürülmez** ve hiçbir öğe uydurulmaz.
///
/// Biçim: üç int32 (sürüm = 2, kategori sayısı, ayrılmış). Her kategori: tür (int32). Tür 0: ad uzunluğu (int16),
/// UTF-16 ad, öğe sayısı (int32), öğeler. Tür 1: bilinen kategori kimliği (int32), öğe yok. Tür 2: öğe sayısı (int32),
/// öğeler. Her kategori 0xBABFFBAB ile biter. Öğe: ShellLink CLSID'i (16 bayt) + ShellLink akışı; akışın uzunluğu yoktur,
/// bu yüzden MS-SHLLINK yapısı (başlık, hedef listesi, bağlantı bilgisi, dizeler, ek bloklar) gezilerek ölçülür.
pub struct CustomDestinationsReader;

impl CustomDestinationsReader {
    pub fn parse(data: &[u8]) -> CustomDestinationsFile {
        // C#'taki try/catch (ArgumentException, IndexOutOfRange, Overflow) burada gereksiz: dilimler yalnız
        // açık sınır denetimlerinden sonra alınır.
        Self::parse_core(data).unwrap_or_else(CustomDestinationsFile::empty)
    }

    fn parse_core(data: &[u8]) -> Option<CustomDestinationsFile> {
        if data.len() < HEADER_BYTES || data.len() > JumpListLimits::MAX_FILE_BYTES as usize {
            return None;
        }

        let mut pos = 0usize;
        let version = try_read_i32(data, &mut pos)?;
        if version != 2 {
            return None;
        }
        let category_count = try_read_i32(data, &mut pos)?;
        if category_count < 1 || category_count > JumpListLimits::MAX_CATEGORIES {
            return None;
        }
        pos += 4; // ayrılmış alan

        let mut categories: Vec<CustomDestinationsCategory> = Vec::with_capacity(category_count as usize);
        let mut total_items: i32 = 0;

        for _ in 0..category_count {
            let type_id = try_read_i32(data, &mut pos)?;

            let mut name: Option<String> = None;
            let mut known: Option<JumpListKnownCategory> = None;
            let kind;
            let mut item_count: i32 = 0;

            match type_id {
                0 => {
                    kind = CustomDestinationsCategoryKind::Custom;
                    let name_chars = try_read_i16(data, &mut pos)?;
                    if name_chars < 0 || name_chars > JumpListLimits::MAX_CATEGORY_NAME_CHARS {
                        return None;
                    }
                    let name_bytes = name_chars as usize * 2;
                    if data.len() - pos < name_bytes {
                        return None;
                    }
                    // Encoding.Unicode.GetString: geçersiz vekil çiftleri U+FFFD olur.
                    let units: Vec<u16> =
                        data[pos..pos + name_bytes].chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
                    name = Some(String::from_utf16_lossy(&units));
                    pos += name_bytes;
                    item_count = try_read_i32(data, &mut pos)?;
                }
                1 => {
                    kind = CustomDestinationsCategoryKind::Known;
                    let known_id = try_read_i32(data, &mut pos)?;
                    // Tanımadığımız kimlikler (ör. -1) atlanır; yapı yine de geçerlidir.
                    known = match known_id {
                        1 => Some(JumpListKnownCategory::Frequent),
                        2 => Some(JumpListKnownCategory::Recent),
                        _ => None,
                    };
                }
                2 => {
                    kind = CustomDestinationsCategoryKind::Tasks;
                    item_count = try_read_i32(data, &mut pos)?;
                }
                _ => return None, // bilinmeyen kategori türü: yapıyı sürdüremeyiz
            }

            if item_count < 0 || item_count > JumpListLimits::MAX_ITEMS_PER_CATEGORY {
                return None;
            }
            total_items += item_count;
            if total_items > JumpListLimits::MAX_TOTAL_ITEMS {
                return None;
            }

            let mut items: Vec<Vec<u8>> = Vec::with_capacity(item_count as usize);
            for _ in 0..item_count {
                if data.len() - pos < CLSID_BYTES || data[pos..pos + CLSID_BYTES] != SHELL_LINK_CLSID {
                    return None; // ShellLink olmayan öğeyi çözemeyiz
                }
                pos += CLSID_BYTES;

                let length = try_measure_shell_link(data, pos)?;
                if length > JumpListLimits::MAX_ITEM_BYTES as usize {
                    return None;
                }

                items.push(data[pos..pos + length].to_vec());
                pos += length;
            }

            let footer = try_read_u32(data, &mut pos)?;
            if footer != CATEGORY_FOOTER {
                return None;
            }

            // Tanınmayan bilinen kategori (known == None) listeye eklenmez: gösterecek bir şey yok.
            if kind == CustomDestinationsCategoryKind::Known && known.is_none() {
                continue;
            }

            categories.push(CustomDestinationsCategory { kind, name, known, items });
        }

        Some(CustomDestinationsFile::new(categories))
    }
}

/// MS-SHLLINK akışının bayt uzunluğunu ölçer: 0x4C baytlık başlık, (isteğe bağlı) hedef kimlik listesi, bağlantı bilgisi,
/// sayılı dizeler ve boyutu 4'ten küçük bir bloğa kadar ek bloklar. Her adımda sınır denetimi yapılır.
fn try_measure_shell_link(data: &[u8], start: usize) -> Option<usize> {
    let mut pos = start;

    if data.len() - pos < LINK_HEADER_BYTES {
        return None;
    }
    if u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) != LINK_HEADER_BYTES as u32 {
        return None;
    }
    if data[pos + 4..pos + 4 + CLSID_BYTES] != SHELL_LINK_CLSID {
        return None;
    }

    let flags = u32::from_le_bytes(data[pos + 0x14..pos + 0x18].try_into().unwrap());
    pos += LINK_HEADER_BYTES;

    if flags & 0x01 != 0 {
        // HasLinkTargetIDList: uint16 boyut + veri
        let id_list_bytes = try_read_u16(data, &mut pos)? as usize;
        if data.len() - pos < id_list_bytes {
            return None;
        }
        pos += id_list_bytes;
    }

    if flags & 0x02 != 0 {
        // HasLinkInfo: uint32 boyut (kendisi dahil)
        let info_bytes = try_read_u32(data, &mut pos)?;
        if info_bytes < 4 {
            return None;
        }
        let skip = info_bytes as i64 - 4;
        if ((data.len() - pos) as i64) < skip {
            return None;
        }
        pos += skip as usize;
    }

    let unicode = flags & 0x80 != 0;
    // Ad, göreli yol, çalışma klasörü, bağımsız değişkenler, simge konumu: her biri uint16 karakter sayısı + karakterler.
    for bit in [0x04u32, 0x08, 0x10, 0x20, 0x40] {
        if flags & bit == 0 {
            continue;
        }
        let chars = try_read_u16(data, &mut pos)? as usize;
        let bytes = chars * if unicode { 2 } else { 1 };
        if data.len() - pos < bytes {
            return None;
        }
        pos += bytes;
    }

    // Ek bloklar: uint32 boyut; boyutu < 4 olan uç blok akışı bitirir (o 4 bayt akışa dahildir).
    loop {
        let block_size = try_read_u32(data, &mut pos)?;
        if block_size < 4 {
            break;
        }
        let skip = block_size as i64 - 4;
        if ((data.len() - pos) as i64) < skip {
            return None;
        }
        pos += skip as usize;
    }

    Some(pos - start)
}

fn try_read_i32(data: &[u8], pos: &mut usize) -> Option<i32> {
    if data.len() - *pos < 4 {
        return None;
    }
    let value = i32::from_le_bytes(data[*pos..*pos + 4].try_into().unwrap());
    *pos += 4;
    Some(value)
}

fn try_read_u32(data: &[u8], pos: &mut usize) -> Option<u32> {
    if data.len() - *pos < 4 {
        return None;
    }
    let value = u32::from_le_bytes(data[*pos..*pos + 4].try_into().unwrap());
    *pos += 4;
    Some(value)
}

/// İşaretli int16 (kategori adı uzunluğu); çağıran negatif değeri reddeder. C#'ta `int` olarak döner.
fn try_read_i16(data: &[u8], pos: &mut usize) -> Option<i32> {
    if data.len() - *pos < 2 {
        return None;
    }
    let value = i16::from_le_bytes(data[*pos..*pos + 2].try_into().unwrap());
    *pos += 2;
    Some(value as i32)
}

fn try_read_u16(data: &[u8], pos: &mut usize) -> Option<u16> {
    if data.len() - *pos < 2 {
        return None;
    }
    let value = u16::from_le_bytes(data[*pos..*pos + 2].try_into().unwrap());
    *pos += 2;
    Some(value)
}
