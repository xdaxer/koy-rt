use std::collections::HashMap;
use std::fmt;


use sha2::{Digest, Sha256};

use super::calendar_event::CalendarEventSource;
use super::calendar_sync_contracts::CalendarSettings;
use crate::text;

/// Desteklenen harici takvim OAuth sağlayıcıları (salt okunur).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CalendarProvider {
    Google = 1,
    Outlook = 2,
}

impl CalendarProvider {
    /// `provider.ToString().ToLowerInvariant()`.
    pub fn lower_name(self) -> &'static str {
        match self {
            CalendarProvider::Google => "google",
            CalendarProvider::Outlook => "outlook",
        }
    }
}

/// OAuth 2.0 yetkilendirme akışı durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OAuthFlowState {
    #[default]
    Disconnected = 0,
    Authorizing = 1,
    ExchangingToken = 2,
    Connected = 3,
    Refreshing = 4,
    Error = 5,
    Cancelled = 6,
}

/// OAuth PKCE yetkilendirme geri dönüşünün doğrulanmış sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthCallbackResult {
    pub success: bool,
    pub code: Option<String>,
    pub error: Option<String>,
}

impl OAuthCallbackResult {
    pub fn ok(code: impl Into<String>) -> Self {
        OAuthCallbackResult { success: true, code: Some(code.into()), error: None }
    }

    pub fn fail(error: impl Into<String>) -> Self {
        OAuthCallbackResult { success: false, code: None, error: Some(error.into()) }
    }
}

/// Sağlayıcı başına OAuth 2.0 PKCE uç noktaları ve salt okunur kapsamlar.
/// PKCE akışı kullanılır. Depoda istemci kimliği ya da sırrı yoktur; sürümde CI gizli anahtarlarından gömülür.
pub struct CalendarOAuthConfig;

impl CalendarOAuthConfig {
    pub const GOOGLE_SCOPE: &'static str = "https://www.googleapis.com/auth/calendar.events.readonly";
    pub const OUTLOOK_SCOPE: &'static str = "Calendars.Read offline_access";

    pub const GOOGLE_CLIENT_ID_ENV_VAR: &'static str = "KOY_GOOGLE_CLIENT_ID";
    pub const GOOGLE_CLIENT_SECRET_ENV_VAR: &'static str = "KOY_GOOGLE_CLIENT_SECRET";
    pub const OUTLOOK_CLIENT_ID_ENV_VAR: &'static str = "KOY_OUTLOOK_CLIENT_ID";

    pub fn to_event_source(provider: CalendarProvider) -> CalendarEventSource {
        match provider {
            CalendarProvider::Google => CalendarEventSource::Google,
            CalendarProvider::Outlook => CalendarEventSource::Outlook,
        }
    }

    pub fn get_authorization_endpoint(provider: CalendarProvider) -> &'static str {
        match provider {
            CalendarProvider::Google => "https://accounts.google.com/o/oauth2/v2/auth",
            CalendarProvider::Outlook => "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
        }
    }

    pub fn get_token_endpoint(provider: CalendarProvider) -> &'static str {
        match provider {
            CalendarProvider::Google => "https://oauth2.googleapis.com/token",
            CalendarProvider::Outlook => "https://login.microsoftonline.com/common/oauth2/v2.0/token",
        }
    }

    pub fn get_read_only_scope(provider: CalendarProvider) -> &'static str {
        match provider {
            CalendarProvider::Google => Self::GOOGLE_SCOPE,
            CalendarProvider::Outlook => Self::OUTLOOK_SCOPE,
        }
    }

    /// Kullanılacak OAuth istemcisini seçer. Sıra: ayarlar → ortam değişkeni → sürüme gömülü yerleşik istemci.
    /// Gizli anahtar yalnız kendi kimliğiyle birlikte kullanılır: ayarlarda kimlik varsa ayarlardaki sır, ortam
    /// değişkeninde kimlik varsa ortamdaki sır, yoksa yerleşik sır. Hiçbiri yoksa boş istemci döner.
    ///
    /// `get_env` verilmezse `Environment.GetEnvironmentVariable` karşılığı `std::env::var` kullanılır.
    pub fn resolve_client(
        provider: CalendarProvider,
        settings: &CalendarSettings,
        built_in: Option<&OAuthBuiltInClients>,
        get_env: Option<&dyn Fn(&str) -> Option<String>>,
    ) -> OAuthClient {
        let default_env = |name: &str| std::env::var(name).ok();
        let get_env: &dyn Fn(&str) -> Option<String> = get_env.unwrap_or(&default_env);

        let (configured_id, configured_secret) = match provider {
            CalendarProvider::Google => (settings.google_client_id.as_str(), settings.google_client_secret.as_str()),
            CalendarProvider::Outlook => (settings.outlook_client_id.as_str(), ""),
        };
        if !text::is_null_or_white_space(Some(configured_id)) {
            return OAuthClient::new(configured_id.trim(), trim(Some(configured_secret)));
        }

        let (id_var, secret_var) = match provider {
            CalendarProvider::Google => (Self::GOOGLE_CLIENT_ID_ENV_VAR, Some(Self::GOOGLE_CLIENT_SECRET_ENV_VAR)),
            CalendarProvider::Outlook => (Self::OUTLOOK_CLIENT_ID_ENV_VAR, None),
        };
        let env_id = get_env(id_var);
        if !text::is_null_or_white_space(env_id.as_deref()) {
            let secret = match secret_var {
                None => String::new(),
                Some(var) => trim(get_env(var).as_deref()),
            };
            return OAuthClient::new(env_id.unwrap_or_default().trim(), secret);
        }

        match provider {
            CalendarProvider::Google => {
                OAuthClient::new(trim(built_in.map(|b| b.google_client_id.as_str())), trim(built_in.map(|b| b.google_client_secret.as_str())))
            }
            CalendarProvider::Outlook => OAuthClient::public(trim(built_in.map(|b| b.outlook_client_id.as_str()))),
        }
    }
}

fn trim(value: Option<&str>) -> String {
    if text::is_null_or_white_space(value) { String::new() } else { value.unwrap_or_default().trim().to_string() }
}

/// Token isteğinde kullanılan OAuth istemcisi. Google masaüstü istemcileri PKCE ile bile token uç noktasında
/// `client_secret` ister (Google bu sırrı gizli saymaz); Outlook genel istemcisinde sır boştur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthClient {
    pub client_id: String,
    pub client_secret: String,
}

impl OAuthClient {
    pub fn new(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        OAuthClient { client_id: client_id.into(), client_secret: client_secret.into() }
    }

    /// `new OAuthClient(id)`: sır boş (genel istemci).
    pub fn public(client_id: impl Into<String>) -> Self {
        Self::new(client_id, "")
    }

    pub fn is_configured(&self) -> bool {
        !text::is_null_or_white_space(Some(&self.client_id))
    }

    /// Token isteği formuna istemci alanlarını ekler; sır boşsa `client_secret` gönderilmez.
    pub fn add_to(&self, form: &mut HashMap<String, String>) {
        form.insert("client_id".to_string(), self.client_id.clone());
        if !text::is_null_or_white_space(Some(&self.client_secret)) {
            form.insert("client_secret".to_string(), self.client_secret.clone());
        }
    }
}

/// Sürüm derlenirken CI gizli anahtarlarından gömülen yerleşik istemciler (yerel derlemede boş).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OAuthBuiltInClients {
    pub google_client_id: String,
    pub google_client_secret: String,
    pub outlook_client_id: String,
}

impl OAuthBuiltInClients {
    pub fn new(google_client_id: impl Into<String>, google_client_secret: impl Into<String>, outlook_client_id: impl Into<String>) -> Self {
        OAuthBuiltInClients {
            google_client_id: google_client_id.into(),
            google_client_secret: google_client_secret.into(),
            outlook_client_id: outlook_client_id.into(),
        }
    }
}

/// `System.Uri` için asgari karşılık: yalnız mutlak `şema://...` adresi ve `Query` özelliği (OAuth geri dönüşü için).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uri {
    original: String,
}

/// `UriFormatException`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UriFormatError;

impl fmt::Display for UriFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Invalid URI")
    }
}

impl std::error::Error for UriFormatError {}

impl Uri {
    pub fn new(uri: &str) -> Result<Uri, UriFormatError> {
        let scheme_end = uri.find("://").ok_or(UriFormatError)?;
        let scheme = &uri[..scheme_end];
        let mut chars = scheme.chars();
        let valid_scheme = chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
        if !valid_scheme || uri[scheme_end + 3..].is_empty() {
            return Err(UriFormatError);
        }
        Ok(Uri { original: uri.to_string() })
    }

    pub fn original_string(&self) -> &str {
        &self.original
    }

    /// `Uri.Query`: "?a=b" (baştaki `?` dahil); sorgu yoksa ya da boşsa "". Parça (`#...`) sorguya girmez.
    pub fn query(&self) -> String {
        let without_fragment = self.original.split('#').next().unwrap_or_default();
        match without_fragment.split_once('?') {
            Some((_, query)) if !query.is_empty() => format!("?{query}"),
            _ => String::new(),
        }
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.original)
    }
}

/// `Uri.EscapeDataString`: RFC 3986 ayrılmamış karakterler (`A-Za-z0-9-._~`) dışındakiler `%XX` olur.
pub fn escape_data_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `Uri.UnescapeDataString`: geçerli `%XX` dizilerini çözer; geçersiz UTF-8 baytları ve bozuk diziler olduğu gibi kalır.
pub fn unescape_data_string(value: &str) -> String {
    let bytes = value.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16);
    let mut out = String::with_capacity(value.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            // Art arda gelen kaçışları tek bayt dizisi olarak topla, UTF-8 olarak çöz.
            let mut run: Vec<u8> = Vec::new();
            let mut j = i;
            while j + 2 < bytes.len() && bytes[j] == b'%' {
                match (hex(bytes[j + 1]), hex(bytes[j + 2])) {
                    (Some(h), Some(l)) => {
                        run.push((h * 16 + l) as u8);
                        j += 3;
                    }
                    _ => break,
                }
            }
            if run.is_empty() {
                out.push('%');
                i += 1;
            } else {
                for chunk in run.utf8_chunks() {
                    out.push_str(chunk.valid());
                    for b in chunk.invalid() {
                        out.push_str(&format!("%{b:02X}"));
                    }
                }
                i = j;
            }
        } else {
            // Kaçışsız parçayı olduğu gibi (karakter sınırlarına dikkat ederek) kopyala.
            let ch = value[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// `RandomNumberGenerator.GetBytes` (kaynak: `crate::random`, Platform `BCryptGenRandom` kurar).
pub(crate) fn random_bytes(len: usize) -> Vec<u8> {
    crate::random::get_bytes(len)
}

/// RFC 7636 PKCE (S256) üretimi, yetkilendirme URL'si oluşturma ve loopback callback doğrulama.
pub struct OAuthPkce;

impl OAuthPkce {
    /// `CreateCodeVerifier()`: 32 bayt.
    pub fn create_code_verifier() -> String {
        Self::create_code_verifier_with_length(32)
    }

    /// `CreateCodeVerifier(byteLength)`; 32 bayttan azı `ArgumentOutOfRangeException` (burada panik).
    pub fn create_code_verifier_with_length(byte_length: usize) -> String {
        if byte_length < 32 {
            panic!("PKCE code_verifier en az 32 bayt olmalıdır.");
        }

        Self::base64_url_encode(&random_bytes(byte_length))
    }

    pub fn compute_code_challenge(code_verifier: &str) -> String {
        if text::is_null_or_white_space(Some(code_verifier)) {
            panic!("codeVerifier boş olamaz.");
        }

        // Encoding.ASCII: ASCII dışı karakterler '?' olur.
        let ascii: Vec<u8> = code_verifier.chars().map(|c| if c.is_ascii() { c as u8 } else { b'?' }).collect();
        let hash = Sha256::digest(&ascii);
        Self::base64_url_encode(&hash)
    }

    pub fn create_state() -> String {
        Self::base64_url_encode(&random_bytes(16))
    }

    /// Dolgusuz base64url (RFC 4648 §5).
    pub fn base64_url_encode(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let b0 = u32::from(chunk[0]);
            let b1 = chunk.get(1).map_or(0, |&b| u32::from(b));
            let b2 = chunk.get(2).map_or(0, |&b| u32::from(b));
            let n = (b0 << 16) | (b1 << 8) | b2;
            out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
            out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
            if chunk.len() > 1 {
                out.push(ALPHABET[(n >> 6 & 63) as usize] as char);
            }
            if chunk.len() > 2 {
                out.push(ALPHABET[(n & 63) as usize] as char);
            }
        }
        out
    }

    pub fn build_authorization_url(provider: CalendarProvider, client_id: &str, redirect_uri: &str, code_challenge: &str, state: &str) -> Uri {
        if text::is_null_or_white_space(Some(client_id)) {
            panic!("clientId boş olamaz.");
        }
        if text::is_null_or_white_space(Some(redirect_uri)) {
            panic!("redirectUri boş olamaz.");
        }

        let endpoint = CalendarOAuthConfig::get_authorization_endpoint(provider);
        let scope = CalendarOAuthConfig::get_read_only_scope(provider);

        let mut query = vec![
            format!("client_id={}", escape_data_string(client_id)),
            format!("redirect_uri={}", escape_data_string(redirect_uri)),
            "response_type=code".to_string(),
            format!("scope={}", escape_data_string(scope)),
            format!("code_challenge={}", escape_data_string(code_challenge)),
            "code_challenge_method=S256".to_string(),
            format!("state={}", escape_data_string(state)),
        ];

        if provider == CalendarProvider::Google {
            query.push("access_type=offline".to_string());
            query.push("prompt=consent".to_string());
        }

        Uri::new(&format!("{endpoint}?{}", query.join("&"))).expect("uç nokta geçerli bir adres")
    }

    pub fn validate_callback(callback_uri: Option<&Uri>, expected_state: &str) -> OAuthCallbackResult {
        let Some(callback_uri) = callback_uri else {
            return OAuthCallbackResult::fail("missing_callback");
        };

        let query = Self::parse_query_string(&callback_uri.query());
        if let Some(error) = query.get("error") {
            if !text::is_null_or_white_space(Some(error)) {
                return OAuthCallbackResult::fail(error.clone());
            }
        }

        match query.get("state") {
            Some(actual) if actual == expected_state => {}
            _ => return OAuthCallbackResult::fail("state_mismatch"),
        }

        match query.get("code") {
            Some(code) if !text::is_null_or_white_space(Some(code)) => OAuthCallbackResult::ok(code.clone()),
            _ => OAuthCallbackResult::fail("missing_code"),
        }
    }

    pub fn parse_query_string(query: &str) -> HashMap<String, String> {
        let mut result = HashMap::new();
        if query.is_empty() {
            return result;
        }

        let trimmed = query.strip_prefix('?').unwrap_or(query);
        for pair in trimmed.split('&').filter(|p| !p.is_empty()) {
            match pair.find('=') {
                None => {
                    result.insert(unescape_data_string(pair), String::new());
                }
                Some(eq) => {
                    let key = unescape_data_string(&pair[..eq].replace('+', " "));
                    let val = unescape_data_string(&pair[eq + 1..].replace('+', " "));
                    result.insert(key, val);
                }
            }
        }

        result
    }
}

/// OAuth oturum akışının saf durum makinesi.
#[derive(Debug, Clone, Default)]
pub struct OAuthSessionState {
    state: OAuthFlowState,
    error_code: Option<String>,
}

impl OAuthSessionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> OAuthFlowState {
        self.state
    }

    pub fn error_code(&self) -> Option<&str> {
        self.error_code.as_deref()
    }

    pub fn is_busy(&self) -> bool {
        matches!(self.state, OAuthFlowState::Authorizing | OAuthFlowState::ExchangingToken | OAuthFlowState::Refreshing)
    }

    pub fn set_initial_connected(&mut self, connected: bool) {
        self.state = if connected { OAuthFlowState::Connected } else { OAuthFlowState::Disconnected };
        self.error_code = None;
    }

    pub fn try_begin_authorize(&mut self) -> bool {
        if self.is_busy() {
            return false;
        }

        self.state = OAuthFlowState::Authorizing;
        self.error_code = None;
        true
    }

    pub fn begin_token_exchange(&mut self) {
        if self.state == OAuthFlowState::Authorizing {
            self.state = OAuthFlowState::ExchangingToken;
        }
    }

    pub fn begin_refresh(&mut self) {
        if !self.is_busy() {
            self.state = OAuthFlowState::Refreshing;
            self.error_code = None;
        }
    }

    pub fn complete_connected(&mut self) {
        self.state = OAuthFlowState::Connected;
        self.error_code = None;
    }

    pub fn cancel(&mut self) {
        self.state = OAuthFlowState::Cancelled;
        self.error_code = None;
    }

    pub fn fail(&mut self, error_code: impl Into<String>) {
        self.state = OAuthFlowState::Error;
        self.error_code = Some(error_code.into());
    }

    pub fn disconnect(&mut self) {
        self.state = OAuthFlowState::Disconnected;
        self.error_code = None;
    }
}
