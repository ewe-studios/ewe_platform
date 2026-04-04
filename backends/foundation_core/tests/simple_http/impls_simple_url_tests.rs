use foundation_core::wire::simple_http::SimpleUrl;
use std::collections::BTreeMap;

#[test]
fn test_parsed_url_without_any_special_elements() {
    let content = "/v1/service/endpoint";
    let resource_url = SimpleUrl::url_with_query(content);
    let (matched, params) = resource_url.extract_matched_url("/v1/service/endpoint");

    assert!(matched);
    assert!(params.is_none());
}

#[test]
fn test_parsed_url_with_multi_params_extracted() {
    let content = "/v1/service/endpoint/{user_id}/{message}";

    let params: Vec<String> = vec!["user_id".into(), "message".into()];

    let resource_url = SimpleUrl::url_with_query(content);

    // Mirror original expectations from in-crate tests where fields are directly inspected.
    // These assertions may surface visibility issues that we'll address in the iteration phase.
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
