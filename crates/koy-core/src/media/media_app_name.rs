use crate::text;

/// Medya oturumunun kaynak kimliğini (AUMID ya da exe yolu) kullanıcıya okunur bir uygulama adına çevirir.
pub struct MediaAppName;

// Paket ya da exe adı → ürün adı (anahtarlar OrdinalIgnoreCase). Listede olmayan kimlikler için ad kimlikten türetilir.
const KNOWN_APPS: [(&str, &str); 11] = [
    ("Microsoft.ZuneMusic", "Media Player"),
    ("Microsoft.ZuneVideo", "Movies & TV"),
    ("SpotifyAB.SpotifyMusic", "Spotify"),
    ("Spotify", "Spotify"),
    ("chrome", "Chrome"),
    ("msedge", "Edge"),
    ("firefox", "Firefox"),
    ("opera", "Opera"),
    ("brave", "Brave"),
    ("vlc", "VLC"),
    ("AppleInc.AppleMusicWin", "Apple Music"),
];

fn known_app(name: &str) -> Option<&'static str> {
    KNOWN_APPS.iter().find(|(k, _)| text::eq_ignore_case(k, name)).map(|(_, v)| *v)
}

impl MediaAppName {
    pub fn from_source_app_id(source_app_id: Option<&str>) -> String {
        if text::is_null_or_white_space(source_app_id) {
            return String::new();
        }

        let id = source_app_id.unwrap_or_default().trim();

        // Store uygulaması: "Paket.Adı_yayıncıKarması!UygulamaKimliği"
        if let Some(bang) = id.find('!') {
            let mut package = &id[..bang];
            if let Some(underscore) = package.find('_') {
                if underscore > 0 {
                    package = &package[..underscore];
                }
            }

            if let Some(known) = known_app(package) {
                return known.to_string();
            }

            // "Yayıncı.Uygulama" → "Uygulama"
            return match package.rfind('.') {
                Some(dot) if dot < package.len() - 1 => package[dot + 1..].to_string(),
                _ => package.to_string(),
            };
        }

        // Masaüstü uygulaması: tam yol ya da "chrome.exe"
        let mut name = match id.rfind(['\\', '/']) {
            Some(slash) => &id[slash + 1..],
            None => id,
        };
        if text::ends_with_ignore_case(name, ".exe") {
            name = &name[..name.len() - 4];
        }

        if let Some(known) = known_app(name) {
            return known.to_string();
        }

        // `char.ToUpperInvariant(name[0]) + name[1..]`: ilk UTF-16 birimi büyütülür.
        let mut chars = name.chars();
        match chars.next() {
            Some(first) => {
                let mut result = text::to_upper_invariant(&first.to_string());
                result.push_str(chars.as_str());
                result
            }
            None => name.to_string(),
        }
    }
}
