//! T3: signed webhook delivery, retry and the failed status.

use crate::subscriptions::support::{create_stream, harness, webhook_request, WebhookOutcome};
use foundation_core::valtron::block_on_future_scoped;
use foundation_event_stream::shared::{SubscriptionStatus, WakeNotification};

const BASE: &str = "http://127.0.0.1:4437";

#[test]
fn webhook_signature_verifies() {
    let harness = harness();
    let (view, _) = block_on_future_scoped(harness.engine.create_or_confirm(
        "wh-1",
        webhook_request("events/*", "http://127.0.0.1:9999/hook", Some(1_000)),
        BASE,
    ))
    .expect("create");
    let kid = view
        .webhook
        .as_ref()
        .map(|webhook| webhook.signing.kid.clone())
        .expect("signing key id");

    create_stream(
        &harness,
        "v1/stream/events/a",
        true,
        Some(b"{\"event\":\"created\"}"),
    );
    let request = harness.sender.last_request().expect("webhook sent");
    let header = request
        .headers
        .iter()
        .find(|(name, _)| name == "webhook-signature")
        .map(|(_, value)| value.clone())
        .expect("signature header");

    let mut timestamp = None;
    let mut signature = None;
    let mut header_kid = None;
    for part in header.split(',') {
        if let Some(value) = part.strip_prefix("t=") {
            timestamp = value.parse::<i64>().ok();
        } else if let Some(value) = part.strip_prefix("kid=") {
            header_kid = Some(value.to_string());
        } else if let Some(value) = part.strip_prefix("ed25519=") {
            signature = Some(value.to_string());
        }
    }
    assert_eq!(header_kid.as_deref(), Some(kid.as_str()));
    assert!(harness.engine.signing().verify(
        timestamp.expect("timestamp"),
        &request.body,
        &signature.expect("signature"),
    ));

    let body: WakeNotification = serde_json::from_slice(&request.body).expect("wake body");
    assert_eq!(body.subscription_id, "wh-1");
    assert_eq!(
        body.callback_url,
        "http://127.0.0.1:4437/v1/stream/__ds/subscriptions/wh-1/callback"
    );
    assert!(body
        .streams
        .iter()
        .any(|stream| stream.path == "events/a" && stream.has_pending));
}

#[test]
fn failed_delivery_sets_failed() {
    let harness = harness();
    harness.sender.push(WebhookOutcome::TransportError);
    block_on_future_scoped(harness.engine.create_or_confirm(
        "wh-2",
        webhook_request("events/*", "http://127.0.0.1:9999/hook", Some(1_000)),
        BASE,
    ))
    .expect("create");
    create_stream(
        &harness,
        "v1/stream/events/fail",
        true,
        Some(b"{\"event\":1}"),
    );

    let record = harness.engine.store().get("wh-2").expect("stored record");
    assert_eq!(record.status, SubscriptionStatus::Failed);
    assert_eq!(record.retry_count, 1);
    assert!(record.next_attempt_at_ms.is_some());

    harness
        .sender
        .push(WebhookOutcome::Response(200, b"{\"done\":true}".to_vec()));
    harness.clock.advance(120_000);
    block_on_future_scoped(harness.engine.retry_due(harness.clock.millis()));

    let record = harness.engine.store().get("wh-2").expect("stored record");
    assert_eq!(record.status, SubscriptionStatus::Active);
    assert_eq!(harness.sender.requests().len(), 2);
}
