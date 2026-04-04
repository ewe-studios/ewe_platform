/// Unit tests for the `url` module moved into the canonical units test tree.
///
/// These tests exercise `SimpleUrl` parsing / matching behavior (parameter
/// extraction, query handling, and matcher presence). They are non-destructive
/// copies adapted to the external test crate environment.
///
/// Note: Keep these tests small and deterministic so they run fast as unit tests.
use foundation_core::wire::simple_http::SimpleUrl;
use std::collections::BTreeMap;
use tracing_test::traced_test;

#[test]
#[traced_test]
fn test_parsed_url_without_any_special_elements() {
    let content = "/v1/service/endpoint";
    let resource_url = SimpleUrl::url_with_query(content);
    let (matched, params) = resource_url.extract_matched_url("/v1/service/endpoint");

    assert!(matched);
    assert!(params.is_none());
    assert_eq!(resource_url.url, content);
    assert!(resource_url.queries.is_none());
    assert!(resource_url.params.is_none());
    assert!(resource_url.matcher.is_some());
}

#[test]
#[traced_test]
fn test_parsed_url_with_multi_params_extracted() {
    let content = "/v1/service/endpoint/{user_id}/{message}";

    let params: Vec<String> = vec!["user_id".into(), "message".into()];

    let resource_url = SimpleUrl::url_with_query(content);

    assert_eq!(resource_url.url, content);
    assert_eq!(resource_url.queries, None);
    assert_eq!(resource_url.params, Some(params));
    assert!(resource_url.matcher.is_some());

    let (matched, params) = resource_url.extract_matched_url("/v1/service/endpoint/123/hello");

    assert!(matched);
    assert!(params.is_some());

    let mut expected_params: BTreeMap<String, String> = BTreeMap::new();
    expected_params.insert("user_id".into(), "123".into());
    expected_params.insert("message".into(), "hello".into());

    assert_eq!(params.unwrap(), expected_params);
}

#[test]
#[traced_test]
fn test_parsed_url_with_query_parameters() {
    let content = "/search?q=test&limit=10";
    // Create a SimpleUrl that expects query strings (url_with_query)
    let resource_url = SimpleUrl::url_with_query(content);

    // The matcher should be present for path matching
    assert!(resource_url.matcher.is_some());
    assert!(resource_url.params.is_none());

    // Match a path with query string appended
    let (matched, params) = resource_url.extract_matched_url("/search?q=test&limit=10");

    assert!(matched);
    // No named path params, but the extractor should still succeed without returning path params
    assert!(params.is_none());
}

#[test]
#[traced_test]
fn test_parsed_url_with_params_and_query_extraction() {
    let content = "/items/{category}/{id}";
    let resource_url = SimpleUrl::url_with_query(content);

    // Match path with query
    let (matched, params) = resource_url.extract_matched_url("/items/books/42");

    assert!(matched);
    assert!(params.is_some());

    let mut expected_params: BTreeMap<String, String> = BTreeMap::new();
    expected_params.insert("category".into(), "books".into());
    expected_params.insert("id".into(), "42".into());

    assert_eq!(params.unwrap(), expected_params);
}

#[test]
#[traced_test]
fn test_parsed_url_with_params_extracted() {
    let content = "/v1/service/endpoint/{user_id}/message";

    let params: Vec<String> = vec!["user_id".into()];

    let resource_url = SimpleUrl::url_with_query(content);

    assert_eq!(resource_url.url, content);
    assert_eq!(resource_url.queries, None);
    assert_eq!(resource_url.params, Some(params));
    assert!(resource_url.matcher.is_some());

    let (matched, params) = resource_url.extract_matched_url("/v1/service/endpoint/123/message");

    assert!(matched);
    assert!(params.is_some());

    let mut expected_params: BTreeMap<String, String> = BTreeMap::new();
    expected_params.insert("user_id".into(), "123".into());

    assert_eq!(params.unwrap(), expected_params);
}

#[test]
#[traced_test]
fn test_parsed_url_with_params() {
    let content = "/v1/service/endpoint/{user_id}/message";

    let params: Vec<String> = vec!["user_id".into()];

    let resource_url = SimpleUrl::url_with_query(content);

    assert_eq!(resource_url.url, content);
    assert_eq!(resource_url.queries, None);
    assert_eq!(resource_url.params, Some(params));
    assert!(resource_url.matcher.is_some());

    assert!(resource_url.matches_url("/v1/service/endpoint/123/message"));
    assert!(!resource_url.matches_url("/v1/service/endpoint/123/hello"));
}

#[test]
#[traced_test]
fn test_parsed_url_with_queries() {
    let content = "/v1/service/endpoint?userId=123&hello=abc";
    let mut queries: BTreeMap<String, String> = BTreeMap::new();
    queries.insert("userId".into(), "123".into());
    queries.insert("hello".into(), "abc".into());

    let resource_url = SimpleUrl::url_with_query(content);
    assert_eq!(resource_url.url, content);
    assert_eq!(resource_url.params, None);
    assert_eq!(resource_url.queries, Some(queries));
    assert!(resource_url.matcher.is_some());
    assert!(resource_url.matches_url("/v1/service/endpoint?userId=123&hello=abc"));
    assert!(!resource_url.matches_url("/v1/service/endpoint?userId=567&hello=abc"));
    assert!(!resource_url.matches_url("/v1/service/endpoint?userId=123&hello=bda"));
}

#[test]
#[traced_test]
fn test_unparsed_url() {
    let content = "/v1/service/endpoint?userId=123&hello=abc";
    let resource_url = SimpleUrl::url_only(content);
    assert_eq!(resource_url.url, content);
    assert_eq!(resource_url.params, None);
    assert_eq!(resource_url.queries, None);
    assert!(resource_url.matcher.is_none());
    assert!(resource_url.matches_url("/v1/service/endpoint?userId=123&hello=abc"));
    assert!(!resource_url.matches_url("/v1/service/endpoint?userId=123&hello=alex"));
    assert!(matches!(
        resource_url.extract_matched_url("/v1/service/endpoint?userId=123&hello=abc"),
        (true, None)
    ));
    assert!(matches!(
        resource_url.extract_matched_url("/v1/service/endpoint?userId=123&hello=alx"),
        (false, None)
    ));
}
