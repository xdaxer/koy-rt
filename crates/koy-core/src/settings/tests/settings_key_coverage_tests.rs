//! `Settings/SettingsKeyCoverageTests.cs`: v3 → v4 anahtar tablosunun tutarlılığı. Altın dosyadaki her düz anahtar ya
//! tabloda ya da bilerek atılanlar listesinde olmalı; tablodaki her hedef gerçekten var olan bir bölüm ve alan olmalı.
//! Yazım hatası ya da unutulmuş alan CI'da kırılır.

use std::collections::HashMap;

use serde_json::Value;

use super::FIXTURE_V3;
use crate::settings::*;

#[test]
fn every_key_in_the_v3_golden_file_is_mapped_or_explicitly_dropped() {
    let Value::Object(fixture) = serde_json::from_str::<Value>(FIXTURE_V3).unwrap() else { panic!("nesne bekleniyordu") };

    let unknown: Vec<&String> = fixture
        .keys()
        .filter(|k| {
            !SettingsKeyMap::top_level().contains(&k.as_str())
                && !SettingsKeyMap::v3_to_v4().contains_key(k.as_str())
                && !SettingsKeyMap::dropped().contains(&k.as_str())
        })
        .collect();

    assert!(unknown.is_empty(), "{unknown:?}");
}

#[test]
fn every_mapping_target_is_a_real_section_and_field() {
    let defaults = AppSettings::default();
    let views = defaults.section_views(&defaults);
    let mut problems = Vec::new();
    for (v3_key, (section, name)) in SettingsKeyMap::v3_to_v4() {
        match views.iter().find(|v| v.name == section) {
            None => problems.push(format!("{v3_key} -> section '{section}' yok")),
            Some(view) => {
                if !view.fields.iter().any(|f| f.name == name) {
                    problems.push(format!("{v3_key} -> {section}.{name} yok"));
                }
            }
        }
    }

    assert!(problems.is_empty(), "{problems:?}");
}

#[test]
fn no_two_v3_keys_map_to_the_same_v4_field() {
    let mut counts: HashMap<String, i32> = HashMap::new();
    for (section, name) in SettingsKeyMap::v3_to_v4().values() {
        *counts.entry(format!("{section}.{name}")).or_default() += 1;
    }
    let targets: Vec<&String> = counts.iter().filter(|(_, c)| **c > 1).map(|(k, _)| k).collect();

    assert!(targets.is_empty(), "{targets:?}");
}
