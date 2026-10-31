//! Auth middleware tests (moved from `src/shared/middleware/auth.rs`).

use std::sync::Arc;

use foundation_http::shared::context::ContextBag;
use foundation_http::shared::middleware::{
    AuthConfig, AuthMiddleware, MiddlewareResult, RequestMiddleware,
};
use foundation_http::{SimpleHeader, SimpleIncomingRequest, SimpleMethod};

fn bearer_request(token: &str) -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_plain_url("/")
        .with_method(SimpleMethod::GET)
        .add_header(SimpleHeader::AUTHORIZATION, format!("Bearer {token}"))
        .build()
        .expect("valid request")
}

fn basic_request(credentials: &str) -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_plain_url("/")
        .with_method(SimpleMethod::GET)
        .add_header(SimpleHeader::AUTHORIZATION, format!("Basic {credentials}"))
        .build()
        .expect("valid request")
}

#[test]
fn test_auth_config_new() {
    let config = AuthConfig::new();
    assert!(config.bearer_validator.is_none());
    assert!(config.basic_validator.is_none());
    assert!(config.public_paths.is_empty());
    assert_eq!(config.realm, "Protected");
}

#[test]
fn test_auth_config_with_bearer() {
    let config = AuthConfig::new().with_bearer_validator(|token| token == "valid-token");
    assert!(config.bearer_validator.is_some());
}

#[test]
fn test_auth_middleware_bearer_valid_token() {
    let mw = AuthMiddleware::bearer(|token| token == "valid-token");
    let ctx = Arc::new(ContextBag::new());
    let mut req = bearer_request("valid-token");
    assert!(matches!(mw.handle(&ctx, &mut req), MiddlewareResult::Continue));
}

#[test]
fn test_auth_middleware_bearer_invalid_token() {
    let mw = AuthMiddleware::bearer(|token| token == "valid-token");
    let ctx = Arc::new(ContextBag::new());
    let mut req = bearer_request("wrong-token");
    assert!(matches!(mw.handle(&ctx, &mut req), MiddlewareResult::Response(_)));
}

#[test]
fn test_auth_middleware_basic_valid_credentials() {
    let mw = AuthMiddleware::basic(|user, pass| user == "user" && pass == "pass");
    let ctx = Arc::new(ContextBag::new());
    // "dXNlcjpwYXNz" decodes to "user:pass".
    let mut req = basic_request("dXNlcjpwYXNz");
    assert!(matches!(mw.handle(&ctx, &mut req), MiddlewareResult::Continue));
}

#[test]
fn test_auth_middleware_basic_invalid_base64() {
    let mw = AuthMiddleware::basic(|user, pass| user == "user" && pass == "pass");
    let ctx = Arc::new(ContextBag::new());
    let mut req = basic_request("!!!not-base64!!!");
    assert!(matches!(mw.handle(&ctx, &mut req), MiddlewareResult::Response(_)));
}

#[test]
fn test_auth_middleware_basic_wrong_credentials() {
    let mw = AuthMiddleware::basic(|user, pass| user == "admin" && pass == "secret");
    let ctx = Arc::new(ContextBag::new());
    // Decodes to "user:pass", which fails the admin/secret validator.
    let mut req = basic_request("dXNlcjpwYXNz");
    assert!(matches!(mw.handle(&ctx, &mut req), MiddlewareResult::Response(_)));
}
