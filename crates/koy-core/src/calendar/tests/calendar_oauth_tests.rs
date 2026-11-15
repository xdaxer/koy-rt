//! Karşılığı: `Calendar/CalendarOAuthTests.cs`.

use std::collections::HashMap;

use sha2::{Digest, Sha256};

use crate::calendar::*;

#[test]
fn pkce_verifier_and_challenge_produce_valid_s256_hash() {
    let verifier = OAuthPkce::create_code_verifier();
    assert!(verifier.len() >= 43);
    assert!(!verifier.contains('='));
    assert!(!verifier.contains('+'));
    assert!(!verifier.contains('/'));

    let challenge = OAuthPkce::compute_code_challenge(&verifier);
    let expected = OAuthPkce::base64_url_encode(&Sha256::digest(verifier.as_bytes()));
    assert_eq!(expected, challenge);
}

#[test]
fn pkce_matches_the_rfc7636_test_vector() {
    // RFC 7636 Ek B: bu doğrulayıcı için beklenen challenge.
    assert_eq!(
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
        OAuthPkce::compute_code_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk")
    );
}

#[test]
fn base64_url_encode_handles_every_padding_length() {
    assert_eq!("", OAuthPkce::base64_url_encode(b""));
    assert_eq!("Zg", OAuthPkce::base64_url_encode(b"f"));
    assert_eq!("Zm8", OAuthPkce::base64_url_encode(b"fo"));
    assert_eq!("Zm9v", OAuthPkce::base64_url_encode(b"foo"));
    assert_eq!("-_8", OAuthPkce::base64_url_encode(&[0xFB, 0xFF]));
}

#[test]
fn build_authorization_url_uses_read_only_scopes_and_no_secret() {
    let google_url = OAuthPkce::build_authorization_url(
        CalendarProvider::Google,
        "google-client-123",
        "http://127.0.0.1:50001/callback/",
        "challenge123",
        "state456",
    );

    let google_query = OAuthPkce::parse_query_string(&google_url.query());
    assert_eq!("google-client-123", google_query["client_id"]);
    assert_eq!("code", google_query["response_type"]);
    assert_eq!("S256", google_query["code_challenge_method"]);
    assert_eq!(CalendarOAuthConfig::GOOGLE_SCOPE, google_query["scope"]);
    assert_eq!("http://127.0.0.1:50001/callback/", google_query["redirect_uri"]);
    assert!(!google_query.contains_key("client_secret"));

    let outlook_url = OAuthPkce::build_authorization_url(
        CalendarProvider::Outlook,
        "outlook-client-456",
        "http://127.0.0.1:50002/callback/",
        "challenge123",
        "state789",
    );

    let outlook_query = OAuthPkce::parse_query_string(&outlook_url.query());
    assert_eq!("outlook-client-456", outlook_query["client_id"]);
    assert_eq!(CalendarOAuthConfig::OUTLOOK_SCOPE, outlook_query["scope"]);
    assert!(!outlook_query.contains_key("client_secret"));
}

#[test]
fn validate_callback_verifies_state_and_extracts_code() {
    let ok = OAuthPkce::validate_callback(Some(&Uri::new("http://127.0.0.1:50001/callback/?code=abc123&state=xyz").unwrap()), "xyz");
    assert!(ok.success);
    assert_eq!(Some("abc123".to_string()), ok.code);

    let mismatch = OAuthPkce::validate_callback(Some(&Uri::new("http://127.0.0.1:50001/callback/?code=abc123&state=wrong").unwrap()), "xyz");
    assert!(!mismatch.success);
    assert_eq!(Some("state_mismatch".to_string()), mismatch.error);

    let denied = OAuthPkce::validate_callback(Some(&Uri::new("http://127.0.0.1:50001/callback/?error=access_denied&state=xyz").unwrap()), "xyz");
    assert!(!denied.success);
    assert_eq!(Some("access_denied".to_string()), denied.error);

    assert_eq!(Some("missing_callback".to_string()), OAuthPkce::validate_callback(None, "xyz").error);
}

#[test]
fn query_string_parsing_decodes_escapes_and_plus() {
    let q = OAuthPkce::parse_query_string("?a=b%20c&d=e+f&flag&g=%E2%82%AC&h=%zz");
    assert_eq!("b c", q["a"]);
    assert_eq!("e f", q["d"]);
    assert_eq!("", q["flag"]);
    assert_eq!("€", q["g"]);
    assert_eq!("%zz", q["h"]);
}

#[test]
fn resolve_client_prefers_settings_then_environment_then_built_in() {
    let built_in = OAuthBuiltInClients::new("built-g", "built-secret", "built-o");
    let env = |name: &str| -> Option<String> {
        match name {
            CalendarOAuthConfig::GOOGLE_CLIENT_ID_ENV_VAR => Some("env-g".to_string()),
            CalendarOAuthConfig::GOOGLE_CLIENT_SECRET_ENV_VAR => Some("env-secret".to_string()),
            CalendarOAuthConfig::OUTLOOK_CLIENT_ID_ENV_VAR => Some("env-o".to_string()),
            _ => None,
        }
    };
    let no_env = |_: &str| -> Option<String> { None };

    let from_settings = CalendarOAuthConfig::resolve_client(
        CalendarProvider::Google,
        &CalendarSettings { google_client_id: " cfg-g ".into(), google_client_secret: " cfg-secret ".into(), ..Default::default() },
        Some(&built_in),
        Some(&env),
    );
    assert_eq!(OAuthClient::new("cfg-g", "cfg-secret"), from_settings);

    let from_env = CalendarOAuthConfig::resolve_client(CalendarProvider::Google, &CalendarSettings::default(), Some(&built_in), Some(&env));
    assert_eq!(OAuthClient::new("env-g", "env-secret"), from_env);

    let from_built_in = CalendarOAuthConfig::resolve_client(CalendarProvider::Google, &CalendarSettings::default(), Some(&built_in), Some(&no_env));
    assert_eq!(OAuthClient::new("built-g", "built-secret"), from_built_in);

    assert_eq!(
        OAuthClient::public("env-o"),
        CalendarOAuthConfig::resolve_client(CalendarProvider::Outlook, &CalendarSettings::default(), Some(&built_in), Some(&env))
    );
    assert_eq!(
        OAuthClient::public("built-o"),
        CalendarOAuthConfig::resolve_client(CalendarProvider::Outlook, &CalendarSettings::default(), Some(&built_in), Some(&no_env))
    );
}

#[test]
fn resolve_client_never_pairs_built_in_secret_with_user_client_id() {
    let built_in = OAuthBuiltInClients::new("built-g", "built-secret", "");
    let no_env = |_: &str| -> Option<String> { None };
    let client = CalendarOAuthConfig::resolve_client(
        CalendarProvider::Google,
        &CalendarSettings { google_client_id: "my-id".into(), ..Default::default() },
        Some(&built_in),
        Some(&no_env),
    );

    assert_eq!("my-id", client.client_id);
    assert_eq!("", client.client_secret);
}

#[test]
fn resolve_client_is_not_configured_without_any_source() {
    let no_env = |_: &str| -> Option<String> { None };
    let client = CalendarOAuthConfig::resolve_client(CalendarProvider::Google, &CalendarSettings::default(), None, Some(&no_env));
    assert!(!client.is_configured());
}

#[test]
fn oauth_client_adds_secret_only_when_present() {
    let mut with_secret = HashMap::new();
    OAuthClient::new("id", "secret").add_to(&mut with_secret);
    assert_eq!("id", with_secret["client_id"]);
    assert_eq!("secret", with_secret["client_secret"]);

    let mut public_client = HashMap::new();
    OAuthClient::public("id").add_to(&mut public_client);
    assert_eq!("id", public_client["client_id"]);
    assert!(!public_client.contains_key("client_secret"));
}

#[test]
fn session_state_follows_valid_transitions() {
    let mut session = OAuthSessionState::new();
    assert_eq!(OAuthFlowState::Disconnected, session.state());
    assert!(!session.is_busy());

    assert!(session.try_begin_authorize());
    assert_eq!(OAuthFlowState::Authorizing, session.state());
    assert!(session.is_busy());
    assert!(!session.try_begin_authorize());

    session.begin_token_exchange();
    assert_eq!(OAuthFlowState::ExchangingToken, session.state());

    session.complete_connected();
    assert_eq!(OAuthFlowState::Connected, session.state());
    assert!(!session.is_busy());

    session.fail("network_error");
    assert_eq!(OAuthFlowState::Error, session.state());
    assert_eq!(Some("network_error"), session.error_code());

    session.disconnect();
    assert_eq!(OAuthFlowState::Disconnected, session.state());
    assert_eq!(None, session.error_code());
}
