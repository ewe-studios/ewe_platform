//! T4: pull-wake claim, ack, release and lease expiry.

use crate::subscriptions::support::{
    ack_request, append_stream, create_stream, harness, pull_request,
};
use foundation_core::valtron::block_on_future_scoped;
use foundation_event_stream::shared::{AckResponse, SubscriptionErrorCode};

const BASE: &str = "http://127.0.0.1:4437";

fn pull_harness() -> crate::subscriptions::support::Harness {
    let harness = harness();
    create_stream(&harness, "v1/stream/wake/pool", true, Some(b"[]"));
    block_on_future_scoped(harness.engine.create_or_confirm(
        "pull-1",
        pull_request("events/*", "wake/pool", Some(1_000)),
        BASE,
    ))
    .expect("create pull-wake subscription");
    harness
}

#[test]
fn claim_then_ack_clears() {
    let harness = pull_harness();
    create_stream(
        &harness,
        "v1/stream/events/a",
        true,
        Some(b"{\"event\":\"created\"}"),
    );

    let claim = block_on_future_scoped(harness.engine.claim("pull-1", "worker-1")).expect("claim");
    assert_eq!(claim.generation, 1);
    let tail = claim
        .streams
        .iter()
        .find(|stream| stream.path == "events/a")
        .expect("linked stream")
        .tail_offset
        .clone();

    let ack = block_on_future_scoped(harness.engine.ack(
        "pull-1",
        &claim.token,
        ack_request(&claim.wake_id, claim.generation, "events/a", &tail, true),
    ))
    .expect("ack");
    assert_eq!(
        ack,
        AckResponse {
            ok: true,
            next_wake: false
        }
    );

    let error = block_on_future_scoped(harness.engine.claim("pull-1", "worker-1"))
        .expect_err("no pending work");
    assert_eq!(error.code, SubscriptionErrorCode::NoPendingWork);
}

#[test]
fn lease_expiry_reopens() {
    let harness = pull_harness();
    create_stream(
        &harness,
        "v1/stream/events/a",
        true,
        Some(b"{\"event\":\"created\"}"),
    );

    let first = block_on_future_scoped(harness.engine.claim("pull-1", "worker-1")).expect("claim");
    assert_eq!(first.generation, 1);

    harness.clock.advance(2_000);
    block_on_future_scoped(harness.engine.tick());

    let second = block_on_future_scoped(harness.engine.claim("pull-1", "worker-2"))
        .expect("claim after expiry");
    assert_eq!(second.generation, 2);
    assert_ne!(second.wake_id, first.wake_id);

    append_stream(&harness, "v1/stream/events/a", b"{\"event\":2}");
    let error = block_on_future_scoped(harness.engine.release(
        "pull-1",
        &first.token,
        ack_request(&first.wake_id, first.generation, "events/a", "0", false),
    ))
    .expect_err("the released token is stale");
    assert_eq!(error.code, SubscriptionErrorCode::Fenced);
}
