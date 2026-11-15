use crate::time::DateTimeOffset;

/// .NET `Guid` karşılığı: 16 bayt, .NET'in bellek düzeninde (ilk üç alan little-endian). `new Guid(ReadOnlySpan<byte>)`
/// ve `TryWriteBytes` ile aynı bayt sırası, yani SHELLTRAYDATA içindeki `guidItem` (Windows GUID yapısı) ile aynı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Guid(pub [u8; 16]);

impl Guid {
    /// `Guid.Empty`.
    pub const EMPTY: Guid = Guid([0; 16]);

    /// `new Guid(ReadOnlySpan<byte>)`; `bytes` en az 16 bayt olmalı.
    pub fn from_bytes(bytes: &[u8]) -> Guid {
        let mut g = [0u8; 16];
        g.copy_from_slice(&bytes[..16]);
        Guid(g)
    }

    /// `Guid.ToByteArray()` / `TryWriteBytes`.
    pub fn to_bytes(self) -> [u8; 16] {
        self.0
    }
}

/// Shell_NotifyIcon çağrısının türü (NIM_*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum TrayMessage {
    Add = 0,
    Modify = 1,
    Delete = 2,
    SetFocus = 3,
    SetVersion = 4,
}

impl TrayMessage {
    /// `(TrayMessage)value`; yalnız tanımlı değerler (C#'ta başka değerler de dönüşür ama ayrıştırıcı
    /// `SetVersion` üstünü zaten eler).
    pub fn from_u32(value: u32) -> Option<TrayMessage> {
        match value {
            0 => Some(TrayMessage::Add),
            1 => Some(TrayMessage::Modify),
            2 => Some(TrayMessage::Delete),
            3 => Some(TrayMessage::SetFocus),
            4 => Some(TrayMessage::SetVersion),
            _ => None,
        }
    }
}

/// Mesajda hangi alanların geçerli olduğu (NIF_*). `[Flags] enum : uint`: bilinmeyen bitler korunur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TrayIconFields(pub u32);

impl TrayIconFields {
    pub const NONE: TrayIconFields = TrayIconFields(0);
    pub const MESSAGE: TrayIconFields = TrayIconFields(0x01);
    pub const ICON: TrayIconFields = TrayIconFields(0x02);
    pub const TIP: TrayIconFields = TrayIconFields(0x04);
    pub const STATE: TrayIconFields = TrayIconFields(0x08);
    pub const INFO: TrayIconFields = TrayIconFields(0x10);
    pub const GUID: TrayIconFields = TrayIconFields(0x20);

    /// `(fields & flag) != 0`.
    pub const fn has(self, flag: TrayIconFields) -> bool {
        self.0 & flag.0 != 0
    }
}

impl std::ops::BitOr for TrayIconFields {
    type Output = TrayIconFields;
    fn bitor(self, rhs: TrayIconFields) -> TrayIconFields {
        TrayIconFields(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for TrayIconFields {
    type Output = TrayIconFields;
    fn bitand(self, rhs: TrayIconFields) -> TrayIconFields {
        TrayIconFields(self.0 & rhs.0)
    }
}

/// Balon bildiriminin simge ve davranış bayrakları (NIIF_*). `[Flags] enum : uint`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TrayBalloonFlags(pub u32);

impl TrayBalloonFlags {
    pub const NONE: TrayBalloonFlags = TrayBalloonFlags(0);
    pub const INFO: TrayBalloonFlags = TrayBalloonFlags(0x01);
    pub const WARNING: TrayBalloonFlags = TrayBalloonFlags(0x02);
    pub const ERROR: TrayBalloonFlags = TrayBalloonFlags(0x03);
    pub const USER: TrayBalloonFlags = TrayBalloonFlags(0x04);
    pub const ICON_MASK: TrayBalloonFlags = TrayBalloonFlags(0x0F);
    pub const NO_SOUND: TrayBalloonFlags = TrayBalloonFlags(0x10);
    pub const LARGE_ICON: TrayBalloonFlags = TrayBalloonFlags(0x20);
    pub const RESPECT_QUIET_TIME: TrayBalloonFlags = TrayBalloonFlags(0x80);

    /// `(flags & flag) != 0`.
    pub const fn has(self, flag: TrayBalloonFlags) -> bool {
        self.0 & flag.0 != 0
    }
}

impl std::ops::BitOr for TrayBalloonFlags {
    type Output = TrayBalloonFlags;
    fn bitor(self, rhs: TrayBalloonFlags) -> TrayBalloonFlags {
        TrayBalloonFlags(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for TrayBalloonFlags {
    type Output = TrayBalloonFlags;
    fn bitand(self, rhs: TrayBalloonFlags) -> TrayBalloonFlags {
        TrayBalloonFlags(self.0 & rhs.0)
    }
}

/// Bir uygulamanın tepsiye gönderdiği tek mesaj. Pencere ve simge tanıtıcıları `isize`;
/// Win32 tipi taşımaz (AGENTS.md #11). `version` alanı `TrayIconFields::INFO`
/// taşındığında Win32 `uTimeout` birleşimini tutar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayNotifyData {
    pub message: TrayMessage,
    pub owner_hwnd: isize,
    pub id: u32,
    pub fields: TrayIconFields,
    pub callback_message: u32,
    pub icon_handle: isize,
    pub tooltip: String,
    pub state: u32,
    pub state_mask: u32,
    pub version: u32,
    pub guid: Guid,
    pub info: String,
    pub info_title: String,
    pub info_flags: TrayBalloonFlags,
    pub balloon_icon_handle: isize,
}

impl TrayNotifyData {
    /// C# kaydının zorunlu alanları; `Info`, `InfoTitle` (`""`), `InfoFlags` (`None`), `BalloonIconHandle` (0)
    /// varsayılanlarıyla. Gerisi için `TrayNotifyData { info: .., ..TrayNotifyData::new(..) }`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        message: TrayMessage,
        owner_hwnd: isize,
        id: u32,
        fields: TrayIconFields,
        callback_message: u32,
        icon_handle: isize,
        tooltip: impl Into<String>,
        state: u32,
        state_mask: u32,
        version: u32,
        guid: Guid,
    ) -> Self {
        TrayNotifyData {
            message,
            owner_hwnd,
            id,
            fields,
            callback_message,
            icon_handle,
            tooltip: tooltip.into(),
            state,
            state_mask,
            version,
            guid,
            info: String::new(),
            info_title: String::new(),
            info_flags: TrayBalloonFlags::NONE,
            balloon_icon_handle: 0,
        }
    }
}

/// Tepsi simgesinin kimliği. GUID verilmişse simge yalnız GUID ile, verilmemişse pencere + kimlik numarasıyla tanınır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TrayIconKey {
    pub owner_hwnd: isize,
    pub id: u32,
    pub guid: Guid,
}

impl TrayIconKey {
    pub const fn new(owner_hwnd: isize, id: u32, guid: Guid) -> Self {
        TrayIconKey { owner_hwnd, id, guid }
    }

    /// `TrayIconKey.From(data)`.
    pub fn from_data(data: &TrayNotifyData) -> TrayIconKey {
        if data.fields.has(TrayIconFields::GUID) && data.guid != Guid::EMPTY {
            TrayIconKey::new(0, 0, data.guid)
        } else {
            TrayIconKey::new(data.owner_hwnd, data.id, Guid::EMPTY)
        }
    }
}

/// Tepsi simgesinden gelen geçerli bir balon bildirimi.
#[derive(Debug, Clone, PartialEq)]
pub struct TrayBalloonNotification {
    pub key: TrayIconKey,
    pub app_name: String,
    pub title: String,
    pub body: String,
    pub flags: TrayBalloonFlags,
    pub time: DateTimeOffset,
    pub icon: Option<Vec<u8>>,
}

/// Dock'un tepsi penceresinde gösterilen simge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayIcon {
    pub key: TrayIconKey,
    pub owner_hwnd: isize,
    pub id: u32,
    pub callback_message: u32,
    pub icon_handle: isize,
    pub tooltip: String,
    pub is_hidden: bool,
    pub version: u32,
}

impl TrayIcon {
    /// Gizli işaretli ya da simgesi olmayan kayıt gösterilmez (boş kare çizilmesin).
    pub fn is_visible(&self) -> bool {
        !self.is_hidden && self.icon_handle != 0
    }
}
