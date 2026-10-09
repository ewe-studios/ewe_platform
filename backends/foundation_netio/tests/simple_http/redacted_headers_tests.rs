//! `RedactedHeaders` — credentials must never reach logs.
//!
//! Header logging previously wrote every value with `{:?}`, so a
//! `Bearer <token>` in `Authorization` (HuggingFace/OpenAI/Anthropic) landed in
//! plaintext wherever that tracing was enabled. `RedactedHeaders` renders the
//! map with sensitive values replaced by `[REDACTED]`.

use std::collections::BTreeMap;

use foundation_netio::shared::http::{RedactedHeaders, SimpleHeader, SimpleHeaders};

fn headers(pairs: Vec<(SimpleHeader, &str)>) -> SimpleHeaders {
    let mut m: SimpleHeaders = BTreeMap::new();
    for (k, v) in pairs {
        m.entry(k).or_default().push(v.to_string());
    }
    m
}

#[test]
fn authorization_value_is_redacted() {
    let h = headers(vec![
        (SimpleHeader::AUTHORIZATION, "Bearer hf_secretTOKENvalue123"),
        (SimpleHeader::CONTENT_TYPE, "application/json"),
    ]);
    let rendered = format!("{:?}", RedactedHeaders(&h));
    assert!(
        !rendered.contains("hf_secretTOKENvalue123"),
        "the token must not appear: {rendered}"
    );
    assert!(rendered.contains("[REDACTED]"), "value must be redacted: {rendered}");
    // Non-sensitive headers still render their value.
    assert!(
        rendered.contains("application/json"),
        "content-type must still be visible: {rendered}"
    );
}

#[test]
fn cookies_and_proxy_auth_are_redacted() {
    let h = headers(vec![
        (SimpleHeader::COOKIE, "session=abc123"),
        (SimpleHeader::SET_COOKIE, "session=abc123; HttpOnly"),
        (SimpleHeader::PROXY_AUTHORIZATION, "Basic dXNlcjpwYXNz"),
    ]);
    let rendered = format!("{:?}", RedactedHeaders(&h));
    for secret in ["abc123", "dXNlcjpwYXNz"] {
        assert!(!rendered.contains(secret), "secret '{secret}' leaked: {rendered}");
    }
}

#[test]
fn custom_credential_headers_are_redacted() {
    let h = headers(vec![
        (SimpleHeader::Custom("x-api-key".into()), "sk-live-SECRET"),
        (SimpleHeader::Custom("x-auth-token".into()), "tok_SECRET"),
        (SimpleHeader::Custom("x-request-id".into()), "req-123-visible"),
    ]);
    let rendered = format!("{:?}", RedactedHeaders(&h));
    assert!(!rendered.contains("sk-live-SECRET"), "api key leaked: {rendered}");
    assert!(!rendered.contains("tok_SECRET"), "auth token leaked: {rendered}");
    // A non-credential custom header stays visible.
    assert!(
        rendered.contains("req-123-visible"),
        "request id should not be redacted: {rendered}"
    );
}

#[test]
fn is_sensitive_classification() {
    assert!(SimpleHeader::AUTHORIZATION.is_sensitive());
    assert!(SimpleHeader::COOKIE.is_sensitive());
    assert!(SimpleHeader::Custom("X-Api-Key".into()).is_sensitive());
    assert!(!SimpleHeader::CONTENT_TYPE.is_sensitive());
    assert!(!SimpleHeader::Custom("x-trace-id".into()).is_sensitive());
}
