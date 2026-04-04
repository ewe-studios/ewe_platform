/// Unit tests for proxy support
///
/// WHY: Verify proxy configuration parsing, environment detection, and NO_PROXY bypass logic
///
/// WHAT: Tests for ProxyConfig, ProxyAuth, ProxyProtocol parsing and functionality
///
/// HOW: External unit tests following ewe_platform testing conventions
use foundation_core::wire::simple_http::client::{ProxyAuth, ProxyConfig, ProxyProtocol};
use foundation_core::wire::simple_http::url::Scheme;
use serial_test::serial;

// ============================================================================
// ProxyConfig::parse() tests
// ============================================================================

#[test]
fn test_proxy_config_parse_http() {
    let config = ProxyConfig::parse("http://proxy.com:8080").unwrap();
    assert_eq!(config.protocol, ProxyProtocol::Http);
    assert_eq!(config.host, "proxy.com");
    assert_eq!(config.port, 8080);
    assert!(config.auth.is_none());
}

#[test]
fn test_proxy_config_parse_https() {
    let config = ProxyConfig::parse("https://proxy.com:8443").unwrap();
    assert_eq!(config.protocol, ProxyProtocol::Https);
    assert_eq!(config.host, "proxy.com");
    assert_eq!(config.port, 8443);
    assert!(config.auth.is_none());
}

#[test]
fn test_proxy_config_parse_with_auth() {
    let config = ProxyConfig::parse("http://user:pass@proxy.com:8080").unwrap();
    assert_eq!(config.protocol, ProxyProtocol::Http);
    assert_eq!(config.host, "proxy.com");
    assert_eq!(config.port, 8080);
    assert!(config.auth.is_some());

    let auth = config.auth.unwrap();
    assert_eq!(auth.username, "user");
    assert_eq!(auth.password, "pass");
}

#[test]
fn test_proxy_config_parse_with_complex_auth() {
    // Test with special characters in password
    let config = ProxyConfig::parse("http://admin:p@ssw0rd!@proxy.example.com:3128").unwrap();
    assert_eq!(config.protocol, ProxyProtocol::Http);
    assert_eq!(config.host, "proxy.example.com");
    assert_eq!(config.port, 3128);

    let auth = config.auth.unwrap();
    assert_eq!(auth.username, "admin");
    assert_eq!(auth.password, "p@ssw0rd!");
}

#[test]
#[cfg(feature = "socks5")]
fn test_proxy_config_parse_socks5() {
    let config = ProxyConfig::parse("socks5://proxy.com:1080").unwrap();
    assert_eq!(config.protocol, ProxyProtocol::Socks5);
    assert_eq!(config.host, "proxy.com");
    assert_eq!(config.port, 1080);
    assert!(config.auth.is_none());
}

#[test]
#[cfg(not(feature = "socks5"))]
fn test_proxy_config_parse_socks5_disabled() {
    let result = ProxyConfig::parse("socks5://proxy.com:1080");
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("SOCKS5"));
    assert!(err_msg.contains("feature"));
}

#[test]
fn test_proxy_config_invalid_protocol() {
    let result = ProxyConfig::parse("ftp://proxy.com:21");
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("Unsupported proxy protocol"));
    assert!(err_msg.contains("ftp"));
}

#[test]
fn test_proxy_config_missing_protocol() {
    let result = ProxyConfig::parse("proxy.com:8080");
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("Missing protocol separator"));
}

#[test]
fn test_proxy_config_missing_port() {
    let result = ProxyConfig::parse("http://proxy.com");
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("Missing port"));
}

#[test]
fn test_proxy_config_invalid_port() {
    let result = ProxyConfig::parse("http://proxy.com:invalid");
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("Invalid port number"));
}

#[test]
fn test_proxy_config_invalid_auth_format() {
    // Missing colon in auth section
    let result = ProxyConfig::parse("http://useronly@proxy.com:8080");
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("Invalid auth format"));
}

// ============================================================================
// ProxyAuth::to_basic_auth() tests
// ============================================================================

#[test]
fn test_proxy_auth_basic_encoding() {
    let auth = ProxyAuth::new("user", "password");
    let encoded = auth.to_basic_auth();
    // "user:password" in Base64 is "dXNlcjpwYXNzd29yZA=="
    assert_eq!(encoded, "dXNlcjpwYXNzd29yZA==");
}

#[test]
fn test_proxy_auth_basic_encoding_admin() {
    let auth = ProxyAuth::new("admin", "secret");
    let encoded = auth.to_basic_auth();
    // "admin:secret" in Base64 is "YWRtaW46c2VjcmV0"
    assert_eq!(encoded, "YWRtaW46c2VjcmV0");
}

#[test]
fn test_proxy_auth_empty_credentials() {
    let auth = ProxyAuth::new("", "");
    let encoded = auth.to_basic_auth();
    // ":" in Base64 is "Og=="
    assert_eq!(encoded, "Og==");
}

// ============================================================================
// ProxyConfig::should_bypass() tests
// ============================================================================

#[test]
#[serial]
fn test_should_bypass_exact_match() {
    std::env::set_var("NO_PROXY", "localhost,127.0.0.1,example.com");

    assert!(ProxyConfig::should_bypass("localhost"));
    assert!(ProxyConfig::should_bypass("127.0.0.1"));
    assert!(ProxyConfig::should_bypass("example.com"));
    assert!(!ProxyConfig::should_bypass("other.com"));
    // NOTE: "example.com" without leading dot also matches subdomains (common behavior)
    assert!(ProxyConfig::should_bypass("api.example.com"));

    std::env::remove_var("NO_PROXY");
}

#[test]
#[serial]
fn test_should_bypass_domain_suffix_with_dot() {
    std::env::set_var("NO_PROXY", ".example.com");

    assert!(ProxyConfig::should_bypass("api.example.com"));
    assert!(ProxyConfig::should_bypass("www.example.com"));
    assert!(ProxyConfig::should_bypass("sub.api.example.com"));
    assert!(!ProxyConfig::should_bypass("example.com")); // Exact match requires dot prefix
    assert!(!ProxyConfig::should_bypass("other.com"));

    std::env::remove_var("NO_PROXY");
}

#[test]
#[serial]
fn test_should_bypass_domain_suffix_without_dot() {
    std::env::set_var("NO_PROXY", "example.com");

    // With suffix matching, "example.com" should match "*.example.com"
    assert!(ProxyConfig::should_bypass("api.example.com"));
    assert!(ProxyConfig::should_bypass("www.example.com"));
    assert!(ProxyConfig::should_bypass("example.com")); // Exact match
    assert!(!ProxyConfig::should_bypass("other.com"));
    assert!(!ProxyConfig::should_bypass("notexample.com")); // Requires exact or suffix

    std::env::remove_var("NO_PROXY");
}

#[test]
#[serial]
fn test_should_bypass_wildcard() {
    std::env::set_var("NO_PROXY", "*");

    assert!(ProxyConfig::should_bypass("anything.com"));
    assert!(ProxyConfig::should_bypass("localhost"));
    assert!(ProxyConfig::should_bypass("192.168.1.1"));

    std::env::remove_var("NO_PROXY");
}

#[test]
#[serial]
fn test_should_bypass_multiple_patterns() {
    std::env::set_var("NO_PROXY", "localhost,.internal.com,192.168.0.0");

    assert!(ProxyConfig::should_bypass("localhost"));
    assert!(ProxyConfig::should_bypass("api.internal.com"));
    assert!(ProxyConfig::should_bypass("192.168.0.0"));
    assert!(!ProxyConfig::should_bypass("external.com"));

    std::env::remove_var("NO_PROXY");
}

#[test]
#[serial]
fn test_should_bypass_empty_no_proxy() {
    std::env::remove_var("NO_PROXY");
    std::env::remove_var("no_proxy");

    assert!(!ProxyConfig::should_bypass("anything.com"));
    assert!(!ProxyConfig::should_bypass("localhost"));
}

#[test]
#[serial]
fn test_should_bypass_case_insensitive_env_var() {
    // Test lowercase no_proxy
    std::env::remove_var("NO_PROXY");
    std::env::set_var("no_proxy", "localhost");

    assert!(ProxyConfig::should_bypass("localhost"));

    std::env::remove_var("no_proxy");
}

#[test]
#[serial]
fn test_should_bypass_whitespace_handling() {
    std::env::set_var("NO_PROXY", " localhost , example.com , 127.0.0.1 ");

    assert!(ProxyConfig::should_bypass("localhost"));
    assert!(ProxyConfig::should_bypass("example.com"));
    assert!(ProxyConfig::should_bypass("127.0.0.1"));

    std::env::remove_var("NO_PROXY");
}

// ============================================================================
// ProxyConfig::from_env() tests
// ============================================================================

#[test]
#[serial]
fn test_from_env_http_uppercase() {
    std::env::set_var("HTTP_PROXY", "http://proxy.com:8080");

    let proxy = ProxyConfig::from_env(&Scheme::HTTP);
    assert!(proxy.is_some());

    let proxy = proxy.unwrap();
    assert_eq!(proxy.protocol, ProxyProtocol::Http);
    assert_eq!(proxy.host, "proxy.com");
    assert_eq!(proxy.port, 8080);

    std::env::remove_var("HTTP_PROXY");
}

#[test]
#[serial]
fn test_from_env_http_lowercase() {
    std::env::remove_var("HTTP_PROXY");
    std::env::set_var("http_proxy", "http://proxy.com:8080");

    let proxy = ProxyConfig::from_env(&Scheme::HTTP);
    assert!(proxy.is_some());

    let proxy = proxy.unwrap();
    assert_eq!(proxy.host, "proxy.com");

    std::env::remove_var("http_proxy");
}

#[test]
#[serial]
fn test_from_env_https_uppercase() {
    std::env::set_var("HTTPS_PROXY", "https://proxy.com:8443");

    let proxy = ProxyConfig::from_env(&Scheme::HTTPS);
    assert!(proxy.is_some());

    let proxy = proxy.unwrap();
    assert_eq!(proxy.protocol, ProxyProtocol::Https);
    assert_eq!(proxy.host, "proxy.com");
    assert_eq!(proxy.port, 8443);

    std::env::remove_var("HTTPS_PROXY");
}

#[test]
#[serial]
fn test_from_env_https_lowercase() {
    std::env::remove_var("HTTPS_PROXY");
    std::env::set_var("https_proxy", "https://proxy.com:8443");

    let proxy = ProxyConfig::from_env(&Scheme::HTTPS);
    assert!(proxy.is_some());

    std::env::remove_var("https_proxy");
}

#[test]
#[serial]
fn test_from_env_not_set() {
    std::env::remove_var("HTTP_PROXY");
    std::env::remove_var("http_proxy");

    let proxy = ProxyConfig::from_env(&Scheme::HTTP);
    assert!(proxy.is_none());
}

#[test]
#[serial]
fn test_from_env_invalid_url() {
    // If env var has invalid URL, from_env returns None (silently ignores parse error)
    std::env::set_var("HTTP_PROXY", "not-a-valid-proxy-url");

    let proxy = ProxyConfig::from_env(&Scheme::HTTP);
    assert!(proxy.is_none()); // Parse error is silently ignored

    std::env::remove_var("HTTP_PROXY");
}

#[test]
#[serial]
fn test_from_env_with_auth() {
    std::env::set_var("HTTP_PROXY", "http://user:pass@proxy.com:8080");

    let proxy = ProxyConfig::from_env(&Scheme::HTTP);
    assert!(proxy.is_some());

    let proxy = proxy.unwrap();
    assert!(proxy.auth.is_some());
    assert_eq!(proxy.auth.unwrap().username, "user");

    std::env::remove_var("HTTP_PROXY");
}

// ============================================================================
// ProxyConfig constructor tests
// ============================================================================

#[test]
fn test_proxy_config_new() {
    let proxy = ProxyConfig::new(ProxyProtocol::Http, "proxy.example.com", 8080);
    assert_eq!(proxy.protocol, ProxyProtocol::Http);
    assert_eq!(proxy.host, "proxy.example.com");
    assert_eq!(proxy.port, 8080);
    assert!(proxy.auth.is_none());
}

#[test]
fn test_proxy_config_with_auth_builder() {
    let proxy = ProxyConfig::new(ProxyProtocol::Http, "proxy.example.com", 8080)
        .with_auth("admin", "secret");

    assert!(proxy.auth.is_some());
    let auth = proxy.auth.unwrap();
    assert_eq!(auth.username, "admin");
    assert_eq!(auth.password, "secret");
}

// ============================================================================
// Edge case tests
// ============================================================================

#[test]
fn test_proxy_config_parse_ipv4_host() {
    let config = ProxyConfig::parse("http://192.168.1.1:8080").unwrap();
    assert_eq!(config.host, "192.168.1.1");
    assert_eq!(config.port, 8080);
}

#[test]
fn test_proxy_config_parse_localhost() {
    let config = ProxyConfig::parse("http://localhost:3128").unwrap();
    assert_eq!(config.host, "localhost");
    assert_eq!(config.port, 3128);
}

#[test]
fn test_proxy_config_parse_high_port() {
    let config = ProxyConfig::parse("http://proxy.com:65535").unwrap();
    assert_eq!(config.port, 65535);
}

#[test]
fn test_proxy_config_parse_low_port() {
    let config = ProxyConfig::parse("http://proxy.com:1").unwrap();
    assert_eq!(config.port, 1);
}

#[test]
fn test_proxy_config_parse_port_out_of_range() {
    // Port > u16::MAX
    let result = ProxyConfig::parse("http://proxy.com:99999");
    assert!(result.is_err());
}
