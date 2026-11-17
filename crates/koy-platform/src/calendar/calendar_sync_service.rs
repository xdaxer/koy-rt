//! Google Calendar API v3 ve Microsoft Graph Outlook Calendar için OAuth 2.0 PKCE + salt okunur eşitleme servisi.
//! Google için istemci sırrı token isteğine eklenir (`OAuthClient`); belirteçler DPAPI ile şifreli saklanır,
//! etkinlikler yerel önbellekte tutulur.
//!
//! Core sözleşmesi `BoxFuture` döndürür; gövde engelleyen HTTP ile yürür (gelecek, yoklandığı iş parçacığında çalışır).
//! Çağıran onu işçi iş parçacığında beklemelidir.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use koy_core::calendar::{
    BoxFuture, CalendarCacheSnapshot, CalendarCacheStore, CalendarEvent, CalendarEventParsers, CalendarEventSource,
    CalendarOAuthConfig, CalendarProvider, CalendarSyncResult, CalendarTokenStore, ICalendarSyncService, OAuthClient,
    OAuthConnectResult, OAuthPkce, OAuthTokenSet, escape_data_string,
};
use koy_core::cancel::CancellationToken;
use koy_core::time::{DateTimeOffset, TimeSpan};

use super::dpapi_token_protector::DpapiTokenProtector;
use super::loopback_oauth_listener::LoopbackOAuthListener;
use crate::net::{HttpClient, HttpError, Request};

fn http() -> &'static HttpClient {
    static CLIENT: std::sync::OnceLock<HttpClient> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| HttpClient::new(Duration::from_secs(15)))
}

fn utc_now() -> DateTimeOffset {
    chrono::Utc::now().fixed_offset()
}

/// C# `FormUrlEncodedContent`: `Uri.EscapeDataString`, boşluk `+`.
fn form_encode(form: &HashMap<String, String>) -> Vec<u8> {
    let enc = |s: &str| escape_data_string(s).replace("%20", "+");
    form.iter().map(|(k, v)| format!("{}={}", enc(k), enc(v))).collect::<Vec<_>>().join("&").into_bytes()
}

/// C# `ToString("yyyy-MM-ddTHH:mm:ssZ")`: `Z` düz harftir, saat değerin kendi farkıyla yazılır.
fn format_utc(value: DateTimeOffset) -> String {
    escape_data_string(&value.format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

/// Yakalanan hatalar (C#'ta `HttpRequestException` / `TaskCanceledException`); ileti `Exception.Message` yerine.
enum SyncError {
    Http(HttpError),
    /// C#'ta yakalanmayan JSON hatası; burada önbelleğe düşülür (sapma, bkz. `sync`).
    Parse(String),
}

impl From<HttpError> for SyncError {
    fn from(e: HttpError) -> Self {
        SyncError::Http(e)
    }
}

impl SyncError {
    fn message(&self) -> String {
        match self {
            SyncError::Http(e) => e.to_string(),
            SyncError::Parse(m) => m.clone(),
        }
    }
}

pub struct CalendarSyncService {
    token_store: CalendarTokenStore,
    cache_store: CalendarCacheStore,
}

impl Default for CalendarSyncService {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl CalendarSyncService {
    pub fn new(token_store: Option<CalendarTokenStore>, cache_store: Option<CalendarCacheStore>) -> Self {
        CalendarSyncService {
            token_store: token_store.unwrap_or_else(|| CalendarTokenStore::new(Arc::new(DpapiTokenProtector), None)),
            cache_store: cache_store.unwrap_or_else(|| CalendarCacheStore::new(None)),
        }
    }

    pub fn connect(&self, provider: CalendarProvider, client: &OAuthClient, ct: &CancellationToken) -> OAuthConnectResult {
        if !client.is_configured() {
            return OAuthConnectResult::new(false, "", Some("missing_client_id".into()));
        }

        let code_verifier = OAuthPkce::create_code_verifier();
        let code_challenge = OAuthPkce::compute_code_challenge(&code_verifier);
        let state = OAuthPkce::create_state();
        let port = LoopbackOAuthListener::allocate_free_port();
        // Google masaüstü istemcisi her loopback adresini kabul eder. Microsoft kayıtlı "http://localhost" adresinde
        // yalnız portu yok sayar; yol ve ana makine aynı olmalı, bu yüzden Outlook kök yolla localhost kullanır.
        let redirect_uri = if provider == CalendarProvider::Outlook {
            format!("http://localhost:{port}/")
        } else {
            format!("http://127.0.0.1:{port}/callback/")
        };

        let callback = LoopbackOAuthListener::authorize(
            provider,
            &client.client_id,
            &code_challenge,
            &state,
            &redirect_uri,
            TimeSpan::from_mins(3),
            ct,
        );

        let code = match (&callback.success, &callback.code) {
            (true, Some(c)) if !c.is_empty() => c.clone(),
            _ => return OAuthConnectResult::new(false, "", Some(callback.error.clone().unwrap_or_else(|| "auth_failed".into()))),
        };

        match exchange_code(provider, client, &code, &redirect_uri, &code_verifier, ct) {
            Ok(Some(token_set)) => {
                // C#: Save başarısızlığı istisna fırlatır; burada yazılamasa da bağlantı sonucu döner.
                let _ = self.token_store.save(provider, &token_set);
                OAuthConnectResult::new(true, token_set.account_label.clone(), None)
            }
            Ok(None) => OAuthConnectResult::new(false, "", Some("token_exchange_failed".into())),
            Err(e) => OAuthConnectResult::new(false, "", Some(e.message())),
        }
    }

    pub fn sync(&self, provider: CalendarProvider, client: &OAuthClient, from_utc: DateTimeOffset, to_utc: DateTimeOffset, ct: &CancellationToken) -> CalendarSyncResult {
        let source = CalendarOAuthConfig::to_event_source(provider);
        let Some(mut tokens) = self.token_store.load(provider) else {
            return self.load_cached_for_provider(source, "missing_token");
        };

        let result: Result<Vec<CalendarEvent>, SyncError> = (|| {
            let now = utc_now();
            if tokens.is_expired(now) {
                if tokens.refresh_token.trim().is_empty() || !client.is_configured() {
                    return Err(SyncError::Parse("token_expired".into()));
                }
                let Some(refreshed) = refresh_token(provider, client, &tokens, ct)? else {
                    return Err(SyncError::Parse("refresh_failed".into()));
                };
                tokens = refreshed;
                let _ = self.token_store.save(provider, &tokens);
            }

            if provider == CalendarProvider::Google {
                fetch_google_events(&tokens.access_token, from_utc, to_utc, ct)
            } else {
                fetch_outlook_events(&tokens.access_token, from_utc, to_utc, ct)
            }
        })();

        match result {
            Ok(events) => {
                self.update_cache(source, &events, utc_now());
                CalendarSyncResult::new(true, events, false, None)
            }
            Err(e) => self.load_cached_for_provider(source, &e.message()),
        }
    }

    fn load_cached_for_provider(&self, source: CalendarEventSource, error: &str) -> CalendarSyncResult {
        let snap = self.cache_store.load();
        let cached: Vec<CalendarEvent> = snap.events.into_iter().filter(|e| e.source == source).collect();
        CalendarSyncResult::new(false, cached, true, Some(error.to_string()))
    }

    fn update_cache(&self, source: CalendarEventSource, fresh: &[CalendarEvent], sync_utc: DateTimeOffset) {
        let current = self.cache_store.load();
        let mut merged: Vec<CalendarEvent> = current.events.into_iter().filter(|e| e.source != source).collect();
        merged.extend_from_slice(fresh);
        let _ = self.cache_store.save(&CalendarCacheSnapshot::new(Some(sync_utc), merged));
    }
}

fn post_token(provider: CalendarProvider, form: &HashMap<String, String>, ct: &CancellationToken) -> Result<Option<String>, SyncError> {
    let request = Request::post(CalendarOAuthConfig::get_token_endpoint(provider), "application/x-www-form-urlencoded", form_encode(form));
    let mut resp = http().send(&request, ct)?;
    if !resp.is_success() {
        return Ok(None);
    }
    Ok(Some(resp.text()?))
}

fn exchange_code(
    provider: CalendarProvider,
    client: &OAuthClient,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
    ct: &CancellationToken,
) -> Result<Option<OAuthTokenSet>, SyncError> {
    let mut form = HashMap::from([
        ("code".to_string(), code.to_string()),
        ("redirect_uri".to_string(), redirect_uri.to_string()),
        ("grant_type".to_string(), "authorization_code".to_string()),
        ("code_verifier".to_string(), code_verifier.to_string()),
    ]);
    client.add_to(&mut form);
    let Some(json) = post_token(provider, &form, ct)? else { return Ok(None) };
    CalendarEventParsers::parse_token_response(&json, utc_now(), "", "").map_err(|e| SyncError::Parse(format!("{e:?}")))
}

fn refresh_token(provider: CalendarProvider, client: &OAuthClient, current: &OAuthTokenSet, ct: &CancellationToken) -> Result<Option<OAuthTokenSet>, SyncError> {
    let mut form = HashMap::from([
        ("refresh_token".to_string(), current.refresh_token.clone()),
        ("grant_type".to_string(), "refresh_token".to_string()),
        ("scope".to_string(), CalendarOAuthConfig::get_read_only_scope(provider).to_string()),
    ]);
    client.add_to(&mut form);
    let Some(json) = post_token(provider, &form, ct)? else { return Ok(None) };
    CalendarEventParsers::parse_token_response(&json, utc_now(), &current.refresh_token, &current.account_label)
        .map_err(|e| SyncError::Parse(format!("{e:?}")))
}

fn get_events(url: &str, access_token: &str, extra: Option<(&str, &str)>, ct: &CancellationToken) -> Result<String, SyncError> {
    let mut request = Request::get(url).header("Authorization", &format!("Bearer {access_token}"));
    if let Some((k, v)) = extra {
        request = request.header(k, v);
    }
    let mut resp = http().send(&request, ct)?;
    // C# EnsureSuccessStatusCode: HttpRequestException.
    if !resp.is_success() {
        return Err(SyncError::Parse(format!("Response status code does not indicate success: {}.", resp.status)));
    }
    Ok(resp.text()?)
}

fn fetch_google_events(access_token: &str, from_utc: DateTimeOffset, to_utc: DateTimeOffset, ct: &CancellationToken) -> Result<Vec<CalendarEvent>, SyncError> {
    let url = format!(
        "https://www.googleapis.com/calendar/v3/calendars/primary/events?timeMin={}&timeMax={}&singleEvents=true&orderBy=startTime&maxResults=250",
        format_utc(from_utc),
        format_utc(to_utc)
    );
    let json = get_events(&url, access_token, None, ct)?;
    CalendarEventParsers::parse_google_events(&json, None).map_err(|e| SyncError::Parse(format!("{e:?}")))
}

fn fetch_outlook_events(access_token: &str, from_utc: DateTimeOffset, to_utc: DateTimeOffset, ct: &CancellationToken) -> Result<Vec<CalendarEvent>, SyncError> {
    let url = format!(
        "https://graph.microsoft.com/v1.0/me/calendarView?startDateTime={}&endDateTime={}&$top=250&$select=id,subject,start,end,isAllDay,isCancelled",
        format_utc(from_utc),
        format_utc(to_utc)
    );
    let json = get_events(&url, access_token, Some(("Prefer", "outlook.timezone=\"UTC\"")), ct)?;
    CalendarEventParsers::parse_outlook_events(&json, None).map_err(|e| SyncError::Parse(format!("{e:?}")))
}

impl ICalendarSyncService for CalendarSyncService {
    fn has_token(&self, provider: CalendarProvider) -> bool {
        self.token_store.exists(provider)
    }

    fn connect_async<'a>(&'a self, provider: CalendarProvider, client: &'a OAuthClient, ct: CancellationToken) -> BoxFuture<'a, OAuthConnectResult> {
        Box::pin(async move { self.connect(provider, client, &ct) })
    }

    fn disconnect_async<'a>(&'a self, provider: CalendarProvider, _ct: CancellationToken) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            self.token_store.delete(provider);
            let _ = self.cache_store.remove_provider(CalendarOAuthConfig::to_event_source(provider));
        })
    }

    fn sync_async<'a>(
        &'a self,
        provider: CalendarProvider,
        client: &'a OAuthClient,
        from_utc: DateTimeOffset,
        to_utc: DateTimeOffset,
        ct: CancellationToken,
    ) -> BoxFuture<'a, CalendarSyncResult> {
        Box::pin(async move { self.sync(provider, client, from_utc, to_utc, &ct) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_encoding_matches_form_url_encoded_content() {
        let form = HashMap::from([("redirect_uri".to_string(), "http://127.0.0.1:5/callback/".to_string())]);
        assert_eq!(String::from_utf8(form_encode(&form)).unwrap(), "redirect_uri=http%3A%2F%2F127.0.0.1%3A5%2Fcallback%2F");
        let form = HashMap::from([("scope".to_string(), "a b".to_string())]);
        assert_eq!(String::from_utf8(form_encode(&form)).unwrap(), "scope=a+b");
    }

    #[test]
    fn utc_format_has_literal_z() {
        let t = chrono::DateTime::parse_from_rfc3339("2026-11-06T09:05:07+00:00").unwrap();
        assert_eq!(format_utc(t), "2026-11-06T09%3A05%3A07Z");
    }
}
