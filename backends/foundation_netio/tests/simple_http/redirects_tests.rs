//! Redirect follow-up request building and sensitive-header stripping tests
//! (moved from `src/shared/client/redirects.rs`).

use std::collections::BTreeMap;

use foundation_core::url::Uri;
use foundation_netio::shared::client::{
    build_followup_request_from_request_descriptor, strip_sensitive_headers_for_redirect,
};
use foundation_netio::shared::http::{
    Proto, RequestDescriptor, SimpleHeader, SimpleHeaders, SimpleMethod, SimpleUrl,
};

fn create_headers_with_auth_and_cookie() -> SimpleHeaders {
    let mut headers: SimpleHeaders = BTreeMap::new();
    headers.insert(SimpleHeader::AUTHORIZATION, vec!["Bearer token123".into()]);
    headers.insert(SimpleHeader::COOKIE, vec!["session=abc123".into()]);
    headers.insert(SimpleHeader::CONTENT_TYPE, vec!["application/json".into()]);
    headers
}

fn descriptor_with_headers(url: &str, headers: SimpleHeaders) -> RequestDescriptor {
    let uri = Uri::parse(url).expect("valid test url");
    RequestDescriptor {
        proto: Proto::HTTP11,
        request_url: SimpleUrl::url_with_query(uri.to_string()),
        request_uri: uri,
        headers,
        method: SimpleMethod::GET,
    }
}

#[test]
fn test_followup_strips_expect_header() {
    let mut headers: SimpleHeaders = BTreeMap::new();
    headers.insert(SimpleHeader::EXPECT, vec!["100-continue".into()]);
    headers.insert(SimpleHeader::HOST, vec!["origin.example.com".into()]);
    let original = descriptor_with_headers("https://origin.example.com/file", headers);

    let new_url = Uri::parse("https://cdn.example.com/file").expect("valid url");
    let followup = build_followup_request_from_request_descriptor(&original, new_url, true, false)
        .expect("followup builds");

    assert!(
        !followup.headers.contains_key(&SimpleHeader::EXPECT),
        "Expect header must not be forwarded on a bodyless redirect follow-up"
    );
}

#[test]
fn test_followup_rewrites_host_header() {
    let mut headers: SimpleHeaders = BTreeMap::new();
    headers.insert(SimpleHeader::HOST, vec!["origin.example.com".into()]);
    let original = descriptor_with_headers("https://origin.example.com/file", headers);

    let new_url = Uri::parse("https://cdn.example.com:8443/file").expect("valid url");
    let followup = build_followup_request_from_request_descriptor(&original, new_url, true, false)
        .expect("followup builds");

    assert_eq!(
        followup.headers.get(&SimpleHeader::HOST),
        Some(&vec!["cdn.example.com:8443".to_string()]),
        "Host header must be rewritten to the redirect target (with non-default port)"
    );
}

#[test]
fn test_strip_sensitive_headers_same_host() {
    let mut headers = create_headers_with_auth_and_cookie();
    strip_sensitive_headers_for_redirect(&mut headers, "example.com", "example.com", false, false);

    // Same host - nothing should be stripped
    assert!(headers.contains_key(&SimpleHeader::AUTHORIZATION));
    assert!(headers.contains_key(&SimpleHeader::COOKIE));
    assert!(headers.contains_key(&SimpleHeader::CONTENT_TYPE));
}

#[test]
fn test_strip_sensitive_headers_different_host_default_behavior() {
    let mut headers = create_headers_with_auth_and_cookie();
    strip_sensitive_headers_for_redirect(
        &mut headers,
        "example.com",
        "cdn.example.com",
        false,
        false,
    );

    // Different host with preserve_auth=false, preserve_cookies=false
    assert!(
        !headers.contains_key(&SimpleHeader::AUTHORIZATION),
        "Authorization should be stripped"
    );
    assert!(
        !headers.contains_key(&SimpleHeader::COOKIE),
        "Cookie should be stripped"
    );
    assert!(
        headers.contains_key(&SimpleHeader::CONTENT_TYPE),
        "Content-Type should remain"
    );
}

#[test]
fn test_strip_sensitive_headers_preserve_auth_only() {
    let mut headers = create_headers_with_auth_and_cookie();
    strip_sensitive_headers_for_redirect(
        &mut headers,
        "example.com",
        "cdn.example.com",
        true,
        false,
    );

    // Different host with preserve_auth=true, preserve_cookies=false
    assert!(
        headers.contains_key(&SimpleHeader::AUTHORIZATION),
        "Authorization should be preserved"
    );
    assert!(
        !headers.contains_key(&SimpleHeader::COOKIE),
        "Cookie should be stripped"
    );
}

#[test]
fn test_strip_sensitive_headers_preserve_cookies_only() {
    let mut headers = create_headers_with_auth_and_cookie();
    strip_sensitive_headers_for_redirect(
        &mut headers,
        "example.com",
        "cdn.example.com",
        false,
        true,
    );

    // Different host with preserve_auth=false, preserve_cookies=true
    assert!(
        !headers.contains_key(&SimpleHeader::AUTHORIZATION),
        "Authorization should be stripped"
    );
    assert!(
        headers.contains_key(&SimpleHeader::COOKIE),
        "Cookie should be preserved"
    );
}

#[test]
fn test_strip_sensitive_headers_preserve_both() {
    let mut headers = create_headers_with_auth_and_cookie();
    strip_sensitive_headers_for_redirect(
        &mut headers,
        "example.com",
        "cdn.example.com",
        true,
        true,
    );

    // Different host with preserve_auth=true, preserve_cookies=true
    assert!(
        headers.contains_key(&SimpleHeader::AUTHORIZATION),
        "Authorization should be preserved"
    );
    assert!(
        headers.contains_key(&SimpleHeader::COOKIE),
        "Cookie should be preserved"
    );
}

#[test]
fn test_redirect_same_host_other_port_strips_auth() {
    let mut headers = create_headers_with_auth_and_cookie();
    headers.insert(SimpleHeader::HOST, vec!["example.com".into()]);
    let original = descriptor_with_headers("https://example.com/a", headers);
    let new_url = Uri::parse("https://example.com:8443/b").expect("valid url");
    let followup = build_followup_request_from_request_descriptor(&original, new_url, false, false)
        .expect("followup builds");

    // Another port on the same host is another origin.
    assert!(
        !followup.headers.contains_key(&SimpleHeader::AUTHORIZATION),
        "Authorization must be stripped for another port"
    );
    assert!(
        !followup.headers.contains_key(&SimpleHeader::COOKIE),
        "Cookie must be stripped for another port"
    );
}

#[test]
fn test_redirect_scheme_change_strips_auth() {
    let mut headers = create_headers_with_auth_and_cookie();
    headers.insert(SimpleHeader::HOST, vec!["example.com:8443".into()]);
    let original = descriptor_with_headers("https://example.com:8443/a", headers);
    let new_url = Uri::parse("http://example.com:8443/b").expect("valid url");
    let followup = build_followup_request_from_request_descriptor(&original, new_url, false, false)
        .expect("followup builds");

    // The same host and port on another scheme is another origin.
    assert!(
        !followup.headers.contains_key(&SimpleHeader::AUTHORIZATION),
        "Authorization must be stripped for another scheme"
    );
    assert!(
        !followup.headers.contains_key(&SimpleHeader::COOKIE),
        "Cookie must be stripped for another scheme"
    );
}

#[test]
fn test_redirect_same_origin_keeps_auth() {
    let headers = create_headers_with_auth_and_cookie();
    let original = descriptor_with_headers("https://example.com/a", headers);

    // The default port and the explicit default port are the same origin, and
    // the host comparison is not case sensitive.
    let new_url = Uri::parse("https://EXAMPLE.com:443/b").expect("valid url");
    let followup = build_followup_request_from_request_descriptor(&original, new_url, false, false)
        .expect("followup builds");

    assert!(
        followup.headers.contains_key(&SimpleHeader::AUTHORIZATION),
        "Authorization stays in the same origin"
    );
    assert!(
        followup.headers.contains_key(&SimpleHeader::COOKIE),
        "Cookie stays in the same origin"
    );
}
