//! CORS middleware tests (moved from `src/shared/middleware/cors.rs`).

use foundation_http::shared::middleware::CorsConfig;

#[test]
fn test_cors_config_permissive() {
    let config = CorsConfig::permissive();
    assert!(config.is_origin_allowed("https://example.com"));
    assert!(config.is_origin_allowed("*"));
}

#[test]
fn test_cors_config_restrictive() {
    let config = CorsConfig::restrictive();
    assert!(!config.is_origin_allowed("https://example.com"));
}

#[test]
fn test_cors_config_with_origin() {
    let config = CorsConfig::restrictive().with_origin("https://example.com");
    assert!(config.is_origin_allowed("https://example.com"));
    assert!(!config.is_origin_allowed("https://other.com"));
}
