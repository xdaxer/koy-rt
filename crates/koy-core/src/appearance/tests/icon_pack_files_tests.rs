//! Simge paketi eşleştirmesi ve uygulama anahtarı (dosya okumadan).

use crate::appearance::{AppIconKey, IconChoice, IconPack, IconPackFiles, IconResolver, IconSourceKind};

#[test]
fn key_prefers_aumid_then_shortcut_name() {
    let rows: [(Option<&str>, Option<&str>, &str); 6] = [
        (Some("Microsoft.WindowsTerminal_8wekyb3d8bbwe!App"), Some("Notepad.lnk"), "Microsoft.WindowsTerminal_8wekyb3d8bbwe!App"),
        (None, Some("Notepad.lnk"), "Notepad.lnk"),
        (Some(""), Some("Notepad.lnk"), "Notepad.lnk"),
        (None, Some(r"C:\Users\x\Start\Notepad.lnk"), "Notepad.lnk"),
        (None, Some("C:/Users/x/Notepad.lnk"), "Notepad.lnk"),
        (Some("bad\\aumid"), Some("Notepad.lnk"), "Notepad.lnk"),
    ];
    for (aumid, id, expected) in rows {
        assert_eq!(Some(expected.to_string()), AppIconKey::r#for(aumid, id), "{aumid:?} {id:?}");
    }
}

#[test]
fn key_is_null_when_nothing_valid() {
    let rows: [(Option<&str>, Option<&str>); 3] = [(None, None), (Some(""), Some("  ")), (None, Some(r"C:\folder\"))];
    for (aumid, id) in rows {
        assert_eq!(None, AppIconKey::r#for(aumid, id), "{aumid:?} {id:?}");
    }
}

#[test]
fn pack_folder_rejects_network_relative_and_unsafe_paths() {
    let rows: [Option<&str>; 7] = [
        Some(r"\\server\share\icons"),
        Some(r"\\?\C:\icons"),
        Some(r"C:\icons\..\other"),
        Some(r"relative\icons"),
        Some(r"C:\CON"),
        Some(""),
        None,
    ];
    for folder in rows {
        assert!(!IconPackFiles::is_safe_folder(folder), "{folder:?}");
    }
}

// Sürücü harfli yol çözümü yol kütüphanesine bağlı: yalnız Windows'ta anlamlı (Linux taban çizgisi AppIconOverrideTests ile aynı).
#[test]
fn pack_folder_accepts_local_drive_paths_on_windows() {
    if !cfg!(windows) {
        return;
    }

    for folder in [r"C:\Icons", r"C:\Icons\", r"  D:\Packs\Mine  "] {
        assert!(IconPackFiles::is_safe_folder(Some(folder)), "{folder}");
    }
}

#[test]
fn all_lists_aumid_then_shortcut_name_without_duplicates() {
    assert_eq!(vec!["App!Main", "Notepad.lnk"], AppIconKey::all(Some("App!Main"), Some(r"C:\x\Notepad.lnk")));
    assert_eq!(vec!["Notepad.lnk"], AppIconKey::all(None, Some("Notepad.lnk")));
    assert_eq!(vec!["notepad.lnk"], AppIconKey::all(Some("notepad.lnk"), Some("Notepad.lnk")));
    assert!(AppIconKey::all(None, None).is_empty());
}

#[test]
fn matches_by_stem_ignoring_case_and_shortcut_extension() {
    let files = ["firefox.PNG", "chrome.png"];

    assert_eq!(Some("firefox.PNG".to_string()), IconPackFiles::find(files, Some("Firefox.lnk"), false));
    assert_eq!(Some("chrome.png".to_string()), IconPackFiles::find(files, Some("Chrome.exe"), false));
    assert_eq!(None, IconPackFiles::find(files, Some("Edge.lnk"), false));
}

#[test]
fn prefers_png_then_ico_then_svg_regardless_of_listing_order() {
    assert_eq!(Some("a.png".to_string()), IconPackFiles::find(["a.svg", "a.ico", "a.png"], Some("a.lnk"), true));
    assert_eq!(Some("a.ico".to_string()), IconPackFiles::find(["a.svg", "a.ico"], Some("a.lnk"), true));
    assert_eq!(Some("a.svg".to_string()), IconPackFiles::find(["a.svg"], Some("a.lnk"), true));
}

#[test]
fn svg_is_ignored_when_the_renderer_cannot_show_it() {
    assert_eq!(None, IconPackFiles::find(["a.svg"], Some("a.lnk"), false));
    assert_eq!(Some("a.ico".to_string()), IconPackFiles::find(["a.svg", "a.ico"], Some("a.lnk"), false));
}

#[test]
fn aumid_keys_match_the_whole_name() {
    const AUMID: &str = "Microsoft.WindowsTerminal_8wekyb3d8bbwe!App";
    let file = format!("{AUMID}.png");
    assert_eq!(Some(file.clone()), IconPackFiles::find([file], Some(AUMID), false));
}

#[test]
fn unsafe_file_names_and_keys_are_skipped() {
    assert_eq!(None, IconPackFiles::find(["CON.png", "..\\a.png", "a.exe", "a.txt"], Some("a.lnk"), true));
    assert_eq!(None, IconPackFiles::find(["CON.png"], Some("CON.lnk"), true));
    assert_eq!(None, IconPackFiles::find(["a.png"], Some(""), true));
    assert_eq!(None, IconPackFiles::find(["a.png"], None, true));
}

#[test]
fn result_feeds_the_resolver_as_a_pack_choice() {
    let pack_file = IconPackFiles::find(["Notepad.png"], Some("Notepad.lnk"), false);

    let choice = IconResolver::resolve(Some("Notepad.lnk"), None, IconPack::Folder, pack_file.as_deref());

    assert_eq!(IconChoice::new(IconSourceKind::Pack, Some("Notepad.png".to_string())), choice);
}
