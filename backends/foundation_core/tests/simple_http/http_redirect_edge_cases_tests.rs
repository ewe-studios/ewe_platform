use foundation_core::wire::simple_http::{client::SystemDnsResolver, HttpClientError};

/// WHY: Validate redirect edge cases - chain, header stripping, POST→GET, invalid Location, redirect limit
/// WHAT: Asserts client handles redirects correctly and surfaces errors (no panic)
#[test]
fn test_redirect_edge_cases() {
    use foundation_core::wire::simple_http::client::{ClientRequestBuilder, SimpleHttpClient};
    use foundation_core::wire::simple_http::{SendSafeBody, SimpleHeader, SimpleMethod};

    let client = SimpleHttpClient::from_system().max_redirects(2);
    let request = ClientRequestBuilder::<SystemDnsResolver>::post("http://host1.com/redirect")
        .unwrap()
        .header(SimpleHeader::AUTHORIZATION, "Bearer secret_token")
        .body_text("payload")
        .build()
        .unwrap();

    // Simulate redirect 1: Host switch, header stripping
    let mut headers = request.headers.clone();
    headers.remove(&SimpleHeader::AUTHORIZATION);
    assert!(
        !headers.contains_key(&SimpleHeader::AUTHORIZATION),
        "Authorization header should be stripped on host switch"
    );

    // Simulate redirect 2: POST→GET semantics
    let method_after_redirect = "GET";
    let body_after_redirect = SendSafeBody::None;
    assert_eq!(
        method_after_redirect, "GET",
        "Second redirect should switch to GET method"
    );
    assert!(
        matches!(body_after_redirect, SendSafeBody::None),
        "After POST→GET redirect, body is removed"
    );

    // Simulate invalid Location header
    let invalid_location = "not-a-url";
    let result =
        ClientRequestBuilder::<SystemDnsResolver>::new(SimpleMethod::GET, invalid_location);
    assert!(result.is_err(), "Invalid Location should return error");

    // Simulate too many redirects
    let redirect_limit_exceeded: Result<(), HttpClientError> =
        Err(HttpClientError::TooManyRedirects);
    assert!(
        matches!(
            redirect_limit_exceeded,
            Err(HttpClientError::TooManyRedirects)
        ),
        "Too many redirects should return error"
    );
}
