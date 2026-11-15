//! Hafif çevrimiçi sözlük servisi (İngilizce için `dictionaryapi.dev`, Türkçe için TDK `sozluk.gov.tr`).
//! Yalnız kullanıcı sorgu yazdığında tekil istek atar; boşta sıfır ağ ve CPU kullanır.

use std::sync::OnceLock;
use std::time::Duration;

use koy_core::calendar::escape_data_string;
use koy_core::cancel::{CancellationToken, OperationCanceled};
use koy_core::search::{DictionaryEntry, DictionaryMeaning, IDictionaryService, ServiceError};
use serde_json::Value;

use crate::net::{HttpClient, HttpError};

const TURKISH_SPECIFIC_CHARS: &str = "çğıİöşüÇĞÖŞÜ";
const MAX_MEANINGS: usize = 6;

fn client() -> &'static HttpClient {
    static CLIENT: OnceLock<HttpClient> = OnceLock::new();
    CLIENT.get_or_init(|| HttpClient::new(Duration::from_secs(4)))
}

#[derive(Default)]
pub struct HttpDictionaryService;

/// `TryGetProperty(name, out v) ? v.GetString() : null` (dize değilse C# istisna atar → JSON hatası sayılır).
fn str_prop<'a>(v: &'a Value, name: &str) -> Result<Option<&'a str>, ()> {
    match v.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(()),
    }
}

fn first_of_array(json: &str) -> Option<Value> {
    let root: Value = serde_json::from_str(json).ok()?;
    match root {
        Value::Array(mut items) if !items.is_empty() => Some(items.swap_remove(0)),
        _ => None,
    }
}

fn is_blank(s: Option<&str>) -> bool {
    koy_core::text::is_null_or_white_space(s)
}

impl HttpDictionaryService {
    fn try_get_string(url: &str, ct: &CancellationToken) -> Result<Option<String>, OperationCanceled> {
        match client().get(url, ct) {
            Ok(mut r) if r.is_success() => Ok(r.text().ok()),
            Ok(_) => Ok(None),
            // C#: TaskCanceledException (zaman aşımı ve iptal) yutulur → null.
            Err(HttpError::Canceled) | Err(_) => Ok(None),
        }
    }

    fn lookup_english(word: &str, ct: &CancellationToken) -> Result<Option<DictionaryEntry>, OperationCanceled> {
        let escaped = escape_data_string(word);
        let Some(json) = Self::try_get_string(&format!("https://api.dictionaryapi.dev/api/v2/entries/en/{escaped}"), ct)? else {
            return Ok(None);
        };
        Ok(Self::parse_english(&json, word, &escaped))
    }

    fn parse_english(json: &str, word: &str, escaped: &str) -> Option<DictionaryEntry> {
        let first = first_of_array(json)?;
        let parse = || -> Result<Option<DictionaryEntry>, ()> {
            let entry_word = str_prop(&first, "word")?.unwrap_or(word).to_string();
            let phonetic = str_prop(&first, "phonetic")?.map(str::to_string);
            let mut meanings = Vec::new();
            if let Some(Value::Array(list)) = first.get("meanings") {
                'outer: for m in list {
                    let pos = str_prop(m, "partOfSpeech")?.unwrap_or("").to_string();
                    let Some(Value::Array(defs)) = m.get("definitions") else { continue };
                    for d in defs {
                        let def = str_prop(d, "definition")?;
                        if is_blank(def) {
                            continue;
                        }
                        let example = str_prop(d, "example")?.map(str::to_string);
                        meanings.push(DictionaryMeaning::new(pos.clone(), def.unwrap_or_default(), example));
                        if meanings.len() >= MAX_MEANINGS {
                            break 'outer;
                        }
                    }
                }
            }
            Ok((!meanings.is_empty()).then(|| {
                DictionaryEntry::new(entry_word, phonetic, meanings, Some(format!("https://www.merriam-webster.com/dictionary/{escaped}")))
            }))
        };
        parse().ok().flatten()
    }

    fn lookup_tdk(word: &str, ct: &CancellationToken) -> Result<Option<DictionaryEntry>, OperationCanceled> {
        let escaped = escape_data_string(word);
        let Some(json) = Self::try_get_string(&format!("https://sozluk.gov.tr/gts?ara={escaped}"), ct)? else {
            return Ok(None);
        };
        Ok(Self::parse_tdk(&json, word, &escaped))
    }

    fn parse_tdk(json: &str, word: &str, escaped: &str) -> Option<DictionaryEntry> {
        let first = first_of_array(json)?;
        let Some(Value::Array(list)) = first.get("anlamlarListe") else { return None };
        let parse = || -> Result<Option<DictionaryEntry>, ()> {
            let entry_word = str_prop(&first, "madde")?.unwrap_or(word).to_string();
            let mut meanings = Vec::new();
            for item in list {
                let def = str_prop(item, "anlam")?;
                if is_blank(def) {
                    continue;
                }
                let mut pos = String::new();
                if let Some(Value::Array(oz)) = item.get("ozelliklerListe")
                    && let Some(f) = oz.first()
                {
                    pos = str_prop(f, "tam_adi")?.unwrap_or("").to_string();
                }
                let mut example = None;
                if let Some(Value::Array(or)) = item.get("orneklerListe")
                    && let Some(f) = or.first()
                {
                    example = str_prop(f, "ornek")?.map(str::to_string);
                }
                meanings.push(DictionaryMeaning::new(pos, def.unwrap_or_default(), example));
                if meanings.len() >= MAX_MEANINGS {
                    break;
                }
            }
            Ok((!meanings.is_empty()).then(|| DictionaryEntry::new(entry_word, None, meanings, Some(format!("https://sozluk.gov.tr/?ara={escaped}")))))
        };
        parse().ok().flatten()
    }
}

impl IDictionaryService for HttpDictionaryService {
    fn lookup(&self, word: &str, ct: &CancellationToken) -> Result<Option<DictionaryEntry>, ServiceError> {
        let trimmed = word.trim();
        let len = trimmed.encode_utf16().count();
        if len == 0 || len > 64 {
            return Ok(None);
        }

        let is_turkish_ui = koy_core::text::eq_ignore_case(&crate::locale::current_ui_two_letter(), "tr");
        let has_turkish_chars = trimmed.chars().any(|c| TURKISH_SPECIFIC_CHARS.contains(c));
        if has_turkish_chars || is_turkish_ui {
            let tdk = Self::lookup_tdk(trimmed, ct)?;
            if tdk.is_some() || has_turkish_chars {
                return Ok(tdk);
            }
        }
        Ok(Self::lookup_english(trimmed, ct)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_english_api_response() {
        let json = r#"[{"word":"hello","phonetic":"/həˈloʊ/","meanings":[{"partOfSpeech":"noun","definitions":[{"definition":" "},{"definition":"A greeting.","example":"She said hello."}]}]}]"#;
        let e = HttpDictionaryService::parse_english(json, "hello", "hello").unwrap();
        assert_eq!(e.word, "hello");
        assert_eq!(e.meanings.len(), 1);
        assert_eq!(e.meanings[0].example.as_deref(), Some("She said hello."));
        assert_eq!(e.source_url.as_deref(), Some("https://www.merriam-webster.com/dictionary/hello"));
        assert!(HttpDictionaryService::parse_english("{}", "x", "x").is_none());
        assert!(HttpDictionaryService::parse_english(r#"[{"word":5}]"#, "x", "x").is_none());
    }

    #[test]
    fn parses_tdk_response() {
        let json = r#"[{"madde":"elma","anlamlarListe":[{"anlam":"Gülgillerden bir ağaç","ozelliklerListe":[{"tam_adi":"isim"}],"orneklerListe":[{"ornek":"Elma ağacı"}]}]}]"#;
        let e = HttpDictionaryService::parse_tdk(json, "elma", "elma").unwrap();
        assert_eq!((e.word.as_str(), e.meanings[0].part_of_speech.as_str()), ("elma", "isim"));
        assert_eq!(e.meanings[0].example.as_deref(), Some("Elma ağacı"));
        assert!(HttpDictionaryService::parse_tdk(r#"{"error":"Sonuç bulunamadı"}"#, "x", "x").is_none());
    }
}
