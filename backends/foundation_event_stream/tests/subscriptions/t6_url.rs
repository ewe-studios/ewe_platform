//! T6: unsafe webhook uniform resource locators (URLs) are refused.

use crate::subscriptions::support::{harness, webhook_request};
use foundation_core::valtron::block_on_future_scoped;
use foundation_event_stream::shared::{validate_webhook_url, SubscriptionErrorCode};

#[test]
fn rejects_unsafe_webhook_urls() {
    assert!(validate_webhook_url("http://10.0.0.1/hook").is_err());
    assert!(validate_webhook_url("http://192.168.1.10/hook").is_err());
    assert!(validate_webhook_url("http://169.254.169.254/latest").is_err());
    assert!(validate_webhook_url("https://192.168.1.10/hook").is_err());
    assert!(validate_webhook_url("https://localhost/hook").is_err());
    assert!(validate_webhook_url("ftp://example.com/hook").is_err());
    assert!(validate_webhook_url("not a url").is_err());

    assert!(validate_webhook_url("http://127.0.0.1:9999/hook").is_ok());
    assert!(validate_webhook_url("http://127.0.0.8:9999/hook").is_ok());
    assert!(validate_webhook_url("http://localhost:9999/hook").is_ok());
    assert!(validate_webhook_url("https://worker.example.com/hook").is_ok());

    let harness = harness();
    let error = block_on_future_scoped(harness.engine.create_or_confirm(
        "unsafe-1",
        webhook_request("events/*", "http://10.0.0.1/hook", None),
        "http://127.0.0.1:4437",
    ))
    .expect_err("the engine refuses the unsafe URL");
    assert_eq!(error.code, SubscriptionErrorCode::WebhookUrlRejected);
}
