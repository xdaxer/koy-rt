//! WinHTTP üzerinde küçük, engelleyen HTTP istemcisi (C# `HttpClient` karşılığı; yeni crate yok).
//!
//! - Sistem vekil sunucusu kullanılır (`WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY`), yönlendirmeler izlenir (HttpClient gibi).
//! - Zaman aşımı bağlantı, gönderme ve yanıt başlıklarını kapsar (C# `HttpClient.Timeout` başlıklara kadar sayar);
//!   gövdenin tamamı için ayrı süre sınırı çağıranın işidir ([`Response::read_to_end_limited`]'e `deadline`).
//! - İptal: `CancellationToken` her okuma adımında denetlenir. Engelleyen WinHTTP çağrısı sürerken iptal, en geç
//!   zaman aşımında etkili olur.

use std::io::{self, Read};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use koy_core::cancel::CancellationToken;
use windows::Win32::Foundation::GetLastError;
use windows::Win32::Networking::WinHttp::{
    URL_COMPONENTS, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_ADDREQ_FLAG_ADD, WINHTTP_FLAG_SECURE,
    WINHTTP_OPEN_REQUEST_FLAGS, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE, WinHttpAddRequestHeaders,
    WinHttpCloseHandle, WinHttpConnect, WinHttpCrackUrl, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts,
};
use windows::core::{HSTRING, PCWSTR};

/// Koy'un User-Agent'ı (C# `AppHttp` ile aynı).
pub const USER_AGENT: &str = "Koy (+https://github.com/koyhq/koy)";

/// `ERROR_WINHTTP_TIMEOUT`.
const ERROR_WINHTTP_TIMEOUT: u32 = 12002;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// Adres çözülemedi ya da http/https değil.
    InvalidUrl,
    /// Zaman aşımı (C#: istemci zaman aşımında `TaskCanceledException`).
    Timeout,
    /// Çağıranın iptali (C# `OperationCanceledException`).
    Canceled,
    /// Gövde izin verilen boyutu aştı.
    TooLarge,
    /// Diğer ağ hataları (C# `HttpRequestException`); Win32 hata kodu.
    Network(u32),
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpError::InvalidUrl => f.write_str("invalid url"),
            HttpError::Timeout => f.write_str("request timed out"),
            HttpError::Canceled => f.write_str("request canceled"),
            HttpError::TooLarge => f.write_str("response too large"),
            HttpError::Network(code) => write!(f, "network error {code}"),
        }
    }
}

impl std::error::Error for HttpError {}

fn last_error() -> HttpError {
    // SAFETY: yalnız iş parçacığının son hata kodunu okur.
    let code = unsafe { GetLastError() }.0;
    if code == ERROR_WINHTTP_TIMEOUT { HttpError::Timeout } else { HttpError::Network(code) }
}

/// WinHTTP tutamacı; bırakılınca kapanır.
struct Handle(*mut core::ffi::c_void);

// SAFETY: WinHTTP tutamaçları iş parçacıkları arasında kullanılabilir (eşzamanlı çağrı yapılmaz).
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: tutamaç bizim ve bir kez kapatılır.
            let _ = unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

/// HTTP istemcisi. Oturum ilk istekte açılır (C#: açılışta hiçbir bağlantı açılmaz).
pub struct HttpClient {
    timeout: Duration,
    user_agent: String,
    session: OnceLock<Option<Handle>>,
}

impl HttpClient {
    pub fn new(timeout: Duration) -> Self {
        HttpClient { timeout, user_agent: USER_AGENT.to_string(), session: OnceLock::new() }
    }

    /// Uygulamanın paylaşılan istemcisi (C# `AppHttp.Client`: 10 sn zaman aşımı).
    pub fn shared() -> &'static HttpClient {
        static CLIENT: OnceLock<HttpClient> = OnceLock::new();
        CLIENT.get_or_init(|| HttpClient::new(Duration::from_secs(10)))
    }

    fn session(&self) -> Result<&Handle, HttpError> {
        self.session
            .get_or_init(|| {
                let agent = HSTRING::from(self.user_agent.as_str());
                // SAFETY: ad geçerli HSTRING; vekil parametreleri boş.
                let h = unsafe { WinHttpOpen(&agent, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0) };
                if h.is_null() { None } else { Some(Handle(h)) }
            })
            .as_ref()
            .ok_or(HttpError::Network(0))
    }

    pub fn get(&self, url: &str, ct: &CancellationToken) -> Result<Response, HttpError> {
        self.send(&Request::get(url), ct)
    }

    /// İsteği gönderir ve yanıt başlıklarını bekler. Gövde [`Response`] üzerinden okunur.
    pub fn send(&self, request: &Request, ct: &CancellationToken) -> Result<Response, HttpError> {
        if ct.is_cancellation_requested() {
            return Err(HttpError::Canceled);
        }
        let parts = UrlParts::parse(&request.url).ok_or(HttpError::InvalidUrl)?;
        let session = self.session()?;

        let host = HSTRING::from(parts.host.as_str());
        // SAFETY: oturum açık, ana makine adı geçerli.
        let connect = unsafe { WinHttpConnect(session.0, &host, parts.port, 0) };
        if connect.is_null() {
            return Err(last_error());
        }
        let connect = Handle(connect);

        let verb = HSTRING::from(request.method.as_str());
        let path = HSTRING::from(parts.path_and_query.as_str());
        let flags = if parts.secure { WINHTTP_FLAG_SECURE } else { WINHTTP_OPEN_REQUEST_FLAGS(0) };
        // SAFETY: bağlantı açık; sürüm/yönlendiren/kabul türleri varsayılan (boş).
        let req = unsafe { WinHttpOpenRequest(connect.0, &verb, &path, PCWSTR::null(), PCWSTR::null(), std::ptr::null(), flags) };
        if req.is_null() {
            return Err(last_error());
        }
        let req = Handle(req);

        let ms = self.timeout.as_millis().min(i32::MAX as u128) as i32;
        // SAFETY: istek tutamacı açık.
        unsafe { WinHttpSetTimeouts(req.0, ms, ms, ms, ms) }.map_err(|_| last_error())?;

        for (name, value) in &request.headers {
            let line: Vec<u16> = format!("{name}: {value}").encode_utf16().collect();
            // SAFETY: başlık satırı geçerli UTF-16 dilimi.
            unsafe { WinHttpAddRequestHeaders(req.0, &line, WINHTTP_ADDREQ_FLAG_ADD) }.map_err(|_| last_error())?;
        }

        let body = request.body.as_deref().unwrap_or(&[]);
        // SAFETY: gövde arabelleği çağrı süresince yaşar; başlıklar ayrıca eklendi.
        unsafe {
            WinHttpSendRequest(
                req.0,
                None,
                if body.is_empty() { None } else { Some(body.as_ptr() as *const _) },
                body.len() as u32,
                body.len() as u32,
                0,
            )
        }
        .map_err(|_| last_error())?;
        if ct.is_cancellation_requested() {
            return Err(HttpError::Canceled);
        }
        // SAFETY: istek gönderildi.
        unsafe { WinHttpReceiveResponse(req.0, std::ptr::null_mut()) }.map_err(|_| last_error())?;

        let mut status: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        // SAFETY: sayısal başlık u32'ye yazılır.
        unsafe {
            WinHttpQueryHeaders(
                req.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some(&mut status as *mut u32 as *mut _),
                &mut size,
                std::ptr::null_mut(),
            )
        }
        .map_err(|_| last_error())?;

        Ok(Response { status: status as u16, request: req, _connect: connect, ct: ct.clone() })
    }
}

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

impl Request {
    pub fn get(url: &str) -> Self {
        Request { method: "GET".into(), url: url.into(), headers: Vec::new(), body: None }
    }

    pub fn post(url: &str, content_type: &str, body: Vec<u8>) -> Self {
        Request {
            method: "POST".into(),
            url: url.into(),
            headers: vec![("Content-Type".into(), content_type.into())],
            body: Some(body),
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// Yanıt: durum kodu ve akış halinde okunan gövde (`Read`).
pub struct Response {
    pub status: u16,
    request: Handle,
    _connect: Handle,
    ct: CancellationToken,
}

impl Response {
    /// C# `IsSuccessStatusCode`: 200–299.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Gövdenin tamamı. `max_bytes` aşılırsa `TooLarge`; `deadline` geçerse `Timeout`; `progress` okunan bayt sayısını alır.
    pub fn read_to_end_limited(
        &mut self,
        max_bytes: Option<usize>,
        deadline: Option<Instant>,
        mut progress: Option<&mut dyn FnMut(usize)>,
    ) -> Result<Vec<u8>, HttpError> {
        let mut out = Vec::new();
        let mut buf = [0u8; 16 * 1024];
        loop {
            if deadline.is_some_and(|d| Instant::now() >= d) {
                return Err(HttpError::Timeout);
            }
            let n = self.read_chunk(&mut buf)?;
            if n == 0 {
                return Ok(out);
            }
            out.extend_from_slice(&buf[..n]);
            if max_bytes.is_some_and(|m| out.len() > m) {
                return Err(HttpError::TooLarge);
            }
            if let Some(p) = progress.as_deref_mut() {
                p(out.len());
            }
        }
    }

    /// Gövdeyi UTF-8 metin olarak okur (C# `ReadAsStringAsync`; BOM atılır, geçersiz bayt U+FFFD olur).
    pub fn text(&mut self) -> Result<String, HttpError> {
        let bytes = self.read_to_end_limited(None, None, None)?;
        let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&bytes);
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }

    fn read_chunk(&mut self, buf: &mut [u8]) -> Result<usize, HttpError> {
        if self.ct.is_cancellation_requested() {
            return Err(HttpError::Canceled);
        }
        let mut available: u32 = 0;
        // SAFETY: istek tutamacı açık.
        unsafe { WinHttpQueryDataAvailable(self.request.0, &mut available) }.map_err(|_| last_error())?;
        if available == 0 {
            return Ok(0);
        }
        let want = (available as usize).min(buf.len());
        let mut read: u32 = 0;
        // SAFETY: arabellek `want` bayt yazılabilir.
        unsafe { WinHttpReadData(self.request.0, buf.as_mut_ptr() as *mut _, want as u32, &mut read) }.map_err(|_| last_error())?;
        Ok(read as usize)
    }
}

impl Read for Response {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_chunk(buf).map_err(io::Error::other)
    }
}

/// `WinHttpCrackUrl` ile ayrıştırılmış adres.
#[derive(Debug, Clone, PartialEq, Eq)]
struct UrlParts {
    secure: bool,
    host: String,
    port: u16,
    path_and_query: String,
}

impl UrlParts {
    fn parse(url: &str) -> Option<UrlParts> {
        let wide: Vec<u16> = url.encode_utf16().collect();
        if wide.is_empty() {
            return None;
        }
        let mut host = vec![0u16; 256];
        let mut path = vec![0u16; wide.len() + 1];
        let mut extra = vec![0u16; wide.len() + 1];
        let mut parts = URL_COMPONENTS {
            dwStructSize: std::mem::size_of::<URL_COMPONENTS>() as u32,
            lpszHostName: windows::core::PWSTR(host.as_mut_ptr()),
            dwHostNameLength: host.len() as u32,
            lpszUrlPath: windows::core::PWSTR(path.as_mut_ptr()),
            dwUrlPathLength: path.len() as u32,
            lpszExtraInfo: windows::core::PWSTR(extra.as_mut_ptr()),
            dwExtraInfoLength: extra.len() as u32,
            ..Default::default()
        };
        // SAFETY: arabellekler yeterli boyutta ve çağrı süresince yaşar.
        unsafe { WinHttpCrackUrl(&wide, 0, &mut parts) }.ok()?;
        let scheme = parts.nScheme.0;
        // INTERNET_SCHEME_HTTP = 1, INTERNET_SCHEME_HTTPS = 2.
        if scheme != 1 && scheme != 2 {
            return None;
        }
        let host = String::from_utf16_lossy(&host[..parts.dwHostNameLength as usize]);
        if host.is_empty() {
            return None;
        }
        let mut path_and_query = String::from_utf16_lossy(&path[..parts.dwUrlPathLength as usize]);
        path_and_query.push_str(&String::from_utf16_lossy(&extra[..parts.dwExtraInfoLength as usize]));
        if path_and_query.is_empty() {
            path_and_query.push('/');
        }
        Some(UrlParts { secure: scheme == 2, host, port: parts.nPort, path_and_query })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    #[test]
    fn parses_urls() {
        let p = UrlParts::parse("https://lrclib.net/api/get?artist_name=a%20b&x=1").unwrap();
        assert_eq!(p, UrlParts { secure: true, host: "lrclib.net".into(), port: 443, path_and_query: "/api/get?artist_name=a%20b&x=1".into() });
        let p = UrlParts::parse("http://127.0.0.1:8123").unwrap();
        assert_eq!((p.secure, p.port, p.path_and_query.as_str()), (false, 8123, "/"));
        assert!(UrlParts::parse("ftp://example.com/x").is_none());
        assert!(UrlParts::parse("not a url").is_none());
    }

    fn serve_once(response: &'static [u8]) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            s.write_all(response).unwrap();
        });
        port
    }

    #[test]
    fn get_reads_status_and_body_from_local_server() {
        let port = serve_once(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello");
        let client = HttpClient::new(Duration::from_secs(5));
        let mut r = client.get(&format!("http://127.0.0.1:{port}/x"), &CancellationToken::none()).unwrap();
        assert!(r.is_success());
        assert_eq!(r.text().unwrap(), "hello");
    }

    #[test]
    fn size_limit_and_status_codes() {
        let port = serve_once(b"HTTP/1.1 404 Not Found\r\nContent-Length: 10\r\nConnection: close\r\n\r\n0123456789");
        let client = HttpClient::new(Duration::from_secs(5));
        let mut r = client.get(&format!("http://127.0.0.1:{port}/"), &CancellationToken::none()).unwrap();
        assert_eq!(r.status, 404);
        assert!(!r.is_success());
        assert_eq!(r.read_to_end_limited(Some(4), None, None), Err(HttpError::TooLarge));
    }

    #[test]
    fn canceled_token_short_circuits() {
        let ct = CancellationToken::new();
        ct.cancel();
        assert_eq!(HttpClient::shared().get("http://127.0.0.1:1/", &ct).err(), Some(HttpError::Canceled));
    }
}
