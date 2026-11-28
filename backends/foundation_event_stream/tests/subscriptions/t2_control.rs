//! T2: the control operations.

use crate::subscriptions::support::{harness, webhook_request};
use foundation_core::valtron::block_on_future_scoped;
use foundation_event_stream::shared::{LinkType, SubscriptionErrorCode};
use foundation_event_stream::SubscriptionType;

const BASE: &str = "http://127.0.0.1:4437";

#[test]
fn idempotent_confirm() {
    let harness = harness();
    let request = webhook_request("events/*", "http://127.0.0.1:9999/hook", Some(1_000));
    let (view, created) = block_on_future_scoped(harness.engine.create_or_confirm(
        "sub-1",
        request.clone(),
        BASE,
    ))
    .expect("create");
    assert!(created);
    assert_eq!(view.id, "sub-1");
    assert_eq!(view.kind, SubscriptionType::Webhook);
    assert_eq!(
        view.webhook.as_ref().map(|webhook| webhook.url.as_str()),
        Some("http://127.0.0.1:9999/hook")
    );
    assert_eq!(
        view.webhook
            .as_ref()
            .map(|webhook| webhook.signing.alg.as_str()),
        Some("ed25519")
    );

    let (confirmed, created) =
        block_on_future_scoped(harness.engine.create_or_confirm("sub-1", request, BASE))
            .expect("confirm");
    assert!(!created);
    assert_eq!(confirmed.id, "sub-1");

    let mut different = webhook_request("events/*", "http://127.0.0.1:9999/hook", Some(2_000));
    different.description = Some("changed".to_string());
    let error = block_on_future_scoped(harness.engine.create_or_confirm("sub-1", different, BASE))
        .expect_err("a different configuration conflicts");
    assert_eq!(error.code, SubscriptionErrorCode::SubscriptionAlreadyExists);
}

#[test]
fn add_and_remove_streams() {
    let harness = harness();
    let mut request = webhook_request("", "http://127.0.0.1:9999/hook", None);
    request.pattern = None;
    request.streams = vec!["manual/a".to_string()];
    block_on_future_scoped(harness.engine.create_or_confirm("sub-2", request, BASE))
        .expect("create");

    block_on_future_scoped(
        harness
            .engine
            .add_explicit_streams("sub-2", vec!["manual/b".to_string()]),
    )
    .expect("add");

    let view = block_on_future_scoped(harness.engine.read("sub-2")).expect("read");
    assert!(view
        .streams
        .iter()
        .any(|stream| stream.path == "manual/b" && stream.link_type == LinkType::Explicit));

    block_on_future_scoped(harness.engine.remove_explicit_stream("sub-2", "manual/b"))
        .expect("remove");
    block_on_future_scoped(harness.engine.remove_explicit_stream("sub-2", "manual/b"))
        .expect("remove is idempotent");

    let view = block_on_future_scoped(harness.engine.read("sub-2")).expect("read");
    assert!(view
        .streams
        .iter()
        .any(|stream| stream.path == "manual/a" && stream.link_type == LinkType::Explicit));
    assert!(!view.streams.iter().any(|stream| stream.path == "manual/b"));

    let error = block_on_future_scoped(harness.engine.add_explicit_streams("missing", vec![]))
        .expect_err("unknown subscription");
    assert_eq!(error.code, SubscriptionErrorCode::SubscriptionNotFound);
}
