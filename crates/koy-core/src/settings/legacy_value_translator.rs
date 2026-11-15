//! Karşılığı: `koy/src/Core/Settings/LegacyValueTranslator.cs`.

use serde_json::Value;

use crate::text;

// v3 → v4 değer çevirisi: sayı olarak (`IdleDisplayMode`…) ya da dile bağlı metin olarak ("Katı", "Kararlı"…) yazılmış
// ayarlar enum adına çevrilir. Eski `Normalize` yardımcılarının kabul ettiği her değer burada karşılığını bulur.
// Tanınmayan değer için `None` döner: göç o anahtarı düşürür, alan varsayılanını alır (dosyanın kalanı etkilenmez).

// Anahtarlar küçük harfle aranır (eski kod da ToLowerInvariant kullanırdı).
const THEME: &[(&str, &str)] = &[
    ("system", "System"), ("sistem", "System"),
    ("dark", "Dark"), ("koyu", "Dark"),
    ("light", "Light"), ("açık", "Light"), ("acik", "Light"),
];

const ANIMATION_SPEED: &[(&str, &str)] = &[
    ("sakin", "Calm"), ("calm", "Calm"), ("slow", "Calm"),
    ("normal", "Normal"),
    ("hızlı", "Fast"), ("hizli", "Fast"), ("fast", "Fast"),
    ("kapalı", "Off"), ("kapali", "Off"), ("off", "Off"), ("none", "Off"),
];

const UPDATE_CHANNEL: &[(&str, &str)] = &[
    ("kararlı", "Stable"), ("kararli", "Stable"), ("stable", "Stable"),
    ("önizleme", "Preview"), ("onizleme", "Preview"), ("preview", "Preview"),
];

const WEATHER_REFRESH: &[(&str, &str)] = &[("30m", "Every30Minutes"), ("1h", "Hourly"), ("startup", "OnStartup")];

const MATERIAL: &[(&str, &str)] = &[
    ("katı", "Solid"), ("kati", "Solid"), ("solid", "Solid"),
    ("buzlu", "Frosted"), ("frosted", "Frosted"), ("yarı saydam", "Frosted"), ("yari saydam", "Frosted"),
    ("translucent", "Frosted"), ("acrylic", "Frosted"), ("mica", "Frosted"),
    ("cam", "Glass"), ("glass", "Glass"),
];

const NOTCH_STYLE: &[(&str, &str)] = &[
    ("yapışık", "Pinned"), ("yapisik", "Pinned"), ("pinned", "Pinned"),
    ("yüzen", "Floating"), ("yuzen", "Floating"), ("yüzen ada", "Floating"), ("floating", "Floating"), ("island", "Floating"),
];

const MUSIC_CARD: &[(&str, &str)] = &[
    ("neutral", "Neutral"), ("düz", "Neutral"), ("duz", "Neutral"),
    ("tinted", "Tinted"),
    ("artwork", "Artwork"), ("albüm kapağı", "Artwork"), ("album kapagi", "Artwork"), ("cover", "Artwork"),
];

/// Düz (v3) anahtar için değer çevirisi var mı ve varsa sonucu. `None` (dış): anahtar tabloda yok, değer olduğu gibi
/// taşınır. `Some(None)`: tanınmayan değer, anahtar düşer. `Some(Some(ad))`: enum adı.
/// (C# `ByKey` sözlüğü + çeviri işlevi.)
pub fn translate(key: &str, node: &Value) -> Option<Option<String>> {
    Some(match key {
        "Theme" => text_map(node, THEME),
        "AnimationSpeed" => text_map(node, ANIMATION_SPEED),
        "UpdateChannel" => text_map(node, UPDATE_CHANNEL),
        "WeatherRefreshInterval" => text_map(node, WEATHER_REFRESH),
        "Material" => text_map(node, MATERIAL),
        "NotchStyle" => text_map(node, NOTCH_STYLE),
        "MusicCardStyle" => text_map(node, MUSIC_CARD),
        "IdleDisplayMode" => number(node, &["ClockAndBattery", "ClockOnly", "Nothing", "RingClockDay"]),
        "NotchOpenBehavior" => number(node, &["Hover", "Click"]),
        "WheelAction" => number(node, &["SwitchPanel", "Volume"]),
        "MiddleClickAction" => number(node, &["NewWindow", "Close"]),
        "DockIndicatorStyle" => number(node, &["Dot", "Line"]),
        _ => return None,
    })
}

fn text_map(node: &Value, map: &[(&str, &str)]) -> Option<String> {
    let Value::String(s) = node else { return None };
    let key = text::to_lower_invariant(s.trim());
    map.iter().find(|(k, _)| *k == key).map(|(_, v)| (*v).to_string())
}

/// Sayı → sıradaki enum adı (eski sabitler 0'dan başlıyordu). Aralık dışı ya da sayı olmayan değer `None`.
fn number(node: &Value, names: &[&str]) -> Option<String> {
    let n = node.as_i64().and_then(|n| i32::try_from(n).ok())?;
    if n >= 0 && (n as usize) < names.len() { Some(names[n as usize].to_string()) } else { None }
}
