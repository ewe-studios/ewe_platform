use foundation_core::wire::simple_http::url::Uri;

#[test]
fn test_uri_parse_simple_http() {
    let uri = Uri::parse("http://example.com").unwrap();
    assert_eq!(uri.scheme().as_str(), "http");
    assert_eq!(uri.host_str().unwrap(), "example.com");
    assert_eq!(uri.port_or_default(), 80);
    assert_eq!(uri.path(), "/");
    assert!(uri.query().is_none());
}

#[test]
fn test_uri_parse_with_port() {
    let uri = Uri::parse("https://example.com:8443/path").unwrap();
    assert_eq!(uri.scheme().as_str(), "https");
    assert_eq!(uri.host_str().unwrap(), "example.com");
    assert_eq!(uri.port().unwrap(), 8443);
    assert_eq!(uri.path(), "/path");
}

#[test]
fn test_uri_parse_with_query() {
    let uri = Uri::parse("http://example.com/path?key=value&foo=bar").unwrap();
    assert_eq!(uri.path(), "/path");
    assert_eq!(uri.query().unwrap(), "key=value&foo=bar");
}

#[test]
fn test_uri_parse_with_fragment() {
    let uri = Uri::parse("http://example.com/path#section").unwrap();
    assert_eq!(uri.path(), "/path");
    assert_eq!(uri.fragment().unwrap(), "section");
}

#[test]
fn test_uri_parse_complete() {
    let uri = Uri::parse("https://user:pass@example.com:8080/path?key=value#section").unwrap();
    assert_eq!(uri.scheme().as_str(), "https");
    assert_eq!(uri.host_str().unwrap(), "example.com");
    assert_eq!(uri.port().unwrap(), 8080);
    assert_eq!(uri.path(), "/path");
    assert_eq!(uri.query().unwrap(), "key=value");
    assert_eq!(uri.fragment().unwrap(), "section");
}

#[test]
fn test_uri_display() {
    let original = "http://example.com:8080/path?query#fragment";
    let uri = Uri::parse(original).unwrap();
    assert_eq!(uri.to_string(), original);
}

#[test]
fn test_uri_parse_missing_scheme() {
    assert!(Uri::parse("example.com/path").is_err());
}

#[test]
fn test_uri_parse_invalid_port() {
    assert!(Uri::parse("http://example.com:99999/path").is_err());
}
