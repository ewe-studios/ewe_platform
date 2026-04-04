//! Unit tests for URL path handling (moved from `url/path.rs` into the canonical
//! `tests/backends/foundation_core/units/simple_http/` tree).
//!
//! These tests are intentionally small and fast unit tests that exercise the
//! public `SimpleUrl` helper used by the simple HTTP backend for path matching
//! and parameter extraction. They avoid network access and focus on parser logic.

use foundation_core::wire::simple_http::{url::PathAndQuery, SimpleUrl};
use std::collections::BTreeMap;

#[test]
fn test_parsed_url_without_any_special_elements() {
    // A plain path with no parameter placeholders should match literally and
    // not produce extracted params.
    let content = "/v1/service/endpoint";
    let resource_url = SimpleUrl::url_with_query(content);
    let (matched, params) = resource_url.extract_matched_url("/v1/service/endpoint");

    assert!(matched);
    assert!(
        params.is_none(),
        "No parameters expected for a literal path"
    );
}

#[test]
fn test_parsed_url_with_multi_params_extracted() {
    // Path with two parameter placeholders should extract both values into a map.
    let content = "/v1/service/endpoint/{user_id}/{message}";
    let expected_params: Vec<String> = vec!["user_id".into(), "message".into()];

    let resource_url = SimpleUrl::url_with_query(content);

    // Basic structural expectations about the constructed resource descriptor
    assert_eq!(resource_url.url, content);
    assert_eq!(resource_url.queries, None);
    assert_eq!(resource_url.params, Some(expected_params));
    assert!(
        resource_url.matcher.is_some(),
        "A matcher must be built for parameterized paths"
    );

    // Match a concrete path and verify extracted parameter values
    let (matched, params_opt) = resource_url.extract_matched_url("/v1/service/endpoint/123/hello");
    assert!(
        matched,
        "The concrete path should match the parameterized pattern"
    );
    assert!(
        params_opt.is_some(),
        "Parameters should be extracted for a matched parameterized path"
    );

    let params = params_opt.unwrap();
    let mut expected_map: BTreeMap<String, String> = BTreeMap::new();
    expected_map.insert("user_id".into(), "123".into());
    expected_map.insert("message".into(), "hello".into());

    assert_eq!(params, expected_map);
}

#[test]
fn test_parsed_url_with_optional_query_component() {
    // When the resource is created using url_with_query, a content containing a
    // query-like suffix should keep queries in the SimpleUrl representation.
    let content = "/search";
    let resource_url = SimpleUrl::url_with_query(content);

    // The `queries` field defaults to None when no explicit query template is present.
    assert_eq!(resource_url.queries, None);

    // Matching a path without queries should still succeed.
    let (matched, params) = resource_url.extract_matched_url("/search");
    assert!(matched);
    assert!(params.is_none());
}

#[test]
fn test_path_and_query_empty() {
    let pq = PathAndQuery::parse("").unwrap();
    assert_eq!(pq.path(), "/");
    assert_eq!(pq.query(), None);
}

#[test]
fn test_path_and_query_root() {
    let pq = PathAndQuery::parse("/").unwrap();
    assert_eq!(pq.path(), "/");
    assert_eq!(pq.query(), None);
}

#[test]
fn test_path_and_query_path_only() {
    let pq = PathAndQuery::parse("/path/to/resource").unwrap();
    assert_eq!(pq.path(), "/path/to/resource");
    assert_eq!(pq.query(), None);
}

#[test]
fn test_path_and_query_with_query() {
    let pq = PathAndQuery::parse("/path?key=value").unwrap();
    assert_eq!(pq.path(), "/path");
    assert_eq!(pq.query(), Some("key=value"));
}

#[test]
fn test_path_and_query_with_complex_query() {
    let pq = PathAndQuery::parse("/path?key=value&foo=bar&baz").unwrap();
    assert_eq!(pq.path(), "/path");
    assert_eq!(pq.query(), Some("key=value&foo=bar&baz"));
}

#[test]
fn test_path_and_query_empty_query() {
    let pq = PathAndQuery::parse("/path?").unwrap();
    assert_eq!(pq.path(), "/path");
    assert_eq!(pq.query(), None); // Empty query is treated as no query
}

#[test]
fn test_path_and_query_relative_path() {
    let pq = PathAndQuery::parse("path/to/resource").unwrap();
    assert_eq!(pq.path(), "/path/to/resource"); // Prepends /
}

#[test]
fn test_path_and_query_display() {
    let pq = PathAndQuery::parse("/path?key=value").unwrap();
    assert_eq!(pq.to_string(), "/path?key=value");
}

#[test]
fn test_path_validation() {
    // Valid paths
    assert!(PathAndQuery::parse("/path/to/resource").is_ok());
    assert!(PathAndQuery::parse("/path-with-dash").is_ok());
    assert!(PathAndQuery::parse("/path_with_underscore").is_ok());
    assert!(PathAndQuery::parse("/path.with.dots").is_ok());
    assert!(PathAndQuery::parse("/path~tilde").is_ok());
    assert!(PathAndQuery::parse("/path:colon").is_ok());
    assert!(PathAndQuery::parse("/path@at").is_ok());

    // Invalid paths - uncomment when strict validation is needed
    // assert!(PathAndQuery::parse("/path with space").is_err());
    // assert!(PathAndQuery::parse("/path<bracket>").is_err());
}
