//! `MusicCardStyleSettingTests.cs` ve `VoicePermissionSettingTests.cs`.

use super::TempDir;
use crate::media::MusicCardStyle;
use crate::settings::*;
use crate::voice::VoiceClaudePermission;

#[test]
fn default_music_card_style_is_tinted() {
    assert_eq!(MusicCardStyle::Tinted, AppSettings::default().music.card_style);
}

#[test]
fn music_card_style_survives_a_save_and_load_round_trip() {
    let dir = TempDir::unique("koy-musicstyle-");
    let store = SettingsStore::new(dir.file("settings.json"));
    let settings = AppSettings { music: MusicSettings { card_style: MusicCardStyle::Artwork, ..Default::default() }, ..Default::default() };

    store.save(&settings).unwrap();
    let loaded = store.load();

    assert_eq!(MusicCardStyle::Artwork, loaded.music.card_style);
    assert_eq!(settings, loaded);
    assert_ne!(AppSettings::default(), loaded);
}

#[test]
fn default_voice_permission_is_read_only() {
    assert_eq!(VoiceClaudePermission::Default, AppSettings::default().voice.claude_permission);
}

#[test]
fn settings_file_without_the_key_stays_read_only() {
    // Eski kurulumun ayar dosyasında anahtar yok: güncelleme izni sessizce genişletmemeli.
    let dir = TempDir::new("koy-voiceperm-");
    let path = dir.file("settings.json");
    std::fs::write(&path, r#"{ "VoiceHarness": "Claude", "VoiceModel": "default" }"#).unwrap();

    let loaded = SettingsStore::new(&path).load();

    assert_eq!(VoiceClaudePermission::Default, loaded.voice.claude_permission);
}

#[test]
fn chosen_permission_and_model_survive_a_save_and_load_round_trip() {
    for permission in [VoiceClaudePermission::Auto, VoiceClaudePermission::AcceptEdits] {
        let dir = TempDir::unique("koy-voiceperm-");
        let store = SettingsStore::new(dir.file("settings.json"));
        let settings = AppSettings {
            voice: VoiceSettings { claude_permission: permission, model: "sonnet".to_string(), ..Default::default() },
            ..Default::default()
        };

        store.save(&settings).unwrap();
        let loaded = store.load();

        assert_eq!(permission, loaded.voice.claude_permission);
        assert_eq!("sonnet", loaded.voice.model);
        assert_eq!(settings, loaded);
    }
}
