use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use crate::quick_launch::*;
use crate::shell::IShellActionService;
use crate::text;

#[derive(Default)]
struct FakeShell {
    launched: Mutex<Vec<String>>,
    throw: Mutex<bool>,
}

impl IShellActionService for FakeShell {
    fn send_win_key(&self) {}
    fn toggle_system_tray(&self) {}
    fn open_quick_settings(&self) {}
    fn open_notification_center(&self) {}
    fn launch(&self, path: &str, _arguments: Option<&str>) -> Result<(), String> {
        if *self.throw.lock().unwrap() {
            return Err("başlatılamadı".to_owned());
        }
        self.launched.lock().unwrap().push(path.to_owned());
        Ok(())
    }
}

type Saves = Arc<Mutex<Vec<Vec<String>>>>;

fn save_callback(saves: &Saves) -> SaveCallback {
    let saves = Arc::clone(saves);
    Box::new(move |items| saves.lock().unwrap().push(items.to_vec()))
}

fn new_service(initial: &[&str]) -> (QuickLaunchService, Saves, Arc<FakeShell>) {
    let saves: Saves = Arc::default();
    let shell = Arc::new(FakeShell::default());
    let service = QuickLaunchService::new(initial.iter().copied(), save_callback(&saves), shell.clone(), Some(Box::new(|_| true)));
    (service, saves, shell)
}

fn last_save(saves: &Saves) -> Vec<String> {
    saves.lock().unwrap().last().unwrap().clone()
}

fn strs(items: &[String]) -> Vec<&str> {
    items.iter().map(String::as_str).collect()
}

#[test]
fn add_keeps_order_and_saves() {
    let (mut s, saves, _) = new_service(&[]);
    assert!(s.add(r"C:\a.exe"));
    assert!(s.add(r"C:\b.exe"));

    assert_eq!(vec![r"C:\a.exe", r"C:\b.exe"], strs(s.items()));
    assert_eq!(vec![r"C:\a.exe".to_owned(), r"C:\b.exe".to_owned()], last_save(&saves));
}

#[test]
fn add_rejects_duplicates_ignoring_case_and_trailing_slash() {
    let (mut s, _, _) = new_service(&[r"C:\Klasor"]);
    assert!(!s.add(r"c:\klasor\"));
    assert_eq!(1, s.items().len());
}

#[test]
fn add_strips_quotes_and_rejects_blank() {
    let (mut s, _, _) = new_service(&[]);
    assert!(s.add("  \"C:\\a.exe\" "));
    assert_eq!(r"C:\a.exe", s.items()[0]);
    assert!(!s.add("  "));
}

#[test]
fn add_rejects_missing_path() {
    let saves: Saves = Arc::default();
    let mut s = QuickLaunchService::new(Vec::<String>::new(), save_callback(&saves), Arc::new(FakeShell::default()), Some(Box::new(|_| false)));
    assert!(!s.add(r"C:\yok.exe"));
}

#[test]
fn add_stops_at_eight() {
    let (mut s, _, _) = new_service(&[]);
    for i in 0..8 {
        assert!(s.add(&format!(r"C:\{i}.exe")));
    }
    assert!(!s.add(r"C:\9.exe"));
    assert_eq!(8, s.items().len());
}

#[test]
fn initial_list_is_cleaned_deduped_and_capped() {
    let mut initial: Vec<String> = vec![r"C:\a".to_owned(), String::new(), r"c:\A".to_owned()];
    for i in 0..12 {
        initial.push(format!(r"C:\{i}"));
    }
    let initial_refs: Vec<&str> = initial.iter().map(String::as_str).collect();
    let (s, _, _) = new_service(&initial_refs);

    assert_eq!(8, s.items().len());
    assert_eq!(r"C:\a", s.items()[0]);
}

#[test]
fn remove_and_move() {
    let (mut s, saves, _) = new_service(&[r"C:\a", r"C:\b", r"C:\c"]);

    assert!(s.move_item(0, 2));
    assert_eq!(vec![r"C:\b", r"C:\c", r"C:\a"], strs(s.items()));
    assert!(!s.move_item(5, 0));
    assert!(s.remove(r"C:\C"));
    assert!(!s.remove(r"C:\yok"));
    assert_eq!(vec![r"C:\b".to_owned(), r"C:\a".to_owned()], last_save(&saves));
}

#[test]
fn launch_goes_through_shell_and_swallows_failure() {
    let (mut s, _, shell) = new_service(&[r"C:\a"]);
    assert!(s.launch(r"C:\a"));
    assert_eq!(vec![r"C:\a".to_owned()], *shell.launched.lock().unwrap());

    // C#'ta hata CrashLog'a yazılır; burada bağlanan hata günlüğüne.
    let logged: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
    let sink = Arc::clone(&logged);
    s.set_error_log(Box::new(move |source, message| sink.lock().unwrap().push((source.to_owned(), message.to_owned()))));

    *shell.throw.lock().unwrap() = true;
    assert!(!s.launch(r"C:\a"));
    assert_eq!(vec![("QuickLaunchService.Launch".to_owned(), "başlatılamadı".to_owned())], *logged.lock().unwrap());
}

#[test]
fn add_picked_shortcut_is_saved_and_raises_changed_once() {
    // Dosya seçiciden gelen yol: .lnk çözülmeden (DereferenceLinks=false) olduğu gibi eklenir ve kaydedilir.
    let (mut s, saves, _) = new_service(&[r"C:\a.exe"]);
    let changed = Arc::new(AtomicI32::new(0));
    let c = Arc::clone(&changed);
    s.changed().subscribe(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    });

    assert!(s.add(r"C:\Users\x\Masaüstü\Uygulama.lnk"));

    assert_eq!(vec![r"C:\a.exe", r"C:\Users\x\Masaüstü\Uygulama.lnk"], strs(s.items()));
    assert_eq!(s.items(), last_save(&saves).as_slice());
    assert_eq!(1, changed.load(Ordering::SeqCst));
}

#[test]
fn add_folder_is_saved_and_displayed() {
    let (mut s, saves, shell) = new_service(&[]);
    let changed = Arc::new(AtomicI32::new(0));
    let c = Arc::clone(&changed);
    s.changed().subscribe(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    });

    const FOLDER: &str = r"C:\Users\User\Projects";

    assert!(s.add(FOLDER));
    assert_eq!(1, s.items().len());
    assert_eq!(FOLDER, s.items()[0]);
    assert_eq!(vec![FOLDER.to_owned()], last_save(&saves));
    assert_eq!(1, changed.load(Ordering::SeqCst));
    assert_eq!("Projects", QuickLaunchService::display_name(FOLDER));

    assert!(s.launch(FOLDER));
    assert_eq!(vec![FOLDER.to_owned()], *shell.launched.lock().unwrap());
}

#[test]
fn add_folder_with_trailing_slash_is_normalized_and_prevents_duplicate() {
    let (mut s, saves, _) = new_service(&[]);
    assert!(s.add(r"C:\Projects\"));

    // Tekrar eden yollar (kuyruk çizgisi ve harf büyüklüğü fark etmeksizin) reddedilir
    assert!(!s.add(r"c:\projects"));
    assert!(!s.add(r"C:\Projects\"));
    assert_eq!(1, s.items().len());

    // Kuyruk çizgisi olmadan silme işlemi de aynı klasörü bulup siler
    assert!(s.remove(r"c:\projects"));
    assert!(s.items().is_empty());
    assert!(last_save(&saves).is_empty());
}

#[test]
fn add_folder_respects_directory_exists_predicate() {
    // `Dictionary<string, bool>(StringComparer.OrdinalIgnoreCase)`: anahtarlar büyük harfe çevrilerek.
    let exists_map: HashMap<String, bool> =
        HashMap::from([(text::to_upper_invariant(r"C:\ExistingFolder"), true), (text::to_upper_invariant(r"C:\MissingFolder"), false)]);
    let saves: Saves = Arc::default();
    let mut s = QuickLaunchService::new(
        Vec::<String>::new(),
        save_callback(&saves),
        Arc::new(FakeShell::default()),
        Some(Box::new(move |p| exists_map.get(&text::to_upper_invariant(p)).copied().unwrap_or(false))),
    );

    assert!(s.add(r"C:\ExistingFolder"));
    assert!(!s.add(r"C:\MissingFolder"));
    assert_eq!(1, s.items().len());
}

#[test]
fn add_network_share_folder_and_deduplicates() {
    let (mut s, _, _) = new_service(&[]);
    assert!(s.add(r"\\server\share"));

    // Büyük/küçük harf ve kuyruk çizgisi farkı olan ağ paylaşımları reddedilir
    assert!(!s.add(r"\\SERVER\share\"));
    assert_eq!(1, s.items().len());
}

#[test]
fn display_name() {
    let cases = [
        (r"C:\Program Files\App\App.exe", "App"),
        (r"C:\Users\x\Belgeler\", "Belgeler"),
        (r"C:\Users\x\Belgeler", "Belgeler"),
        (r"C:\", "C:"),
        (r"C:", "C:"),
        (r"D:\Projects\Koy", "Koy"),
        (r"D:\Projects\Koy\", "Koy"),
        (r"\\server\share", r"\\server\share"),
        (r"\\server\share\", r"\\server\share"),
        (r"\\server\share\belgeler", "belgeler"),
        (r"\\server\share\belgeler\", "belgeler"),
    ];
    for (path, expected) in cases {
        assert_eq!(expected, QuickLaunchService::display_name(path), "{path}");
    }
}
