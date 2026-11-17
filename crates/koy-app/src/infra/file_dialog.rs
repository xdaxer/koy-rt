//! Kabuk dosya / klasör seçicisi (WPF `Microsoft.Win32.OpenFileDialog` / `OpenFolderDialog` karşılığı).
//! Arayüz iş parçacığında çağrılır; iletişim kutusu kendi ileti döngüsüyle kapanana kadar döner.

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_NODEREFERENCELINKS, FOS_OVERWRITEPROMPT, FOS_PICKFOLDERS, FileOpenDialog, FileSaveDialog,
    IFileOpenDialog, IFileSaveDialog, SIGDN_FILESYSPATH,
};
use windows::core::{HSTRING, PCWSTR};

/// WPF süzgeç dizesini ("Ad|*.exe;*.lnk|Tümü|*.*") ad / desen çiftlerine ayırır.
pub fn parse_filter(filter: &str) -> Vec<(String, String)> {
    let parts: Vec<&str> = filter.split('|').collect();
    parts.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].to_string(), c[1].to_string())).collect()
}

/// Dosya seçer; iptalde None. `dereference_links = false`: kısayolun kendisi seçilir (hedefi değil).
pub fn pick_file(owner: Option<isize>, title: &str, filter: &str, dereference_links: bool) -> Option<String> {
    show(owner, title, Some(filter), false, dereference_links)
}

/// Klasör seçer; iptalde None.
pub fn pick_folder(owner: Option<isize>, title: &str) -> Option<String> {
    show(owner, title, None, true, true)
}

/// Kaydedilecek dosyayı sorar (WPF `SaveFileDialog`: üzerine yazma onayı açık); iptalde None. `default_ext` noktasız.
pub fn save_file(owner: Option<isize>, title: &str, filter: &str, file_name: &str, default_ext: &str) -> Option<String> {
    let specs: Vec<(HSTRING, HSTRING)> = parse_filter(filter).into_iter().map(|(n, p)| (HSTRING::from(n), HSTRING::from(p))).collect();
    let raw: Vec<COMDLG_FILTERSPEC> = specs.iter().map(|(n, p)| COMDLG_FILTERSPEC { pszName: PCWSTR(n.as_ptr()), pszSpec: PCWSTR(p.as_ptr()) }).collect();
    // SAFETY: COM nesnesi bu kapsamda yaşar; dizeler çağrı boyunca canlıdır; dönen yol CoTaskMemFree ile bırakılır.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let dialog: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let options = dialog.GetOptions().ok()? | FOS_FORCEFILESYSTEM | FOS_OVERWRITEPROMPT;
        dialog.SetOptions(options).ok()?;
        let _ = dialog.SetTitle(&HSTRING::from(title));
        if !raw.is_empty() {
            let _ = dialog.SetFileTypes(&raw);
        }
        let _ = dialog.SetFileName(&HSTRING::from(file_name));
        if !default_ext.is_empty() {
            let _ = dialog.SetDefaultExtension(&HSTRING::from(default_ext));
        }
        let owner = owner.map(|h| HWND(h as *mut _));
        dialog.Show(owner).ok()?;
        let item = dialog.GetResult().ok()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        path
    }
}

fn show(owner: Option<isize>, title: &str, filter: Option<&str>, folders: bool, dereference_links: bool) -> Option<String> {
    let specs: Vec<(HSTRING, HSTRING)> =
        filter.map(parse_filter).unwrap_or_default().into_iter().map(|(n, p)| (HSTRING::from(n), HSTRING::from(p))).collect();
    let raw: Vec<COMDLG_FILTERSPEC> = specs.iter().map(|(n, p)| COMDLG_FILTERSPEC { pszName: PCWSTR(n.as_ptr()), pszSpec: PCWSTR(p.as_ptr()) }).collect();
    // SAFETY: COM nesnesi bu kapsamda yaşar; süzgeç dizeleri (specs) çağrı boyunca canlıdır; dönen yol
    // CoTaskMemFree ile bırakılır.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let mut options = dialog.GetOptions().ok()? | FOS_FORCEFILESYSTEM;
        if folders {
            options |= FOS_PICKFOLDERS;
        } else {
            options |= FOS_FILEMUSTEXIST;
        }
        if !dereference_links {
            options |= FOS_NODEREFERENCELINKS;
        }
        dialog.SetOptions(options).ok()?;
        let _ = dialog.SetTitle(&HSTRING::from(title));
        if !raw.is_empty() {
            let _ = dialog.SetFileTypes(&raw);
        }
        let owner = owner.map(|h| HWND(h as *mut _));
        dialog.Show(owner).ok()?;
        let item = dialog.GetResult().ok()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_pairs() {
        assert_eq!(
            parse_filter("Apps|*.exe;*.lnk|All files|*.*"),
            vec![("Apps".to_string(), "*.exe;*.lnk".to_string()), ("All files".to_string(), "*.*".to_string())]
        );
        assert!(parse_filter("").is_empty());
    }
}
