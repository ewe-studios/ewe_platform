use foundation_core::wire::simple_http::client::{Cookie, CookieJar, CookieParseError, SameSite};
use std::time::{Duration, SystemTime};

/// WHY: Basic cookie creation is the foundation - must work correctly
/// WHAT: `Cookie::new()` should create cookie with name and value
#[test]
fn test_cookie_new_basic() {
    let cookie = Cookie::new("session", "abc123");
    assert_eq!(cookie.name, "session");
    assert_eq!(cookie.value, "abc123");
    assert_eq!(cookie.domain, None);
    assert_eq!(cookie.path, None);
    assert_eq!(cookie.expires, None);
    assert_eq!(cookie.max_age, None);
    assert!(!cookie.secure);
    assert!(!cookie.http_only);
    assert_eq!(cookie.same_site, SameSite::Lax);
}

/// WHY: Builder pattern is the main API for cookie construction
/// WHAT: Builder methods should set attributes correctly
#[test]
fn test_cookie_builder_methods() {
    let expires = SystemTime::now();
    let max_age = Duration::from_secs(3600);

    let cookie = Cookie::new("session", "abc123")
        .domain("example.com")
        .path("/api")
        .secure(true)
        .http_only(true)
        .expires(expires)
        .max_age(max_age)
        .same_site(SameSite::Strict);

    assert_eq!(cookie.name, "session");
    assert_eq!(cookie.value, "abc123");
    assert_eq!(cookie.domain, Some("example.com".to_string()));
    assert_eq!(cookie.path, Some("/api".to_string()));
    assert!(cookie.secure);
    assert!(cookie.http_only);
    assert_eq!(cookie.expires, Some(expires));
    assert_eq!(cookie.max_age, Some(max_age));
    assert_eq!(cookie.same_site, SameSite::Strict);
}

/// WHY: Parsing Set-Cookie headers is core functionality
/// WHAT: Should parse basic name=value cookie
#[test]
fn test_cookie_parse_basic() {
    let cookie = Cookie::parse("session=abc123").unwrap();
    assert_eq!(cookie.name, "session");
    assert_eq!(cookie.value, "abc123");
    assert_eq!(cookie.domain, None);
    assert_eq!(cookie.path, None);
}

/// WHY: Real Set-Cookie headers include attributes
/// WHAT: Should parse all standard attributes correctly
#[test]
fn test_cookie_parse_with_attributes() {
    let cookie = Cookie::parse("session=abc123; Domain=example.com; Path=/api; Secure; HttpOnly; Max-Age=3600; SameSite=Strict").unwrap();

    assert_eq!(cookie.name, "session");
    assert_eq!(cookie.value, "abc123");
    assert_eq!(cookie.domain, Some("example.com".to_string()));
    assert_eq!(cookie.path, Some("/api".to_string()));
    assert!(cookie.secure);
    assert!(cookie.http_only);
    assert_eq!(cookie.max_age, Some(Duration::from_secs(3600)));
    assert_eq!(cookie.same_site, SameSite::Strict);
}

/// WHY: Invalid headers must be rejected with proper errors
/// WHAT: Should return error for malformed cookie strings
#[test]
fn test_cookie_parse_invalid_format() {
    // Missing =
    let result = Cookie::parse("invalidsession");
    assert!(result.is_err());
    match result.unwrap_err() {
        CookieParseError::InvalidFormat(_) => {}
        _ => panic!("Expected InvalidFormat error"),
    }

    // Empty string
    let result = Cookie::parse("");
    assert!(result.is_err());
}

/// WHY: `CookieJar` is the central storage - must correctly store cookies
/// WHAT: Should add cookie and allow retrieval by domain/path/name
#[test]
fn test_cookie_jar_add_basic() {
    let mut jar = CookieJar::new();
    let cookie = Cookie::new("session", "abc123")
        .domain("example.com")
        .path("/");

    jar.add(cookie.clone());

    // Verify cookie was added (we'll test retrieval next)
    assert_eq!(jar.len(), 1);
}

/// WHY: Same cookie (domain/path/name) should replace old value
/// WHAT: Adding cookie with same key should replace existing
#[test]
fn test_cookie_jar_add_replaces() {
    let mut jar = CookieJar::new();

    let cookie1 = Cookie::new("session", "old_value")
        .domain("example.com")
        .path("/");
    jar.add(cookie1);

    let cookie2 = Cookie::new("session", "new_value")
        .domain("example.com")
        .path("/");
    jar.add(cookie2);

    // Should only have one cookie (replaced)
    assert_eq!(jar.len(), 1);
}

/// WHY: Domain matching is critical for cookie security per RFC 6265
/// WHAT: Exact domain match should return true
#[test]
fn test_domain_matches_exact() {
    // We need to access the internal function, so we'll test through get_for_url later
    // For now, let's create a helper test using the URL module
    use foundation_core::wire::simple_http::url::Uri;

    let mut jar = CookieJar::new();
    let cookie = Cookie::new("session", "abc")
        .domain("example.com")
        .path("/");
    jar.add(cookie);

    let uri = Uri::parse("http://example.com/").unwrap();
    let matches = jar.get_for_url(&uri);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "session");
}

/// WHY: Subdomain cookies (starting with dot) must match subdomains per RFC 6265
/// WHAT: Cookie with .example.com should match www.example.com and api.example.com
#[test]
fn test_domain_matches_subdomain() {
    use foundation_core::wire::simple_http::url::Uri;

    let mut jar = CookieJar::new();
    let cookie = Cookie::new("session", "abc")
        .domain(".example.com")
        .path("/");
    jar.add(cookie);

    // Should match subdomain
    let uri = Uri::parse("http://www.example.com/").unwrap();
    let matches = jar.get_for_url(&uri);
    assert_eq!(matches.len(), 1);

    // Should match another subdomain
    let uri = Uri::parse("http://api.example.com/").unwrap();
    let matches = jar.get_for_url(&uri);
    assert_eq!(matches.len(), 1);

    // Should match example.com itself
    let uri = Uri::parse("http://example.com/").unwrap();
    let matches = jar.get_for_url(&uri);
    assert_eq!(matches.len(), 1);

    // Should NOT match different domain
    let uri = Uri::parse("http://other.com/").unwrap();
    let matches = jar.get_for_url(&uri);
    assert_eq!(matches.len(), 0);
}

/// WHY: Path matching ensures cookies only sent to correct paths
/// WHAT: Cookie with path /api should match /api and /api/users but not /
#[test]
fn test_path_matches() {
    use foundation_core::wire::simple_http::url::Uri;

    let mut jar = CookieJar::new();
    let cookie = Cookie::new("api_token", "xyz")
        .domain("example.com")
        .path("/api");
    jar.add(cookie);

    // Should match exact path
    let uri = Uri::parse("http://example.com/api").unwrap();
    let matches = jar.get_for_url(&uri);
    assert_eq!(matches.len(), 1);

    // Should match subpath
    let uri = Uri::parse("http://example.com/api/users").unwrap();
    let matches = jar.get_for_url(&uri);
    assert_eq!(matches.len(), 1);

    // Should NOT match parent path
    let uri = Uri::parse("http://example.com/").unwrap();
    let matches = jar.get_for_url(&uri);
    assert_eq!(matches.len(), 0);

    // Should NOT match different path
    let uri = Uri::parse("http://example.com/admin").unwrap();
    let matches = jar.get_for_url(&uri);
    assert_eq!(matches.len(), 0);
}

/// WHY: Secure cookies must only be sent over HTTPS per RFC 6265
/// WHAT: Secure cookie should match HTTPS but not HTTP
#[test]
fn test_secure_cookie_filtering() {
    use foundation_core::wire::simple_http::url::Uri;

    let mut jar = CookieJar::new();
    let cookie = Cookie::new("secure_token", "abc")
        .domain("example.com")
        .path("/")
        .secure(true);
    jar.add(cookie);

    // Should match HTTPS
    let uri = Uri::parse("https://example.com/").unwrap();
    let matches = jar.get_for_url(&uri);
    assert_eq!(matches.len(), 1);

    // Should NOT match HTTP
    let uri = Uri::parse("http://example.com/").unwrap();
    let matches = jar.get_for_url(&uri);
    assert_eq!(matches.len(), 0);
}

/// WHY: Need to remove specific cookies or clear entire jar
/// WHAT: `clear()` should remove all cookies, `remove()` should remove specific cookie
#[test]
fn test_cookie_jar_clear_and_remove() {
    let mut jar = CookieJar::new();

    let cookie1 = Cookie::new("session", "abc")
        .domain("example.com")
        .path("/");
    let cookie2 = Cookie::new("token", "xyz")
        .domain("example.com")
        .path("/api");
    jar.add(cookie1);
    jar.add(cookie2);

    assert_eq!(jar.len(), 2);

    // Remove specific cookie
    jar.remove("example.com", "/api", "token");
    assert_eq!(jar.len(), 1);

    // Clear all
    jar.clear();
    assert_eq!(jar.len(), 0);
    assert!(jar.is_empty());
}
