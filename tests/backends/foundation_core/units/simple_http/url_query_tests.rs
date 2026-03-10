use foundation_core::wire::simple_http::{
    client::ParsedUrl, url::percent_decode, url::percent_encode, url::Query,
};

/// Unit tests for URL query extraction/visibility via `ParsedUrl`.
///
/// These tests exercise the `ParsedUrl::parse(...)` and `.query()` accessor to
/// ensure query strings are preserved and accessible from the parsed URL type.
///
/// They are intentionally small and deterministic to run as part of the fast
/// unit test tree under `tests/backends/foundation_core/units/simple_http/`.
#[test]
fn test_parsed_url_with_query_returns_query_string() {
    let url = ParsedUrl::parse("http://example.com/search?q=test&limit=10").unwrap();
    let q = url.query();
    assert!(q.is_some(), "expected a query component");
    assert_eq!(q.unwrap(), "q=test&limit=10");
}

#[test]
fn test_parsed_url_without_query_returns_none() {
    let url = ParsedUrl::parse("http://example.com/path").unwrap();
    assert!(url.query().is_none(), "expected no query component");
}

#[test]
fn test_parsed_url_with_empty_and_present_values() {
    let url = ParsedUrl::parse("http://example.com/?a=&b=2").unwrap();
    let q = url.query().expect("expected query present");
    // The exact ordering is preserved as in the raw URL
    assert_eq!(q, "a=&b=2");
}

#[test]
fn test_parsed_url_with_percent_encoded_values() {
    let url = ParsedUrl::parse("http://example.com/?q=hello%20world&lang=en").unwrap();
    let q = url.query().expect("expected query present");
    assert!(q.contains("q=hello%20world"));
    assert!(q.contains("lang=en"));
}

#[test]
fn test_query_new() {
    let query = Query::new();
    assert!(query.is_empty());
    assert_eq!(query.len(), 0);
}

#[test]
fn test_query_parse_simple() {
    let query = Query::parse("key=value").unwrap();
    assert_eq!(query.get("key"), Some("value"));
    assert_eq!(query.len(), 1);
}

#[test]
fn test_query_parse_multiple() {
    let query = Query::parse("key1=value1&key2=value2&key3=value3").unwrap();
    assert_eq!(query.get("key1"), Some("value1"));
    assert_eq!(query.get("key2"), Some("value2"));
    assert_eq!(query.get("key3"), Some("value3"));
    assert_eq!(query.len(), 3);
}

#[test]
fn test_query_parse_duplicate_keys() {
    let query = Query::parse("key=value1&key=value2").unwrap();
    assert_eq!(query.get("key"), Some("value1")); // First value
    assert_eq!(query.get_all("key"), vec!["value1", "value2"]);
}

#[test]
fn test_query_parse_empty_value() {
    let query = Query::parse("key=").unwrap();
    assert_eq!(query.get("key"), Some(""));
}

#[test]
fn test_query_parse_no_value() {
    let query = Query::parse("key").unwrap();
    assert_eq!(query.get("key"), Some(""));
}

#[test]
fn test_query_parse_empty() {
    let query = Query::parse("").unwrap();
    assert!(query.is_empty());
}

#[test]
fn test_query_append() {
    let mut query = Query::new();
    query.append("key1", "value1");
    query.append("key2", "value2");

    assert_eq!(query.get("key1"), Some("value1"));
    assert_eq!(query.get("key2"), Some("value2"));
    assert_eq!(query.len(), 2);
}

#[test]
fn test_query_display() {
    let mut query = Query::new();
    query.append("key1", "value1");
    query.append("key2", "value2");

    assert_eq!(query.to_string(), "key1=value1&key2=value2");
}

#[test]
fn test_percent_decode_spaces() {
    assert_eq!(percent_decode("hello+world").unwrap(), "hello world");
    assert_eq!(percent_decode("hello%20world").unwrap(), "hello world");
}

#[test]
fn test_percent_decode_special_chars() {
    assert_eq!(percent_decode("100%25").unwrap(), "100%");
    assert_eq!(percent_decode("a%2Bb").unwrap(), "a+b");
    assert_eq!(percent_decode("x%3Dy").unwrap(), "x=y");
}

#[test]
fn test_percent_encode_spaces() {
    assert_eq!(percent_encode("hello world"), "hello+world");
}

#[test]
fn test_percent_encode_special_chars() {
    assert_eq!(percent_encode("100%"), "100%25");
    assert_eq!(percent_encode("a+b"), "a%2Bb");
    assert_eq!(percent_encode("x=y"), "x%3Dy");
}

#[test]
fn test_percent_encode_unreserved() {
    assert_eq!(percent_encode("azAZ09-_.~"), "azAZ09-_.~");
}

#[test]
fn test_query_roundtrip() {
    let original = "key=hello+world&foo=100%25";
    let query = Query::parse(original).unwrap();
    let encoded = query.to_string();

    // Re-parse and check
    let query2 = Query::parse(&encoded).unwrap();
    assert_eq!(query2.get("key"), Some("hello world"));
    assert_eq!(query2.get("foo"), Some("100%"));
}

#[test]
fn test_percent_decode_invalid() {
    assert!(percent_decode("hello%").is_err()); // Incomplete
    assert!(percent_decode("hello%2").is_err()); // Incomplete
    assert!(percent_decode("hello%ZZ").is_err()); // Invalid hex
}
