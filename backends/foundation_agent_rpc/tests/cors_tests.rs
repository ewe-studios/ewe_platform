//! The CORS answer of a browser-facing host (spec 83 feature 05, section 6.5).
//!
//! An allowed origin gets the exact origin, `Vary: Origin`, the allowed
//! methods and headers on a preflight, and the readable headers on every
//! response. Another origin gets nothing, so the browser blocks the page.

use foundation_agent_rpc::core::cors::CorsConfig;

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn config() -> CorsConfig {
    CorsConfig::default().with_allowed_origin("https://app.example.com")
}

#[test]
fn defaults_name_the_connect_headers() {
    let config = CorsConfig::default();
    assert!(
        config.allowed_origins.is_empty(),
        "no origin calls by default"
    );
    assert!(config.allowed_headers.iter().any(|h| h == "Authorization"));
    assert!(config
        .allowed_headers
        .iter()
        .any(|h| h == "Connect-Timeout-Ms"));
    assert_eq!(config.max_age.as_secs(), 7200);
    assert!(
        !config.allowed_origins.iter().any(|origin| origin == "*"),
        "credentials never use a wildcard origin"
    );
}

#[test]
fn an_allowed_origin_gets_the_preflight_answer() {
    let headers = config().preflight_headers("https://app.example.com");
    assert_eq!(
        header(&headers, "Access-Control-Allow-Origin"),
        Some("https://app.example.com")
    );
    assert_eq!(header(&headers, "Vary"), Some("Origin"));
    assert_eq!(
        header(&headers, "Access-Control-Allow-Methods"),
        Some("POST, GET")
    );
    let allowed = header(&headers, "Access-Control-Allow-Headers").unwrap();
    assert!(allowed.contains("Authorization"));
    assert!(allowed.contains("Connect-Protocol-Version"));
    assert_eq!(header(&headers, "Access-Control-Max-Age"), Some("7200"));
}

#[test]
fn every_response_to_an_allowed_origin_names_it() {
    let headers = config().response_headers("https://app.example.com");
    assert_eq!(
        header(&headers, "Access-Control-Allow-Origin"),
        Some("https://app.example.com")
    );
    assert_eq!(header(&headers, "Vary"), Some("Origin"));
    assert_eq!(
        header(&headers, "Access-Control-Expose-Headers"),
        Some("Grpc-Status, Grpc-Message, Grpc-Status-Details-Bin")
    );
    assert_eq!(
        header(&headers, "Access-Control-Allow-Credentials"),
        Some("true")
    );
}

#[test]
fn another_origin_gets_no_cors_headers() {
    let config = config();
    assert_eq!(
        config.preflight_headers("https://evil.example.com"),
        Vec::<(String, String)>::new()
    );
    assert_eq!(
        config.response_headers("https://evil.example.com"),
        Vec::<(String, String)>::new()
    );
    assert!(!config.allows("https://evil.example.com"));
}
