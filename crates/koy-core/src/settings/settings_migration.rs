//! Karşılığı: `koy/src/Core/Settings/SettingsMigration.cs`.

use serde_json::{Map, Value};

use super::legacy_value_translator;
use super::{AppSettings, SettingsKeyMap};
use crate::text;

/// Eski sürümlerin ayar dosyalarını güncel şemaya taşır. JSON ağacı üstünde çalışır (tipli okumadan önce): tip
/// değişse bile eski dosya okunabilir ve bilinmeyen değer yalnız kendi alanını bozar. Saf: monitör listesi gerekmez.
pub struct SettingsMigration;

/// Dosyada "Version" yoksa varsayılan: tipli dönemde eksik sürüm güncel sayılırdı.
const MISSING_VERSION_ASSUMED: i32 = 3;

impl SettingsMigration {
    /// Ağacı yerinde güncel şemaya çevirir ve dosyadaki ÖZGÜN sürümü döner.
    ///
    /// Sürüm 2 → 3: tek hedefli `MonitorTarget` yerine monitör başına ayar.
    /// - "Primary" ve "All": eski varsayılan "yalnız ana ekran" idi ve değiştirmek için arayüz yoktu; çoklu ekran artık
    ///   gerçek olduğundan kayıt yazılmaz, bütün ekranlar yeni ekran varsayılanını (hepsi açık) alır.
    /// - Belirli ekran (elle yazılmış aygıt adı ya da kimlik): yalnız o ekran açık kalır, diğerleri (yeni ekran
    ///   varsayılanı "hiçbiri" olduğundan) kapalı. Anahtar eski haliyle yazılır; ekran ilk bulunduğunda
    ///   `MonitorPolicy.KeyMatches` eşler, ilk düzenlemede kararlı kimliğe çevrilir.
    pub fn migrate(root: &mut Map<String, Value>) -> i32 {
        let original = read_version(root);
        if original >= AppSettings::CURRENT_VERSION {
            return original;
        }

        if original < 3 {
            migrate_v2_to_v3(root);
        }
        if original < 4 {
            migrate_v3_to_v4(root);
        }

        root.insert("Version".to_string(), Value::from(AppSettings::CURRENT_VERSION));
        original
    }
}

/// Sürüm 3 → 4: (1) sayı ve dile bağlı metin değerleri enum adına çevrilir (`LegacyValueTranslator`),
/// (2) düz anahtarlar bölümlere taşınır (`SettingsKeyMap`). Tanınmayan değerin anahtarı düşer, alan
/// varsayılanını alır; tanınmayan üst düzey anahtar (elle yazılmış ya da gelecek sürüm) atılır. Dosyanın tamamı sıfırlanmaz.
fn migrate_v3_to_v4(root: &mut Map<String, Value>) {
    let mut sections: Vec<(String, Map<String, Value>)> = Vec::new();

    for (key, node) in root.iter() {
        let Some((section_name, name)) = SettingsKeyMap::v3_to_v4().get(key) else { continue };

        let value = match legacy_value_translator::translate(key, node) {
            Some(translated) => {
                // Açık null değer enum'a okunamaz; tanınmayan sayılır.
                match translated {
                    Some(name) => Value::String(name),
                    None => continue,
                }
            }
            None => node.clone(),
        };

        let index = match sections.iter().position(|(n, _)| n == section_name) {
            Some(i) => i,
            None => {
                sections.push((section_name.clone(), Map::new()));
                sections.len() - 1
            }
        };
        sections[index].1.insert(name.clone(), value);
    }

    // C# ile aynı: "Version" yoksa null bir düğüm yazılır (hemen ardından Migrate güncel sürümü yazar).
    let version = root.get("Version").cloned().unwrap_or(Value::Null);
    root.clear();
    root.insert("Version".to_string(), version);
    for (name, section) in sections {
        root.insert(name, Value::Object(section));
    }
}

fn read_version(root: &Map<String, Value>) -> i32 {
    match root.get("Version").and_then(Value::as_i64).and_then(|v| i32::try_from(v).ok()) {
        Some(v) => v,
        None => MISSING_VERSION_ASSUMED,
    }
}

fn migrate_v2_to_v3(root: &mut Map<String, Value>) {
    let created = !matches!(root.get("Monitors"), Some(Value::Object(_)));
    let mut monitors = match root.get("Monitors") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    };
    let target: Option<String> = match root.get("MonitorTarget") {
        Some(Value::String(s)) => Some(s.clone()),
        _ => None,
    };

    // DisplayLayoutResolver.ParseMode(target) == Specific: "All" / boş / "Primary" dışındaki her değer.
    let specific = match target.as_deref() {
        Some(t) => {
            !text::eq_ignore_case(t, "All") && !text::is_null_or_white_space(Some(t)) && !text::eq_ignore_case(t, "Primary")
        }
        None => false,
    };

    if specific && !text::is_null_or_white_space(target.as_deref()) {
        let target = target.unwrap_or_default();
        // Monitör anahtarları büyük/küçük harf duyarsız (MonitorPolicy): aynı ekranın eski kaydı yinelenmesin.
        let existing: Vec<String> = monitors.keys().filter(|k| text::eq_ignore_case(k, &target)).cloned().collect();
        for key in existing {
            monitors.shift_remove(&key);
        }

        let mut entry = Map::new();
        entry.insert("NotchEnabled".to_string(), Value::Bool(true));
        entry.insert("DockEnabled".to_string(), Value::Bool(true));
        monitors.insert(target, Value::Object(entry));
        root.insert("NewScreenDefault".to_string(), Value::String("None".to_string()));
    }

    // Yeni kurulan sözlük köke eklenir; var olan C#'ta yerinde değiştirilirdi (burada aynı anahtara geri yazılır, sıra korunur).
    let _ = created;
    root.insert("Monitors".to_string(), Value::Object(monitors));
}
