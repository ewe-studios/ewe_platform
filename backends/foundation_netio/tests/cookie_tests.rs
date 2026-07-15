//! Tests extracted from simple_http/client/shared/cookie.rs
mod tests {
    use foundation_netio::shared::client::cookie::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn to_set_cookie_string_minimal() {
        let cookie = Cookie::new("session", "abc123");
        let s = cookie.to_set_cookie_string();
        assert!(s.starts_with("session=abc123; "));
        assert!(s.contains("SameSite=Lax"));
    }

    #[test]
    fn to_set_cookie_string_with_max_age() {
        let cookie = Cookie::new("remember", "me").max_age(Duration::from_secs(86400));
        let s = cookie.to_set_cookie_string();
        assert!(s.contains("Max-Age=86400"));
    }

    #[test]
    fn to_set_cookie_string_with_path_and_domain() {
        let cookie = Cookie::new("auth", "token")
            .path("/api")
            .domain(".example.com");
        let s = cookie.to_set_cookie_string();
        assert!(s.contains("Path=/api"));
        assert!(s.contains("Domain=.example.com"));
    }

    #[test]
    fn to_set_cookie_string_with_security_flags() {
        let cookie = Cookie::new("secure_session", "xyz")
            .secure(true)
            .http_only(true)
            .same_site(SameSite::Strict);
        let s = cookie.to_set_cookie_string();
        assert!(s.contains("Secure"));
        assert!(s.contains("HttpOnly"));
        assert!(s.contains("SameSite=Strict"));
    }

    #[test]
    fn to_set_cookie_string_with_expires() {
        let expires = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let cookie = Cookie::new("temp", "val").expires(expires);
        let s = cookie.to_set_cookie_string();
        assert!(s.contains("Expires="));
    }

    #[test]
    fn to_set_cookie_string_roundtrip_with_parse() {
        let original = Cookie::new("session", "abc123")
            .path("/")
            .http_only(true)
            .max_age(Duration::from_secs(3600));
        let formatted = original.to_set_cookie_string();
        let parsed = Cookie::parse(&formatted).unwrap();

        assert_eq!(parsed.name, original.name);
        assert_eq!(parsed.value, original.value);
        assert_eq!(parsed.path, original.path);
        assert_eq!(parsed.http_only, original.http_only);
        assert_eq!(parsed.max_age, original.max_age);
        assert_eq!(parsed.same_site, original.same_site);
    }
}
