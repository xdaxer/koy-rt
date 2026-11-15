use super::tray_notify_data::{Guid, TrayBalloonFlags, TrayIconFields, TrayMessage, TrayNotifyData};

/// Shell_NotifyIcon'un `Shell_TrayWnd` penceresine WM_COPYDATA ile gönderdiği SHELLTRAYDATA bloğunu çözer.
/// shell32 çağıranın yapısını her zaman sabit, 32 bit tanıtıcılı tam NOTIFYICONDATA düzenine çevirir;
/// bu yüzden alanlar 32 ve 64 bit uygulamalar için aynı konumdadır.
pub struct TrayMessageParser;

// Konumlar SHELLTRAYDATA başına göredir (imza 4 + mesaj 4 = 8 bayt başlık).
const OFF_MESSAGE: usize = 4;
const OFF_HWND: usize = 12;
const OFF_ID: usize = 16;
const OFF_FLAGS: usize = 20;
const OFF_CALLBACK: usize = 24;
const OFF_ICON: usize = 28;
const OFF_TIP: usize = 32;
const TIP_CHARS: usize = 128;
const OFF_STATE: usize = 288;
const OFF_STATE_MASK: usize = 292;
const OFF_INFO: usize = 296;
const INFO_CHARS: usize = 256;
const OFF_VERSION: usize = 808;
const OFF_INFO_TITLE: usize = 812;
const INFO_TITLE_CHARS: usize = 64;
const OFF_INFO_FLAGS: usize = 940;
const OFF_GUID: usize = 944;
const OFF_BALLOON_ICON: usize = 960;

impl TrayMessageParser {
    /// WM_COPYDATA'daki dwData değeri: tepsi mesajı.
    pub const COPY_DATA_TRAY_MESSAGE: i32 = 1;

    /// SHELLTRAYDATA başındaki sabit imza.
    pub const SIGNATURE: i32 = 0x34753423;

    /// İpucu alanının sonuna kadar olan en küçük geçerli blok.
    pub const MINIMUM_LENGTH: i32 = (OFF_TIP + TIP_CHARS * 2) as i32;

    /// Kabul edilen en büyük blok. Gerçek SHELLTRAYDATA ~1 KB'tır; başka bir süreç Koy'un penceresine
    /// WM_COPYDATA ile keyfi boyutta veri gönderebilir, bu yüzden üst sınır konur.
    pub const MAXIMUM_LENGTH: i32 = 4096;

    /// WM_COPYDATA'daki `cbData` bu çözümleyiciye verilebilir mi? Platform işaretsiz 32 bit değeri
    /// `i32`'ye çevirip dilim kurmadan önce bunu sorar (devasa değer negatif `i32` olup çökertirdi).
    pub fn is_acceptable_length(byte_count: i64) -> bool {
        (Self::MINIMUM_LENGTH as i64..=Self::MAXIMUM_LENGTH as i64).contains(&byte_count)
    }

    /// `TryParse(span, out result)`: çözülemezse `None`.
    pub fn try_parse(data: &[u8]) -> Option<TrayNotifyData> {
        if !Self::is_acceptable_length(data.len() as i64) {
            return None;
        }

        if i32::from_le_bytes(read4(data, 0)) != Self::SIGNATURE {
            return None;
        }

        let message = read_u32(data, OFF_MESSAGE);
        if message > TrayMessage::SetVersion as u32 {
            return None;
        }

        let fields = TrayIconFields(read_u32(data, OFF_FLAGS));
        let has_info = fields.has(TrayIconFields::INFO);

        Some(TrayNotifyData {
            message: TrayMessage::from_u32(message)?,
            owner_hwnd: to_handle(read_u32(data, OFF_HWND)),
            id: read_u32(data, OFF_ID),
            fields,
            callback_message: read_u32(data, OFF_CALLBACK),
            icon_handle: to_handle(read_u32(data, OFF_ICON)),
            tooltip: read_string(&data[OFF_TIP..OFF_TIP + TIP_CHARS * 2]),
            state: read_u32_or_zero(data, OFF_STATE),
            state_mask: read_u32_or_zero(data, OFF_STATE_MASK),
            version: read_u32_or_zero(data, OFF_VERSION),
            guid: if data.len() >= OFF_GUID + 16 { Guid::from_bytes(&data[OFF_GUID..OFF_GUID + 16]) } else { Guid::EMPTY },
            info: if has_info { read_string_or_empty(data, OFF_INFO, INFO_CHARS * 2) } else { String::new() },
            info_title: if has_info { read_string_or_empty(data, OFF_INFO_TITLE, INFO_TITLE_CHARS * 2) } else { String::new() },
            info_flags: if has_info { TrayBalloonFlags(read_u32_or_zero(data, OFF_INFO_FLAGS)) } else { TrayBalloonFlags::NONE },
            balloon_icon_handle: if has_info { to_handle(read_u32_or_zero(data, OFF_BALLOON_ICON)) } else { 0 },
        })
    }
}

fn read4(data: &[u8], offset: usize) -> [u8; 4] {
    [data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(read4(data, offset))
}

// 64 bit Windows'ta tanıtıcılar 32 bite sığar ve işaretli genişletilir (LongToHandle).
fn to_handle(value: u32) -> isize {
    value as i32 as isize
}

fn read_u32_or_zero(data: &[u8], offset: usize) -> u32 {
    if data.len() >= offset + 4 { read_u32(data, offset) } else { 0 }
}

fn read_string_or_empty(data: &[u8], offset: usize, byte_count: usize) -> String {
    if data.len() >= offset + byte_count { read_string(&data[offset..offset + byte_count]) } else { String::new() }
}

/// UTF-16LE metin, ilk NUL'a (ya da alan sonuna) kadar. C# ile aynı: sonlandırıcı yoksa alan tam okunur.
/// Çiftsiz vekil (surrogate) yarısı C# dizgesinde kalır; Rust `String` taşıyamadığı için U+FFFD olur.
fn read_string(utf16: &[u8]) -> String {
    let units: Vec<u16> = utf16.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}
