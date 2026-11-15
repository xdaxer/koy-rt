//! Yerel loopback (`http://127.0.0.1:{port}/callback/`) üzerinden OAuth 2.0 yetkilendirme kodunu bekler.
//!
//! C# `HttpListener` kullanır; burada `std::net` ile yalnız gereken kadarı yazılır: istek satırı okunur, yol öneki
//! eşleşmeyen istekler (ör. favicon) 404 alır ve beklemeye devam edilir. `localhost` adresi tarayıcıda `::1`'e de
//! çözülebildiği için IPv4 ve IPv6 loopback birlikte dinlenir.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

use koy_core::calendar::{CalendarProvider, OAuthCallbackResult, OAuthPkce, Uri};
use koy_core::cancel::CancellationToken;
use koy_core::time::TimeSpan;

pub struct LoopbackOAuthListener;

impl LoopbackOAuthListener {
    pub fn allocate_free_port() -> u16 {
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).and_then(|l| l.local_addr()).map(|a| a.port()).unwrap_or(0)
    }

    pub fn authorize(
        provider: CalendarProvider,
        client_id: &str,
        code_challenge: &str,
        state: &str,
        redirect_uri: &str,
        timeout: TimeSpan,
        ct: &CancellationToken,
    ) -> OAuthCallbackResult {
        let Some((host, port, prefix_path)) = split_redirect(redirect_uri) else {
            return OAuthCallbackResult::fail("auth_failed");
        };
        let mut listeners = Vec::new();
        if let Ok(l) = TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
            listeners.push(l);
        }
        if let Ok(l) = TcpListener::bind((Ipv6Addr::LOCALHOST, port)) {
            listeners.push(l);
        }
        if listeners.is_empty() {
            return OAuthCallbackResult::fail("auth_failed");
        }
        for l in &listeners {
            let _ = l.set_nonblocking(true);
        }

        let auth_url = OAuthPkce::build_authorization_url(provider, client_id, redirect_uri, code_challenge, state);
        crate::calendar::open_url(auth_url.original_string());

        let deadline = Instant::now() + timeout.to_std();
        loop {
            if ct.is_cancellation_requested() {
                return OAuthCallbackResult::fail("cancelled");
            }
            if Instant::now() >= deadline {
                return OAuthCallbackResult::fail("timeout");
            }
            let accepted = listeners.iter().find_map(|l| l.accept().ok());
            let Some((stream, _)) = accepted else {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            };
            let _ = stream.set_nonblocking(false);
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            if let Some(result) = handle(stream, &host, port, &prefix_path, state) {
                return result;
            }
        }
    }
}

/// `http://host:port/yol/` → (host, port, yol).
fn split_redirect(redirect_uri: &str) -> Option<(String, u16, String)> {
    let rest = redirect_uri.strip_prefix("http://")?;
    let slash = rest.find('/').unwrap_or(rest.len());
    let (authority, path) = rest.split_at(slash);
    let (host, port) = authority.rsplit_once(':')?;
    let path = if path.is_empty() { "/".to_string() } else { path.to_string() };
    Some((host.to_string(), port.parse().ok()?, path))
}

/// Bir isteği işler; geri çağrı isteği değilse (yanlış yol) `None` döner ve bekleme sürer.
fn handle(mut stream: TcpStream, host: &str, port: u16, prefix_path: &str, state: &str) -> Option<OAuthCallbackResult> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 16 * 1024 {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let head = String::from_utf8_lossy(&buf);
    let target = head.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("");
    let path = target.split('?').next().unwrap_or("");
    // HttpListener öneki: yol büyük/küçük harf duyarsız önekle eşleşmeli ("/callback" de "/callback/" önekine girer).
    let prefix_trim = prefix_path.trim_end_matches('/');
    if !koy_core::text::starts_with_ignore_case(path, prefix_trim) {
        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return None;
    }

    let uri = Uri::new(&format!("http://{host}:{port}{target}")).ok();
    let result = OAuthPkce::validate_callback(uri.as_ref(), state);
    write_html_response(&mut stream, result.success);
    Some(result)
}

fn write_html_response(stream: &mut TcpStream, success: bool) {
    let title = "Koy";
    let heading = if success { "Bağlantı tamamlandı / Connected" } else { "Yetkilendirme tamamlanamadı / Authorization failed" };
    let body = if success {
        "Bu sekmeyi kapatıp Koy uygulamasına dönebilirsiniz. / You can close this tab and return to Koy."
    } else {
        "Lütfen Koy ayarlarından yeniden deneyin. / Please try again from Koy settings."
    };
    let html = format!(
        "<!DOCTYPE html>\n<html lang=\"tr\">\n<head><meta charset=\"utf-8\"><title>{title}</title>\n\
<style>body{{font-family:Segoe UI,sans-serif;background:#111;color:#f4f4f5;display:flex;align-items:center;justify-content:center;height:100vh;margin:0}}\n\
.card{{background:#1c1c1f;border:1px solid #2e2e33;border-radius:14px;padding:28px 32px;max-width:420px;text-align:center}}\n\
h1{{font-size:18px;margin:0 0 8px}}p{{font-size:14px;color:#a1a1aa;margin:0;line-height:1.5}}</style></head>\n\
<body><div class=\"card\"><h1>{heading}</h1><p>{body}</p></div></body></html>"
    );
    let status = if success { "200 OK" } else { "400 Bad Request" };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_redirect_uris() {
        assert_eq!(split_redirect("http://127.0.0.1:5000/callback/"), Some(("127.0.0.1".into(), 5000, "/callback/".into())));
        assert_eq!(split_redirect("http://localhost:6000/"), Some(("localhost".into(), 6000, "/".into())));
        assert_eq!(split_redirect("https://x/"), None);
    }

    #[test]
    fn allocates_a_port() {
        assert_ne!(LoopbackOAuthListener::allocate_free_port(), 0);
    }
}
