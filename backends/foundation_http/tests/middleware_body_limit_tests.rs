//! Body limit middleware tests (moved from `src/shared/middleware/body_limit.rs`).

use std::sync::Arc;

use foundation_http::shared::context::ContextBag;
use foundation_http::shared::middleware::{
    BodyLimitMiddleware, MiddlewareResult, RequestMiddleware,
};
use foundation_http::{
    SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleMethod, Status,
};

fn request_with_content_length(bytes: usize) -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_plain_url("/")
        .with_method(SimpleMethod::POST)
        .add_header(SimpleHeader::CONTENT_LENGTH, bytes.to_string())
        .build()
        .expect("valid request")
}

#[test]
fn test_body_limit_enforces_max_bytes() {
    let mw = BodyLimitMiddleware::new(1024);
    let ctx = Arc::new(ContextBag::new());

    // Exactly at the limit is allowed.
    let mut ok = request_with_content_length(1024);
    assert!(matches!(mw.handle(&ctx, &mut ok), MiddlewareResult::Continue));

    // One byte over the limit is rejected.
    let mut too_large = request_with_content_length(1025);
    match mw.handle(&ctx, &mut too_large) {
        MiddlewareResult::Response(resp) => assert_eq!(resp.status, Status::PayloadTooLarge),
        _ => panic!("expected a 413 response"),
    }
}

#[test]
fn test_body_limit_custom_message() {
    let mw = BodyLimitMiddleware::new(1024).with_message("Custom error");
    let ctx = Arc::new(ContextBag::new());

    let mut req = request_with_content_length(1025);
    match mw.handle(&ctx, &mut req) {
        MiddlewareResult::Response(resp) => match resp.body {
            Some(SendSafeBody::Text(text)) => assert_eq!(text, "Custom error"),
            _ => panic!("expected a text body"),
        },
        _ => panic!("expected a 413 response"),
    }
}
