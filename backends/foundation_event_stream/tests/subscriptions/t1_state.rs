//! T1: configuration hash, generation and lease state.

use crate::subscriptions::support::{
    ack_request, append_stream, create_stream, harness, pull_request, webhook_request,
};
use foundation_core::valtron::block_on_future_scoped;
use foundation_event_stream::shared::{
    stable_config_hash, SubscriptionCreate, SubscriptionErrorCode, WebhookCreate,
};
use foundation_event_stream::SubscriptionType;

#[test]
fn config_hash_is_stable() {
    let config = SubscriptionCreate {
        kind: SubscriptionType::Webhook,
        pattern: Some("events/*".to_string()),
        streams: vec!["events/b".to_string(), "events/a".to_string()],
        webhook: Some(WebhookCreate {
            url: "https://example.com/hook".to_string(),
        }),
        wake_stream: None,
        lease_ttl_ms: 30_000,
        description: Some("processor".to_string()),
    };
    assert_eq!(
        stable_config_hash(&config),
        stable_config_hash(&config.clone())
    );

    let mut changed = config.clone();
    changed.lease_ttl_ms = 60_000;
    assert_ne!(stable_config_hash(&config), stable_config_hash(&changed));

    // The engine normalizes explicit streams, so their order and duplicates do not matter.
    let harness = harness();
    let mut first = webhook_request("events/*", "http://127.0.0.1:9999/hook", None);
    first.streams = vec!["events/b".to_string(), "events/a".to_string()];
    let mut second = first.clone();
    second.streams = vec!["events/a".to_string(), "events/b".to_string()];
    let base = "http://127.0.0.1:4437";
    let (_, created) =
        block_on_future_scoped(harness.engine.create_or_confirm("hash-1", first, base))
            .expect("first create");
    assert!(created);
    let (_, created) = block_on_future_scoped(harness.engine.create_or_confirm(
        "hash-2",
        second.clone(),
        base,
    ))
    .expect("second create");
    assert!(created);

    let (_, confirmed) =
        block_on_future_scoped(harness.engine.create_or_confirm("hash-1", second, base))
            .expect("confirm");
    assert!(!confirmed);
}

#[test]
fn generation_fences_stale_claim() {
    let harness = harness();
    create_stream(&harness, "v1/stream/wake/pool", true, Some(b"[]"));
    block_on_future_scoped(harness.engine.create_or_confirm(
        "fence-1",
        pull_request("events/*", "wake/pool", Some(60_000)),
        "http://127.0.0.1:4437",
    ))
    .expect("create subscription");
    create_stream(&harness, "v1/stream/events/a", true, Some(b"{\"event\":1}"));

    let first =
        block_on_future_scoped(harness.engine.claim("fence-1", "worker-1")).expect("first claim");
    let tail = first
        .streams
        .iter()
        .find(|stream| stream.path == "events/a")
        .expect("linked stream")
        .tail_offset
        .clone();
    block_on_future_scoped(harness.engine.ack(
        "fence-1",
        &first.token,
        ack_request(&first.wake_id, first.generation, "events/a", &tail, true),
    ))
    .expect("ack");

    append_stream(&harness, "v1/stream/events/a", b"{\"event\":2}");
    let second =
        block_on_future_scoped(harness.engine.claim("fence-1", "worker-1")).expect("second claim");
    assert_eq!(second.generation, first.generation + 1);

    let error = block_on_future_scoped(harness.engine.ack(
        "fence-1",
        &first.token,
        ack_request(&first.wake_id, first.generation, "events/a", &tail, true),
    ))
    .expect_err("the stale wake is refused");
    assert_eq!(error.code, SubscriptionErrorCode::Fenced);
}
