//! LRCLIB çevrim içi kaynağı. Önce tam eşleşme, bulunamazsa arama denenir.

use koy_core::cancel::{CancellationToken, OperationCanceled};
use koy_core::lyrics::{ILyricsSource, LrclibApi, LyricsError, LyricsLookup, LyricsQuery};

use crate::net::{HttpClient, HttpError};

pub struct LrclibSource {
    http: &'static HttpClient,
}

impl Default for LrclibSource {
    fn default() -> Self {
        Self::new(None)
    }
}

impl LrclibSource {
    pub fn new(http: Option<&'static HttpClient>) -> Self {
        LrclibSource { http: http.unwrap_or_else(HttpClient::shared) }
    }

    fn lookup(&self, query: &LyricsQuery, ct: &CancellationToken) -> Result<LyricsLookup, HttpError> {
        {
            let mut exact = self.http.get(&LrclibApi::get_url(query), ct)?;
            if exact.is_success() {
                let json = exact.text()?;
                if let Some(synced) = LrclibApi::parse_synced(&json) {
                    return Ok(LyricsLookup::found(synced));
                }
            } else if exact.status != 404 {
                return Ok(LyricsLookup::failed());
            }
        }

        let mut search = self.http.get(&LrclibApi::search_url(query), ct)?;
        if !search.is_success() {
            return Ok(LyricsLookup::failed());
        }
        let results = search.text()?;
        Ok(match LrclibApi::pick_best(&results, query) {
            Some(best) => LyricsLookup::found(best),
            None => LyricsLookup::not_found(),
        })
    }
}

impl ILyricsSource for LrclibSource {
    fn find(&self, query: &LyricsQuery, ct: &CancellationToken) -> Result<LyricsLookup, LyricsError> {
        // Sanatçısız sorgu hizmette zorunlu alanı boş bırakır ve başka şarkının sözünü getirme riski taşır.
        if query.artist.is_empty() {
            return Ok(LyricsLookup::not_found());
        }
        match self.lookup(query, ct) {
            Ok(r) => Ok(r),
            // Bizim iptalimiz yukarı çıkar; istemci zaman aşımı ve ağ hatası "başarısız" sayılır.
            Err(HttpError::Canceled) => Err(Box::new(OperationCanceled)),
            Err(_) => Ok(LyricsLookup::failed()),
        }
    }
}
