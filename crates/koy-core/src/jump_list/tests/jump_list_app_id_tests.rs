//! Karşılığı: `tests/Core.Tests/JumpList/JumpListAppIdTests.cs`.

use crate::jump_list::JumpListAppId;

// Windows 11 build 26200'de doğrulanmış vektörler (ROADMAP).
#[test]
fn known_vectors_match() {
    for (app_id, expected) in [
        ("Chrome", "5d696d521de238c3"),
        ("MSEdge", "ccba5a5986c77e43"),
        ("Microsoft.VisualStudioCode", "1ced32d74a95c7bc"),
    ] {
        assert_eq!(Some(expected.to_string()), JumpListAppId::hash_name(Some(app_id)), "{app_id}");
    }
}

#[test]
fn hash_ignores_case_because_the_id_is_uppercased() {
    assert_eq!(JumpListAppId::hash_name(Some("Chrome")), JumpListAppId::hash_name(Some("CHROME")));
    assert_eq!(JumpListAppId::hash_name(Some("Chrome")), JumpListAppId::hash_name(Some("chrome")));
}

#[test]
fn empty_id_has_no_hash() {
    for app_id in [None, Some(""), Some("   ")] {
        assert_eq!(None, JumpListAppId::hash_name(app_id), "{app_id:?}");
    }
}

#[test]
fn hash_has_no_leading_zero_padding() {
    // Windows %x ile yazar; baştaki sıfırlar atılır (diskte 15 haneli adlar var). Çok sayıda kimlikten biri
    // 1/16 olasılıkla sıfırla başlar; hiçbirinde 16 haneden uzun ad ya da sıfırla başlayan ad çıkmamalı.
    for i in 0..400 {
        let name = JumpListAppId::hash_name(Some(&format!("Test.App.{i}"))).unwrap();
        assert!((1..=16).contains(&name.len()));
        assert!(!(name.len() > 1 && name.starts_with('0')));
        assert_eq!(name, name.to_lowercase());
    }
}
