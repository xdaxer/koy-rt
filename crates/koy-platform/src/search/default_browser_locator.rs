//! Kullanıcının varsayılan tarayıcısını kayıt defterinden okur: https (yoksa http) için seçilen ProgId'nin "açma"
//! komutu. Tanıma kararı Core'da (`DefaultBrowserSearch`).

use koy_core::search::{DefaultBrowser, DefaultBrowserSearch};

use crate::registry::{self, Hive};

const SCHEMES: [&str; 2] = ["https", "http"];

pub struct DefaultBrowserLocator;

impl DefaultBrowserLocator {
    pub fn find() -> Option<DefaultBrowser> {
        for scheme in SCHEMES {
            let choice = format!(r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\{scheme}\UserChoice");
            let Some(prog_id) = registry::get_string(Hive::CurrentUser, &choice, Some("ProgId"), true).filter(|p| !p.is_empty()) else {
                continue;
            };
            let command = registry::get_string(Hive::ClassesRoot, &format!(r"{prog_id}\shell\open\command"), None, true);
            if let Some(browser) = DefaultBrowserSearch::from_open_command(command.as_deref())
                && std::path::Path::new(&browser.executable).is_file()
            {
                return Some(browser);
            }
        }
        None
    }
}
