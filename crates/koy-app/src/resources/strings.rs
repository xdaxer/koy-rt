//! Yerelleştirilmiş metinler (C# `Strings` + `ResourceManager`). Tablolar derleme anında .resx dosyalarından üretilir.
//! Geçerli dil `set_ui_culture` ile seçilir (C# `CultureInfo.CurrentUICulture`); Türkçe tabloda olmayan anahtar
//! İngilizce varsayılana, orada da yoksa anahtarın kendisine düşer (C# `?? name`).

use std::sync::atomic::{AtomicBool, Ordering};

mod generated {
    include!(concat!(env!("OUT_DIR"), "/strings_gen.rs"));
}

static TURKISH: AtomicBool = AtomicBool::new(true);

fn lookup(table: &'static [(&'static str, &'static str)], key: &str) -> Option<&'static str> {
    table.binary_search_by(|(k, _)| (*k).cmp(key)).ok().map(|i| table[i].1)
}

pub struct Strings;

impl Strings {
    /// `settings.General.Language == "en"` ise İngilizce, aksi halde Türkçe (C#: en-US / tr-TR).
    pub fn set_ui_culture(language: &str) {
        TURKISH.store(language != "en", Ordering::SeqCst);
    }

    pub fn is_turkish() -> bool {
        TURKISH.load(Ordering::SeqCst)
    }

    pub fn get(name: &str) -> String {
        let localized = if Self::is_turkish() { lookup(generated::TR, name) } else { None };
        localized.or_else(|| lookup(generated::EN, name)).map_or_else(|| name.to_string(), str::to_string)
    }

    /// Biçimli metin (C# `string.Format(Strings.X, args)`): `{0}`, `{1}`… yer tutucuları sırayla doldurulur.
    pub fn format(name: &str, args: &[&str]) -> String {
        format_placeholders(&Self::get(name), args)
    }
}

/// `{0}`/`{1:...}` yer tutucularını doldurur; biçim belirteci (iki noktadan sonrası) yok sayılır, `{{`/`}}` kaçışları
/// tek ayraç olur (C# composite format).
pub fn format_placeholders(template: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let mut spec = String::new();
                for d in chars.by_ref() {
                    if d == '}' {
                        break;
                    }
                    spec.push(d);
                }
                let index = spec.split([':', ',']).next().unwrap_or("").trim();
                match index.parse::<usize>().ok().and_then(|i| args.get(i)) {
                    Some(a) => out.push_str(a),
                    None => {
                        out.push('{');
                        out.push_str(&spec);
                        out.push('}');
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_key_and_formats() {
        Strings::set_ui_culture("en");
        assert_eq!(Strings::get("Cancel"), "Cancel");
        assert_eq!(Strings::get("NoSuchKey_123"), "NoSuchKey_123");
        Strings::set_ui_culture("tr");
        assert_ne!(Strings::get("Cancel"), "Cancel");
        assert_eq!(format_placeholders("{0} / {1:F0} {{x}}", &["a", "b"]), "a / b {x}");
    }

    #[test]
    fn tables_are_sorted_and_complete() {
        assert!(generated::EN.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(generated::TR.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(generated::EN.len() > 1000);
    }
}

/// Geçerli arayüz kültürü (C# `CultureInfo.CurrentCulture`: en-US ya da tr-TR).
pub fn culture() -> koy_core::clock::Culture {
    if Strings::is_turkish() { koy_core::clock::Culture::tr_tr() } else { koy_core::clock::Culture::en_us() }
}

/// Ayardaki dile göre kültür (C# `new CultureInfo(settings.General.Language == "en" ? "en-US" : "tr-TR")`).
pub fn culture_for(language: &str) -> koy_core::clock::Culture {
    if language == "en" { koy_core::clock::Culture::en_us() } else { koy_core::clock::Culture::tr_tr() }
}
