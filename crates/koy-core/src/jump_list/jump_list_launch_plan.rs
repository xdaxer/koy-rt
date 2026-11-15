//! Karşılığı: `JumpList/JumpListLaunchPlan.cs`.

use super::jump_list_models::JumpListLaunch;
use super::win_path;
use crate::text;

/// Paketli uygulamaların pencere sahipleri: exe yolu uygulamayı temsil etmez, belge bu exe ile açılmaz.
const HOST_EXECUTABLES: [&str; 1] = ["ApplicationFrameHost.exe"];

/// Bir girdinin çalıştırılma biçimi: `file` kabukla (ShellExecute) açılır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpListLaunchPlan {
    pub file: String,
    pub arguments: Option<String>,
    pub working_directory: Option<String>,
}

impl JumpListLaunchPlan {
    /// Çalıştırma planı (C#: `JumpListLaunchPlan.For`; `for` Rust'ta anahtar sözcük). Görev kısayolu olduğu gibi çalışır.
    /// Belge girdisi (son / sık kullanılan) mümkünse uygulamanın kendi exe'siyle açılır: Windows'un atlama listesi de
    /// belgeyi o uygulamada açar (ör. VS Code'un son klasörü Gezgin'de değil VS Code'da açılmalı). Kabuk kısayolu
    /// (.lnk, .url), exe, paketli uygulama yolu ve exe'si bilinmeyen uygulamalarda hedef varsayılan ilişkiyle açılır.
    pub fn for_launch(launch: &JumpListLaunch, app_exe_path: Option<&str>) -> JumpListLaunchPlan {
        if !launch.is_document || !can_open_with_app(&launch.target, app_exe_path) {
            return JumpListLaunchPlan {
                file: launch.target.clone(),
                arguments: launch.arguments.clone(),
                working_directory: launch.working_directory.clone(),
            };
        }

        // Sondaki ters bölü (D:\) kapanış tırnağını kaçırır; komut satırı kuralı gereği ikilenir.
        let target = if launch.target.ends_with('\\') { format!("{}\\", launch.target) } else { launch.target.clone() };
        JumpListLaunchPlan {
            file: app_exe_path.unwrap_or_default().to_string(),
            arguments: Some(format!("\"{target}\"")),
            working_directory: launch.working_directory.clone(),
        }
    }
}

fn can_open_with_app(target: &str, app_exe_path: Option<&str>) -> bool {
    let app_exe_path = match app_exe_path {
        Some(p)
            if !text::is_null_or_white_space(Some(p))
                && text::ends_with_ignore_case(p, ".exe")
                && !text::contains_ignore_case(p, "\\WindowsApps\\") =>
        {
            p
        }
        _ => return false,
    };

    let exe_name = win_path::get_file_name(app_exe_path);
    if HOST_EXECUTABLES.iter().any(|h| text::eq_ignore_case(h, exe_name)) {
        return false;
    }

    // Tırnak içine alacağımız hedefte tırnak olamaz (Windows yollarında zaten geçersiz); URL olmayan kabuk adlarını
    // (::{CLSID}, shell:) ve kısayol / program dosyalarını ilişkilendirmeye bırak.
    if target.contains('"') || target.starts_with("::") || text::starts_with_ignore_case(target, "shell:") {
        return false;
    }

    let ext = win_path::get_extension(target);
    !text::eq_ignore_case(ext, ".lnk") && !text::eq_ignore_case(ext, ".url") && !text::eq_ignore_case(ext, ".exe")
}
