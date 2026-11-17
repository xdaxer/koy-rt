//! Ayar modelinin JSON ve "yansıma" altyapısı.
//!
//! C# tarafı üç şeyi yansıma ve kaynak üreticili `System.Text.Json` ile yapıyordu: (1) tipli JSON okuma/yazma,
//! (2) hata yolundan (`JsonException.Path`) alan onarımı, (3) `AdvancedSettings` / `SettingsChanges` içindeki alan
//! listeleme. Rust'ta bunların hepsi bu dosyadaki tek özellikten ([`SettingValue`]) ve `section!` makrosundan gelir:
//! her ayar bölümü alanlarını (C# özellik adı = JSON adı, PascalCase) bir kez tanımlar.
//!
//! Okuma kuralları C# ile aynıdır:
//! - dosyada olmayan alan varsayılanında kalır, bilinmeyen anahtar yok sayılır, anahtar adı büyük/küçük harf duyarlıdır;
//! - `null`: bölüm, liste, sözlük ve metin alanında alan varsayılanında kalır (C#'ta null atanır, `FillMissingSections`
//!   varsayılanla doldurur); `bool`/sayı/enum alanında hatadır;
//! - enum adı büyük/küçük harf duyarsız okunur (`JsonStringEnumConverter`); sayı olarak yazılmış enum Rust'ta
//!   temsil edilemeyeceği için hatadır (alan varsayılana döner).

use serde_json::{Map, Number, Value};

use super::advanced_settings::AdvancedSettingKind;
use super::json_tree::{Fail, NEGATIVE_INFINITY_TOKEN, POSITIVE_INFINITY_TOKEN, Seg, format_double};
use super::value_collections::{ValueDictionary, ValueList};
use crate::text;

/// Bir ayar alanının (bölüm kaydı, sayı, metin, liste...) JSON ve listeleme davranışı.
pub trait SettingValue: Sized + Clone + PartialEq {
    /// `AdvancedSettings` türü; liste, sözlük ve iç içe kayıt için `None`.
    const KIND: Option<AdvancedSettingKind> = None;

    /// JSON `null` alanı varsayılanında bırakır mı (C# referans tipi).
    const NULL_KEEPS_DEFAULT: bool = false;

    /// `ValueList<string>` alanı mı (profil doğrulama testleri alanları bununla ayıklar).
    const IS_STRING_LIST: bool = false;

    fn decode(node: &Value) -> Result<Self, Fail>;

    fn encode(&self) -> Value;

    /// `Enum.GetNames`.
    fn choices() -> Vec<String> {
        Vec::new()
    }

    /// `AdvancedSettings.Format`: değişmez kültürde metin.
    fn format(&self) -> String {
        String::new()
    }

    /// Eşitlik testleri için: değeri farklı bir değere çevirir (yalnız skalerler).
    fn different(&self) -> Option<Self> {
        None
    }
}

/// Liste ve sözlük öğesi: null ya da bozuk öğenin ne olacağını öğe tipi bilir.
pub trait ListItem: Sized + Clone + PartialEq {
    /// Öğe metin mi (`ValueList<string>` ayrımı için).
    const IS_TEXT: bool = false;

    /// `Ok(None)`: öğe sessizce atlanır (yalnız `DockItemEntry` null girdisi); hata: öğe (ya da iç alan) okunamadı.
    fn decode_item(node: &Value) -> Result<Option<Self>, Fail>;
    fn encode_item(&self) -> Value;
}

/// Alan listesi görünümü (`AdvancedSettings.List` ve `SettingsChanges` için).
#[derive(Debug, Clone)]
pub struct FieldView {
    pub name: &'static str,
    pub kind: Option<AdvancedSettingKind>,
    pub choices: Vec<String>,
    pub value: String,
    pub default: String,
    /// Alan varsayılan kayıttaki alana eşit mi (kayıt eşitliği).
    pub same: bool,
    /// `ValueList<string>` alanı mı.
    pub is_string_list: bool,
}

/// Ayar bölümü kaydı: C# `GetProperties()` yansımasının karşılığı.
pub trait SectionModel: Sized {
    /// Bildirim sırasıyla alanlar.
    fn describe(&self, defaults: &Self) -> Vec<FieldView>;

    /// Verilen JSON adlı skaler alanı farklı bir değere çevirilmiş kopya (yoksa/skaler değilse `None`).
    fn perturb(&self, field: &str) -> Option<Self>;
}

// ---------------------------------------------------------------- skalerler

impl SettingValue for bool {
    const KIND: Option<AdvancedSettingKind> = Some(AdvancedSettingKind::Bool);

    fn decode(node: &Value) -> Result<Self, Fail> {
        node.as_bool().ok_or_else(Fail::here)
    }

    fn encode(&self) -> Value {
        Value::Bool(*self)
    }

    fn format(&self) -> String {
        if *self { "true" } else { "false" }.to_string()
    }

    fn different(&self) -> Option<Self> {
        Some(!*self)
    }
}

impl SettingValue for i32 {
    const KIND: Option<AdvancedSettingKind> = Some(AdvancedSettingKind::Integer);

    fn decode(node: &Value) -> Result<Self, Fail> {
        node.as_i64().and_then(|n| i32::try_from(n).ok()).ok_or_else(Fail::here)
    }

    fn encode(&self) -> Value {
        Value::Number(Number::from(*self))
    }

    fn format(&self) -> String {
        self.to_string()
    }

    fn different(&self) -> Option<Self> {
        Some(self.wrapping_add(1))
    }
}

impl SettingValue for i64 {
    const KIND: Option<AdvancedSettingKind> = Some(AdvancedSettingKind::Integer);

    fn decode(node: &Value) -> Result<Self, Fail> {
        node.as_i64().ok_or_else(Fail::here)
    }

    fn encode(&self) -> Value {
        Value::Number(Number::from(*self))
    }

    fn format(&self) -> String {
        self.to_string()
    }

    fn different(&self) -> Option<Self> {
        Some(self.wrapping_add(1))
    }
}

impl SettingValue for f64 {
    const KIND: Option<AdvancedSettingKind> = Some(AdvancedSettingKind::Number);

    fn decode(node: &Value) -> Result<Self, Fail> {
        match node {
            Value::Number(n) => n.as_f64().ok_or_else(Fail::here),
            // 1e400 System.Text.Json'da hata vermeden +Infinity okunur (bkz. json_tree).
            Value::String(s) if s == POSITIVE_INFINITY_TOKEN => Ok(f64::INFINITY),
            Value::String(s) if s == NEGATIVE_INFINITY_TOKEN => Ok(f64::NEG_INFINITY),
            _ => Err(Fail::here()),
        }
    }

    fn encode(&self) -> Value {
        Number::from_f64(*self).map(Value::Number).unwrap_or(Value::Null)
    }

    fn format(&self) -> String {
        format_double(*self)
    }

    fn different(&self) -> Option<Self> {
        Some(*self + 1.0)
    }
}

impl SettingValue for String {
    const KIND: Option<AdvancedSettingKind> = Some(AdvancedSettingKind::Text);
    const NULL_KEEPS_DEFAULT: bool = true;

    fn decode(node: &Value) -> Result<Self, Fail> {
        match node {
            Value::String(s) if s != POSITIVE_INFINITY_TOKEN && s != NEGATIVE_INFINITY_TOKEN => Ok(s.clone()),
            _ => Err(Fail::here()),
        }
    }

    fn encode(&self) -> Value {
        Value::String(self.clone())
    }

    fn format(&self) -> String {
        self.clone()
    }

    fn different(&self) -> Option<Self> {
        Some(format!("{self}x"))
    }
}

/// `string?` alanı (ör. `AccentColor`). Çalışma zamanında `string` olduğundan `AdvancedSettings`te metin türüdür.
impl SettingValue for Option<String> {
    const KIND: Option<AdvancedSettingKind> = Some(AdvancedSettingKind::Text);

    fn decode(node: &Value) -> Result<Self, Fail> {
        match node {
            Value::Null => Ok(None),
            other => String::decode(other).map(Some),
        }
    }

    fn encode(&self) -> Value {
        match self {
            Some(s) => Value::String(s.clone()),
            None => Value::Null,
        }
    }

    fn format(&self) -> String {
        self.clone().unwrap_or_default()
    }

    fn different(&self) -> Option<Self> {
        Some(Some(format!("{}x", self.as_deref().unwrap_or(""))))
    }
}

// ---------------------------------------------------------------- enum

/// Enum adı tablosu (C# `Enum.GetNames` + `JsonStringEnumConverter`). Sıra bildirim sırasıdır.
pub trait NamedEnum: Copy + PartialEq + 'static {
    const NAMES: &'static [&'static str];
    fn name(&self) -> &'static str;
    fn from_name(name: &str) -> Option<Self>;
}

/// `JsonStringEnumConverter` okuması: ad büyük/küçük harf duyarsız eşleşir, baştaki/sondaki boşluk yok sayılır.
pub fn enum_from_text<E: NamedEnum>(text: &str) -> Option<E> {
    let trimmed = text.trim();
    E::NAMES
        .iter()
        .find(|n| text::eq_ignore_case(n, trimmed))
        .and_then(|n| E::from_name(n))
}

fn enum_decode<E: NamedEnum>(node: &Value) -> Result<E, Fail> {
    match node {
        Value::String(s) => enum_from_text::<E>(s).ok_or_else(Fail::here),
        _ => Err(Fail::here()),
    }
}

fn enum_different<E: NamedEnum>(value: &E) -> Option<E> {
    let index = E::NAMES.iter().position(|n| *n == value.name())?;
    E::from_name(E::NAMES[(index + 1) % E::NAMES.len()])
}

/// Enum için `NamedEnum` + `SettingValue` + `ListItem` uygular. Değer adları C# enum bildirimiyle aynı ve sıralıdır;
/// Rust enum'unda eksik/fazla değer olursa `match` derlenmez.
macro_rules! named_enums {
    ($($ty:ty => [$($v:ident),+ $(,)?]),+ $(,)?) => { $(
        impl NamedEnum for $ty {
            const NAMES: &'static [&'static str] = &[$(stringify!($v)),+];

            fn name(&self) -> &'static str {
                match self { $(<$ty>::$v => stringify!($v)),+ }
            }

            fn from_name(name: &str) -> Option<Self> {
                $( if name == stringify!($v) { return Some(<$ty>::$v); } )+
                None
            }
        }

        impl SettingValue for $ty {
            const KIND: Option<AdvancedSettingKind> = Some(AdvancedSettingKind::Choice);

            fn decode(node: &Value) -> Result<Self, Fail> {
                enum_decode::<$ty>(node)
            }

            fn encode(&self) -> Value {
                Value::String(NamedEnum::name(self).to_string())
            }

            fn choices() -> Vec<String> {
                <$ty as NamedEnum>::NAMES.iter().map(|n| n.to_string()).collect()
            }

            fn format(&self) -> String {
                NamedEnum::name(self).to_string()
            }

            fn different(&self) -> Option<Self> {
                enum_different(self)
            }
        }

        impl ListItem for $ty {
            fn decode_item(node: &Value) -> Result<Option<Self>, Fail> {
                enum_decode::<$ty>(node).map(Some)
            }

            fn encode_item(&self) -> Value {
                Value::String(NamedEnum::name(self).to_string())
            }
        }
    )+ };
}

named_enums! {
    super::UpdateChannel => [Stable, Preview],
    crate::appearance::AppTheme => [System, Dark, Light],
    crate::appearance::IconPack => [Builtin, Folder],
    crate::appearance::AnimationSpeed => [Calm, Normal, Fast, Off],
    crate::appearance::MaterialLevel => [Solid, Frosted, Glass],
    crate::units::TemperatureUnit => [Celsius, Fahrenheit],
    crate::units::WindSpeedUnit => [KilometersPerHour, MetersPerSecond, MilesPerHour, Knots],
    crate::units::NetworkSpeedUnit => [Bytes, Bits],
    crate::dock::DockEdge => [Bottom, Top, Left, Right],
    crate::visibility::VisibilityMode => [Fixed, Smart, Peek],
    crate::dock::DockStyleMode => [FloatingCapsule, FullWidth],
    crate::dock::DockIconAlignment => [Left, Center, Right],
    crate::dock::DockProximityMode => [DockDown, ReserveSpace, Dodge, AlwaysHidden, AlwaysFloating],
    crate::dock::DockStartIcon => [Koy, Windows, Custom],
    crate::dock::DockIndicatorStyle => [Dot, Line, Highlight],
    crate::dock::DockColorSource => [Accent, Neutral, Custom],
    crate::dock::DockBadgeCorner => [TopRight, TopLeft, BottomRight, BottomLeft],
    crate::dock::DockBadgeStyle => [Number, DotOnly],
    crate::dock::DockRevealTrigger => [EdgeTouch, EdgeHold, ShortcutOnly],
    crate::dock::MiddleClickAction => [NewWindow, Close],
    crate::dock::DockGroupingMode => [Never, Always, WhenSpaceRunsOut],
    crate::dock::DockWindowMonitorScope => [AllMonitors, CurrentMonitor],
    crate::dock::DockLeftClickAction => [BringToFront, MinimizeIfActive, CycleWindows, ShowThumbnails],
    crate::dock::DockLabelMode => [Never, Always, OnHover],
    crate::tray::TrayIconVisibility => [Shown, Overflow, Hidden],
    crate::notch::NotchStyleMode => [Pinned, Floating],
    crate::notch::NotchCollapsedBackground => [Default, AlbumColor, Ambient],
    crate::notch::NotchOpenBehavior => [Hover, Click, DragOver],
    crate::notch::NotchTabBarStyle => [IconOnly, IconAndLabel, Hidden],
    crate::notch::HomeCard => [Music, HeadphoneBattery, SystemNetwork, QuickLaunch, Weather, Timers],
    crate::notch::IdleDisplayMode => [ClockAndBattery, ClockOnly, Nothing, RingClockDay],
    crate::notch::WheelAction => [SwitchPanel, Volume],
    crate::notch::NotchEdgeLineColor => [NotchSurface, Accent],
    crate::clock::ClockHourFormat => [Auto, Hour12, Hour24],
    crate::clock::WeekStartSetting => [System, Saturday, Sunday, Monday],
    crate::notch::LiveBackgroundTarget => [None, Notch, MusicCard, Both],
    crate::notch::LiveBackgroundSourceKind => [BuiltInAmbient, LocalFile],
    crate::media::MusicCardStyle => [Neutral, Tinted, Artwork],
    crate::notch::NotchVisualizerStyle => [Bars, Wave, Dots],
    crate::notch::NotchVisualizerColorSource => [Accent, Artwork],
    crate::display::NewScreenMode => [NotchAndDock, NotchOnly, None],
    crate::input::CornerAction => [None, OpenNotch, OpenSearch, OpenKoyWindow, ToggleDock],
    crate::voice::VoiceHarnessType => [Claude, Codex, Agy, Custom],
    crate::voice::VoiceClaudePermission => [Default, Auto, AcceptEdits],
    crate::search::SearchHistoryMode => [Disabled, Suggestion, ImmediateCompletion],
    crate::weather::WeatherRefreshInterval => [Every30Minutes, Hourly, OnStartup],
    crate::weather::WeatherLocationSource => [None, Device, City],
}

// ---------------------------------------------------------------- liste ve sözlük

impl ListItem for String {
    const IS_TEXT: bool = true;

    fn decode_item(node: &Value) -> Result<Option<Self>, Fail> {
        // C#'ta null öğe listede null kalır (Rust'ta temsil edilemez): hata sayılır, öğe atılır.
        String::decode(node).map(Some)
    }

    fn encode_item(&self) -> Value {
        Value::String(self.clone())
    }
}

impl<T: ListItem> SettingValue for ValueList<T> {
    const NULL_KEEPS_DEFAULT: bool = true;
    const IS_STRING_LIST: bool = T::IS_TEXT;

    fn decode(node: &Value) -> Result<Self, Fail> {
        let Value::Array(items) = node else { return Err(Fail::here()) };
        let mut list = ValueList::new();
        for (i, item) in items.iter().enumerate() {
            match T::decode_item(item) {
                Ok(Some(v)) => list.push(v),
                Ok(None) => {}
                Err(f) => return Err(f.under(Seg::Index(i))),
            }
        }
        Ok(list)
    }

    fn encode(&self) -> Value {
        Value::Array(self.iter().map(ListItem::encode_item).collect())
    }
}

impl<T: ListItem> SettingValue for ValueDictionary<T> {
    const NULL_KEEPS_DEFAULT: bool = true;

    fn decode(node: &Value) -> Result<Self, Fail> {
        let Value::Object(map) = node else { return Err(Fail::here()) };
        let mut dict = ValueDictionary::new();
        for (key, value) in map {
            match T::decode_item(value) {
                Ok(Some(v)) => {
                    dict.insert(key, v);
                }
                Ok(None) => {}
                Err(f) => return Err(f.under(Seg::Key(key.clone()))),
            }
        }
        Ok(dict)
    }

    fn encode(&self) -> Value {
        let mut map = Map::new();
        for (k, v) in self.entries() {
            map.insert(k.clone(), v.encode_item());
        }
        Value::Object(map)
    }
}

// ---------------------------------------------------------------- başka modüllerin kayıtları

/// Kayıt alanı denetimi: ad → beklenen tür. `null` metin alanı boş metne çevrilir (C#'ta null atanır; tüketiciler
/// null denetimi yapmaz, bkz. `Dock_item_entry_with_null_fields_does_not_break_normalize`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Text,
    Bool,
}

fn precheck_record(node: &Value, shapes: &[(&str, Shape)], keep_null: &[&str]) -> Result<Map<String, Value>, Fail> {
    let Value::Object(obj) = node else { return Err(Fail::here()) };
    let mut obj = obj.clone();
    for (name, shape) in shapes {
        let Some(v) = obj.get(*name) else { continue };
        let ok = match shape {
            Shape::Text => match v {
                Value::String(_) => true,
                Value::Null => true,
                _ => false,
            },
            Shape::Bool => v.is_boolean(),
        };
        if !ok {
            return Err(Fail::here().under(Seg::Prop((*name).to_string())));
        }
        if *shape == Shape::Text && v.is_null() && !keep_null.contains(name) {
            obj.insert((*name).to_string(), Value::String(String::new()));
        }
    }
    Ok(obj)
}

impl ListItem for crate::dock::DockFolder {
    fn decode_item(node: &Value) -> Result<Option<Self>, Fail> {
        let mut obj = precheck_record(node, &[("Id", Shape::Text), ("Name", Shape::Text)], &[])?;
        // "Items": ["x.exe", 7, "y.exe"] → yalnız 7 düşer (en içteki liste öğesi).
        match obj.get("Items") {
            Some(Value::Array(items)) => {
                for (i, item) in items.iter().enumerate() {
                    if !item.is_string() {
                        return Err(Fail::here().under(Seg::Index(i)).under(Seg::Prop("Items".into())));
                    }
                }
            }
            Some(Value::Null) => {
                obj.shift_remove("Items");
            }
            Some(_) => return Err(Fail::here().under(Seg::Prop("Items".into()))),
            None => {}
        }
        serde_json::from_value(Value::Object(obj)).map(Some).map_err(|_| Fail::here())
    }

    fn encode_item(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

impl ListItem for crate::dock::DockItemEntry {
    fn decode_item(node: &Value) -> Result<Option<Self>, Fail> {
        // "Items": [null, ...]: boş kayıt sınırdan önce sessizce atılır (FillMissingSections).
        if node.is_null() {
            return Ok(None);
        }
        let obj = precheck_record(node, &[("Kind", Shape::Text), ("Id", Shape::Text), ("Command", Shape::Text)], &[])?;
        serde_json::from_value(Value::Object(obj)).map(Some).map_err(|_| Fail::here())
    }

    fn encode_item(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

impl ListItem for crate::display::MonitorSettings {
    fn decode_item(node: &Value) -> Result<Option<Self>, Fail> {
        let obj = precheck_record(
            node,
            &[("NotchEnabled", Shape::Bool), ("DockEnabled", Shape::Bool), ("Name", Shape::Text)],
            &["Name"],
        )?;
        serde_json::from_value(Value::Object(obj)).map(Some).map_err(|_| Fail::here())
    }

    fn encode_item(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

// ---------------------------------------------------------------- bölüm makrosu

/// Ayar bölümü kaydı tanımlar: yapı, `Default`, [`SettingValue`] ve [`SectionModel`].
/// Her alan: `rust_adı: Tip = varsayılan => "JsonAdı"` (JSON adı C# özellik adıdır).
macro_rules! section {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $(#[$fmeta:meta])* $field:ident : $ty:ty = $def:expr => $json:literal ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name {
            $( $(#[$fmeta])* pub $field: $ty, )*
        }

        impl Default for $name {
            fn default() -> Self {
                Self { $( $field: $def, )* }
            }
        }

        impl $crate::settings::setting_value::SettingValue for $name {
            fn decode(node: &serde_json::Value) -> Result<Self, $crate::settings::json_tree::Fail> {
                match node {
                    // Boş bırakılmış bölüm: varsayılan (C#'ta null, FillMissingSections doldurur).
                    serde_json::Value::Null => Ok(Self::default()),
                    serde_json::Value::Object(map) => {
                        #[allow(unused_mut)]
                        let mut s = Self::default();
                        $(
                            if let Some(v) = map.get($json) {
                                if !(v.is_null() && <$ty as $crate::settings::setting_value::SettingValue>::NULL_KEEPS_DEFAULT) {
                                    s.$field = <$ty as $crate::settings::setting_value::SettingValue>::decode(v)
                                        .map_err(|f| f.under($crate::settings::json_tree::Seg::Prop($json.to_string())))?;
                                }
                            }
                        )*
                        Ok(s)
                    }
                    _ => Err($crate::settings::json_tree::Fail::here()),
                }
            }

            fn encode(&self) -> serde_json::Value {
                #[allow(unused_mut)]
                let mut m = serde_json::Map::new();
                $( m.insert($json.to_string(), <$ty as $crate::settings::setting_value::SettingValue>::encode(&self.$field)); )*
                serde_json::Value::Object(m)
            }
        }

        impl $crate::settings::setting_value::SectionModel for $name {
            fn describe(&self, defaults: &Self) -> Vec<$crate::settings::setting_value::FieldView> {
                vec![ $(
                    $crate::settings::setting_value::FieldView {
                        name: $json,
                        kind: <$ty as $crate::settings::setting_value::SettingValue>::KIND,
                        choices: <$ty as $crate::settings::setting_value::SettingValue>::choices(),
                        value: <$ty as $crate::settings::setting_value::SettingValue>::format(&self.$field),
                        default: <$ty as $crate::settings::setting_value::SettingValue>::format(&defaults.$field),
                        same: self.$field == defaults.$field,
                        is_string_list: <$ty as $crate::settings::setting_value::SettingValue>::IS_STRING_LIST,
                    },
                )* ]
            }

            fn perturb(&self, field: &str) -> Option<Self> {
                $(
                    if field == $json {
                        let v = <$ty as $crate::settings::setting_value::SettingValue>::different(&self.$field)?;
                        let mut copy = self.clone();
                        copy.$field = v;
                        return Some(copy);
                    }
                )*
                let _ = field;
                None
            }
        }
    };
}

pub(crate) use section;
