use serde::{Deserialize, Serialize};

use crate::settings::AppSettings;
use crate::text;
use crate::win_path;

/// Dock'taki Başlat düğmesinin simgesi: Koy'un kendi simgesi (varsayılan), Windows logosu ya da kullanıcının dosyası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DockStartIcon {
    #[default]
    Koy,
    Windows,
    Custom,
}

/// Çözülmüş simge kaynağı. `path` yalnız `DockStartIcon::Custom` iken dolu.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DockStartIconSource {
    pub kind: DockStartIcon,
    pub path: Option<String>,
}

impl DockStartIconSource {
    pub fn new(kind: DockStartIcon, path: Option<String>) -> Self {
        DockStartIconSource { kind, path }
    }
}

/// Başlat düğmesinde hangi simgenin çizileceği kararı. Özel dosya seçilmiş ama yol boşsa, uzantı desteklenmiyorsa ya da
/// dosya yoksa Koy simgesine düşer: dock hiçbir zaman boş ya da bozuk bir düğmeyle açılmaz.
pub struct DockStartIconResolver;

impl DockStartIconResolver {
    /// Özel simge için kabul edilen uzantılar (WPF'in yerleşik çözücüleri).
    pub const SUPPORTED_EXTENSIONS: [&'static str; 4] = [".png", ".jpg", ".jpeg", ".ico"];

    pub fn is_supported_file(path: Option<&str>) -> bool {
        let Some(path) = path else { return false };
        if text::is_null_or_white_space(Some(path)) {
            return false;
        }

        let ext = win_path::get_extension(text::trim(path));
        Self::SUPPORTED_EXTENSIONS.iter().any(|e| text::eq_ignore_case(e, ext))
    }

    /// `file_exists`: Sınama için dosya denetimi; `None` ise gerçek dosya sistemi (`File.Exists`).
    pub fn resolve(kind: DockStartIcon, custom_path: Option<&str>, file_exists: Option<&dyn Fn(&str) -> bool>) -> DockStartIconSource {
        match kind {
            DockStartIcon::Windows => DockStartIconSource::new(DockStartIcon::Windows, None),
            DockStartIcon::Custom if Self::is_supported_file(custom_path) => {
                let path = text::trim(custom_path.unwrap_or(""));
                // Bozuk yol dosya denetiminde atabilir; dock'u düşürmek yerine Koy simgesine dön.
                let exists = match file_exists {
                    Some(f) => f(path),
                    None => std::path::Path::new(path).is_file(),
                };
                if exists {
                    return DockStartIconSource::new(DockStartIcon::Custom, Some(path.to_string()));
                }

                DockStartIconSource::new(DockStartIcon::Koy, None)
            }
            _ => DockStartIconSource::new(DockStartIcon::Koy, None),
        }
    }

    /// C# `Resolve(AppSettings settings, fileExists)` aşırı yüklemesi.
    pub fn resolve_settings(settings: &AppSettings, file_exists: Option<&dyn Fn(&str) -> bool>) -> DockStartIconSource {
        Self::resolve(settings.dock.start_icon, Some(settings.dock.start_icon_path.as_str()), file_exists)
    }
}
