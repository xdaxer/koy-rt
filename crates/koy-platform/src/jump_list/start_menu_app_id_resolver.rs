//! Paketsiz uygulamanın (Discord gibi, AUMID'i süreç düzeyinde koyan) AppUserModelID'ini Başlat menüsü kısayolundan bulur:
//! hedefi exe olan (ya da Squirrel `Update.exe --processStart` düzeni) ve AUMID'i olan ilk kısayol. Yalnız sağ tıkta ve
//! `JumpListService`'in kısa ömürlü STA iş parçacığında çağrılır (ShellLink COM'u apartman ister). Tarama sınırlıdır:
//! en çok [`MAX_SHORTCUTS`] kısayol, [`MAX_DEPTH`] klasör derinliği.

use std::path::{Path, PathBuf};

use koy_core::jump_list::ShortcutAppIdMatcher;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile, STGM_READ};
use windows::Win32::UI::Shell::{FOLDERID_CommonStartMenu, FOLDERID_StartMenu, IShellLinkW, ShellLink};
use windows::core::{HSTRING, Interface};

use crate::search::win_env::known_folder;
use crate::shell::AppUserModelIdReader;

pub const MAX_SHORTCUTS: i32 = 2000;
pub const MAX_DEPTH: i32 = 4;
const TEXT_BUFFER: usize = 1024;

pub(crate) struct StartMenuAppIdResolver;

impl StartMenuAppIdResolver {
    pub(crate) fn resolve(exe_path: &str) -> Option<String> {
        let mut roots = Vec::with_capacity(2);
        add_root(&mut roots, &known_folder(&FOLDERID_StartMenu));
        add_root(&mut roots, &known_folder(&FOLDERID_CommonStartMenu));

        // SAFETY: STA'da ShellLink nesnesi; arayüzler Drop ile bırakılır.
        let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.ok()?;
        let file: IPersistFile = link.cast().ok()?;

        let mut squirrel: Option<String> = None;
        let mut budget = MAX_SHORTCUTS;
        let mut paths = Vec::new();
        for root in &roots {
            enumerate(root, 0, &mut budget, &mut paths);
        }
        for path in paths {
            let Some((target, args)) = try_read(&file, &link, &path) else { continue };
            let exact = ShortcutAppIdMatcher::matches_exact(exe_path, Some(&target));
            if !exact && (squirrel.is_some() || !ShortcutAppIdMatcher::matches_squirrel(exe_path, Some(&target), Some(&args))) {
                continue;
            }
            let Some(aumid) = read_aumid(&path) else { continue };
            if exact {
                return Some(aumid);
            }
            squirrel = Some(aumid);
        }
        squirrel
    }
}

/// Eşleşen kısayolun AUMID'i. Özellik deposu ShellLink nesnesinde önbelleklendiğinden (sonraki Load'larda sıfırlanmıyor)
/// her eşleşme için taze nesne kullanılır.
fn read_aumid(path: &Path) -> Option<String> {
    // SAFETY: STA'da taze ShellLink nesnesi.
    unsafe {
        let fresh: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let file: IPersistFile = fresh.cast().ok()?;
        file.Load(&HSTRING::from(path.as_os_str()), STGM_READ).ok()?;
        AppUserModelIdReader::for_shell_link(&fresh.cast().ok()?)
    }
}

fn add_root(roots: &mut Vec<PathBuf>, start_menu: &str) {
    if start_menu.is_empty() {
        return;
    }
    let programs = Path::new(start_menu).join("Programs");
    if programs.is_dir() {
        roots.push(programs);
    }
}

/// Önce klasörün .lnk dosyaları, sonra alt klasörler (C#'taki sıra); bütçe bitince durur.
fn enumerate(dir: &Path, depth: i32, budget: &mut i32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let (mut files, mut subs): (Vec<PathBuf>, Vec<PathBuf>) = (Vec::new(), Vec::new());
    for e in entries.filter_map(|e| e.ok()) {
        let p = e.path();
        if p.is_dir() {
            subs.push(p);
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")) {
            files.push(p);
        }
    }
    files.sort();
    subs.sort();
    for f in files {
        if *budget <= 0 {
            return;
        }
        *budget -= 1;
        out.push(f);
    }
    if depth >= MAX_DEPTH {
        return;
    }
    for sub in subs {
        if *budget <= 0 {
            return;
        }
        enumerate(&sub, depth + 1, budget, out);
    }
}

fn try_read(file: &IPersistFile, link: &IShellLinkW, path: &Path) -> Option<(String, String)> {
    let mut buffer = [0u16; TEXT_BUFFER];
    // SAFETY: STA'da; arabellekler TEXT_BUFFER karakter.
    unsafe {
        file.Load(&HSTRING::from(path.as_os_str()), STGM_READ).ok()?;
        link.GetPath(&mut buffer, std::ptr::null_mut(), 0).ok()?;
        let target = String::from_utf16_lossy(&buffer[..buffer.iter().position(|&c| c == 0).unwrap_or(0)]);
        buffer.fill(0);
        let _ = link.GetArguments(&mut buffer);
        let args = String::from_utf16_lossy(&buffer[..buffer.iter().position(|&c| c == 0).unwrap_or(0)]);
        (!target.is_empty()).then_some((target, args))
    }
}
