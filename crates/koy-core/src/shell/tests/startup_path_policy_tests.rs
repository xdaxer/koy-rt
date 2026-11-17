//! `Shell/StartupPathPolicyTests.cs`.

use crate::shell::StartupPathPolicy;

#[test]
fn resolve_target_path_kurulu_surumde_saplama_varsa_kok_saplamayi_secer() {
    let stub = r"C:\Users\u\AppData\Local\Koy\Koy.exe";
    let resolved = StartupPathPolicy::resolve_target_path(
        true,
        false,
        Some(r"C:\Users\u\AppData\Local\Koy"),
        Some(r"C:\Users\u\AppData\Local\Koy\current\Koy.exe"),
        |p| p.eq_ignore_ascii_case(stub),
    );
    assert_eq!(Some(stub.to_string()), resolved);
}

#[test]
fn resolve_target_path_saplama_yoksa_mevcut_surec_yoluna_duser() {
    let current = r"C:\Users\u\AppData\Local\Koy\current\Koy.exe";
    let resolved = StartupPathPolicy::resolve_target_path(true, false, Some(r"C:\Users\u\AppData\Local\Koy"), Some(current), |_| false);
    assert_eq!(Some(current.to_string()), resolved);
}

#[test]
fn resolve_target_path_tasinabilir_veya_gelistirmede_surec_yolunu_korur() {
    let dev = r"C:\src\Koy\bin\Debug\net10.0\Koy.exe";
    for (installed, portable) in [(false, false), (true, true)] {
        let resolved = StartupPathPolicy::resolve_target_path(installed, portable, Some(r"C:\src\Koy\bin\Debug\net10.0"), Some(dev), |_| true);
        assert_eq!(Some(dev.to_string()), resolved, "{installed} {portable}");
    }
}

#[test]
fn format_command_line_yolu_tirnaklar_ve_argumani_ekler() {
    assert_eq!(
        Some("\"C:\\Program Files\\Koy\\Koy.exe\"".to_string()),
        StartupPathPolicy::format_command_line(r"C:\Program Files\Koy\Koy.exe", None)
    );
    assert_eq!(
        Some("\"C:\\Users\\Ad Soyad\\Koy.exe\" --autostart".to_string()),
        StartupPathPolicy::format_command_line("\"C:\\Users\\Ad Soyad\\Koy.exe\"", Some("--autostart"))
    );
}

#[test]
fn extract_executable_path_cesitli_komut_satirlarini_cozumler() {
    let cases: [(&str, Option<&str>); 7] = [
        ("\"C:\\Users\\Ad Soyad\\AppData\\Local\\Koy\\Koy.exe\"", Some(r"C:\Users\Ad Soyad\AppData\Local\Koy\Koy.exe")),
        ("\"C:\\Users\\u\\AppData\\Local\\Koy\\Koy.exe\" --autostart", Some(r"C:\Users\u\AppData\Local\Koy\Koy.exe")),
        (r"C:\Program Files\Koy\Koy.exe --minimized", Some(r"C:\Program Files\Koy\Koy.exe")),
        (r"C:\Program Files\Koy\Koy.exe", Some(r"C:\Program Files\Koy\Koy.exe")),
        ("   ", None),
        ("\"\"", None),
        ("\"C:\\unclosed", None),
    ];
    for (cmd, expected) in cases {
        assert_eq!(expected.map(str::to_string), StartupPathPolicy::extract_executable_path(Some(cmd)), "{cmd}");
    }
}

#[test]
fn is_startup_approved_enabled_gorev_yoneticisi_durumunu_dogru_okur() {
    let cases: [(Option<&[u8]>, bool); 7] = [
        (None, true),
        (Some(&[]), true),
        (Some(&[0x02, 0, 0, 0]), true),
        (Some(&[0x06, 0, 0, 0]), true),
        (Some(&[0x03, 0, 0, 0]), false),
        (Some(&[0x07, 0, 0, 0]), false),
        (Some(&[0x01, 0, 0, 0]), false),
    ];
    for (bytes, expected) in cases {
        assert_eq!(expected, StartupPathPolicy::is_startup_approved_enabled(bytes), "{bytes:?}");
    }
}

const STUB: &str = r"C:\Users\u\AppData\Local\Koy\Koy.exe";

fn cmd(p: &str) -> String {
    StartupPathPolicy::format_command_line(p, None).unwrap()
}

#[test]
fn is_entry_valid_beklenen_saplama_ve_dosya_varsa_true() {
    assert!(StartupPathPolicy::is_entry_valid(Some(&cmd(STUB)), None, Some(STUB), |_| true));
}

#[test]
fn is_entry_valid_dosya_silinmisse_false() {
    assert!(!StartupPathPolicy::is_entry_valid(Some(&cmd(STUB)), None, Some(STUB), |_| false));
}

#[test]
fn is_entry_valid_current_klasorune_veya_gelistirmeye_bakiyorsa_false() {
    let current = cmd(r"C:\Users\u\AppData\Local\Koy\current\Koy.exe");
    let dev = cmd(r"C:\repo\koy\src\App\bin\Debug\net10.0\Koy.exe");
    assert!(!StartupPathPolicy::is_entry_valid(Some(&current), None, Some(STUB), |_| true));
    assert!(!StartupPathPolicy::is_entry_valid(Some(&dev), None, Some(STUB), |_| true));
}

#[test]
fn is_entry_valid_gorev_yoneticisinde_devre_disiysa_false() {
    let disabled = [0x03u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert!(!StartupPathPolicy::is_entry_valid(Some(&cmd(STUB)), Some(&disabled), Some(STUB), |_| true));
}

#[test]
fn should_migrate_installed_entry_eski_veya_current_yolunu_saplamaya_tasir() {
    let cases = [
        ("\"C:\\Users\\u\\AppData\\Local\\Koy\\current\\Koy.exe\"", true),
        ("\"C:\\repo\\src\\App\\bin\\Debug\\Koy.exe\"", true),
        ("\"C:\\Users\\u\\Downloads\\Koy-Portable\\Koy.exe\"", true),
        ("\"C:\\Users\\u\\AppData\\Local\\Koy\\Koy.exe\"", false),
    ];
    for (reg, expected) in cases {
        assert_eq!(expected, StartupPathPolicy::should_migrate_installed_entry(Some(reg), None, Some(STUB), |_| true), "{reg}");
    }
}

#[test]
fn should_migrate_installed_entry_silinmis_eski_yolu_da_tasir() {
    assert!(StartupPathPolicy::should_migrate_installed_entry(Some("\"C:\\old\\OldName.exe\""), None, Some(STUB), |_| false));
}

#[test]
fn should_migrate_installed_entry_gorev_yoneticisinde_kapatilmissa_dokunmaz() {
    let current = "\"C:\\Users\\u\\AppData\\Local\\Koy\\current\\Koy.exe\"";
    assert!(!StartupPathPolicy::should_migrate_installed_entry(Some(current), Some(&[0x03, 0, 0, 0]), Some(STUB), |_| true));
}
