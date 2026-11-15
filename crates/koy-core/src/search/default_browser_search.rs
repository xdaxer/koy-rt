use crate::text;
use crate::win_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BrowserFamily {
    #[default]
    Unknown,
    Chromium,
    Firefox,
}

/// `executable`: Tarayıcının exe yolu (varsayılan "açma" komutundan). `name`: Arama satırında görünen ad ("Chrome").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultBrowser {
    pub executable: String,
    pub name: String,
    pub family: BrowserFamily,
}

impl DefaultBrowser {
    pub fn new(executable: impl Into<String>, name: impl Into<String>, family: BrowserFamily) -> Self {
        DefaultBrowser { executable: executable.into(), name: name.into(), family }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserSearchCommand {
    pub executable: String,
    pub arguments: Vec<String>,
}

/// Hiç sonuç yokken web araması kullanıcının varsayılan tarayıcısında, o tarayıcının kendi arama motoruyla yapılır.
/// Arama motoru adresi tarayıcının profil dosyalarında saklı ve biçimi değişken; onun yerine tarayıcıya "ara" denir:
/// Chromium ailesi "? terim" adresini varsayılan motorla arar, Firefox ailesi `-search` seçeneğini tanır.
/// Tanınmayan tarayıcıda `None` döner, çağıran sabit bir motora düşer.
pub struct DefaultBrowserSearch;

impl DefaultBrowserSearch {
    /// `Known` sözlüğü (OrdinalIgnoreCase): dosya adı → (ad, aile).
    fn known(file_name: &str) -> Option<(&'static str, BrowserFamily)> {
        let key = text::to_lower_invariant(file_name);
        Some(match key.as_str() {
            "chrome" => ("Chrome", BrowserFamily::Chromium),
            "msedge" => ("Edge", BrowserFamily::Chromium),
            "brave" => ("Brave", BrowserFamily::Chromium),
            "vivaldi" => ("Vivaldi", BrowserFamily::Chromium),
            "chromium" => ("Chromium", BrowserFamily::Chromium),
            "thorium" => ("Thorium", BrowserFamily::Chromium),
            "firefox" => ("Firefox", BrowserFamily::Firefox),
            "librewolf" => ("LibreWolf", BrowserFamily::Firefox),
            "waterfox" => ("Waterfox", BrowserFamily::Firefox),
            "floorp" => ("Floorp", BrowserFamily::Firefox),
            "zen" => ("Zen", BrowserFamily::Firefox),
            _ => return None,
        })
    }

    /// Kayıt defterindeki "açma" komutundan (ör. `"C:\…\chrome.exe" --single-argument %1`) tarayıcıyı çıkarır.
    pub fn from_open_command(command: Option<&str>) -> Option<DefaultBrowser> {
        if text::is_null_or_white_space(command) {
            return None;
        }

        let text = command?.trim();
        let exe: &str;
        if let Some(rest) = text.strip_prefix('"') {
            // `text.IndexOf('"', 1)`: ilk tırnaktan sonrası; `end <= 1` boş yol demek.
            let end = rest.find('"')?;
            if end == 0 {
                return None;
            }
            exe = &rest[..end];
        } else {
            exe = match text.find(' ') {
                Some(space) => &text[..space],
                None => text,
            };
        }

        if !text::ends_with_ignore_case(exe, ".exe") {
            return None;
        }

        let (name, family) = Self::known(win_path::get_file_name_without_extension(exe))?;
        Some(DefaultBrowser::new(exe, name, family))
    }

    pub fn search_command(browser: &DefaultBrowser, term: &str) -> Option<BrowserSearchCommand> {
        let term = term.trim();
        if term.is_empty() {
            return None;
        }

        match browser.family {
            // --single-argument: ardından gelen her şey tek adres; terim "--" ile başlasa da seçenek sayılmaz.
            BrowserFamily::Chromium => Some(BrowserSearchCommand {
                executable: browser.executable.clone(),
                arguments: vec!["--single-argument".to_string(), format!("? {term}")],
            }),
            // Firefox'ta böyle bir koruma yok: tire ile başlayan terim seçenek sanılabilir, sabit motora bırakılır.
            BrowserFamily::Firefox if !term.starts_with('-') => Some(BrowserSearchCommand {
                executable: browser.executable.clone(),
                arguments: vec!["-search".to_string(), term.to_string()],
            }),
            _ => None,
        }
    }
}
