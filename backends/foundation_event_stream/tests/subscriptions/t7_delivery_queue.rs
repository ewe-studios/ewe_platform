//! T7: the webhook delivery queue leaves the append path.
//!
//! A create, a confirm and an append only queue the delivery. The worker runs it in `tick`, and
//! the injected clock drives the delivery timeout and the retry schedule.

use crate::subscriptions::support::{
    append_stream, create_stream, harness, harness_with, harness_with_clock, webhook_request,
    RecordingSender, TestClock, WebhookOutcome,
};
use foundation_core::valtron::block_on_future_scoped;
use foundation_event_stream::shared::{
    compare_offsets, SubscriptionConfig, SubscriptionStatus, WakeNotification, WebhookFuture,
    WebhookRequest, WebhookResponse, WebhookSender,
};
use std::cmp::Ordering;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};

const BASE: &str = "http://127.0.0.1:4437";

/// A sender whose answer depends on the injected clock.
///
/// Before `release_at_ms` the receiver does not answer, and the call reports a transport error.
/// The old inline code awaited this call in the append. The queue never calls it there.
struct ClockGatedSender {
    clock: Arc<TestClock>,
    release_at_ms: i64,
    requests: Mutex<Vec<WebhookRequest>>,
}

impl ClockGatedSender {
    fn new(clock: Arc<TestClock>, stall_ms: i64) -> Arc<Self> {
        let release_at_ms = clock.millis().saturating_add(stall_ms);
        Arc::new(Self {
            clock,
            release_at_ms,
            requests: Mutex::new(Vec::new()),
        })
    }

    fn requests(&self) -> Vec<WebhookRequest> {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl WebhookSender for ClockGatedSender {
    fn send(&self, request: WebhookRequest) -> WebhookFuture<'_> {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(request);
        let released = self.clock.millis() >= self.release_at_ms;
        Box::pin(async move {
            if released {
                Ok(WebhookResponse {
                    status: 200,
                    body: b"{}".to_vec(),
                })
            } else {
                Err("the receiver stalled".to_string())
            }
        })
    }
}

/// A sender that moves the injected clock and never answers.
///
/// The worker must abandon the attempt when the delivery timeout passes, without a real wait.
struct StallingSender {
    clock: Arc<TestClock>,
    attempts: AtomicUsize,
}

impl WebhookSender for StallingSender {
    fn send(&self, _request: WebhookRequest) -> WebhookFuture<'_> {
        self.attempts.fetch_add(1, AtomicOrdering::SeqCst);
        let clock = self.clock.clone();
        Box::pin(async move {
            // The receiver does not answer. Time passes on the injected clock.
            clock.advance(2_000);
            core::future::pending::<Result<WebhookResponse, String>>().await
        })
    }
}

#[test]
fn stalled_receiver_does_not_delay_create_confirm_or_append() {
    let clock = Arc::new(TestClock::new());
    let sender = ClockGatedSender::new(clock.clone(), 5_000);
    let harness = harness_with_clock(
        clock.clone(),
        SubscriptionConfig::new("v1/stream"),
        sender.clone(),
    );
    let request = webhook_request("events/*", "http://127.0.0.1:9999/hook", Some(30_000));

    let (_, created) = block_on_future_scoped(harness.engine.create_or_confirm(
        "wh-stall",
        request.clone(),
        BASE,
    ))
    .expect("create");
    assert!(created);
    let (_, created) =
        block_on_future_scoped(harness.engine.create_or_confirm("wh-stall", request, BASE))
            .expect("confirm");
    assert!(!created);
    assert!(
        sender.requests().is_empty(),
        "create and confirm must not call the receiver"
    );

    create_stream(&harness, "v1/stream/events/a", true, Some(b"{\"event\":1}"));
    assert!(
        sender.requests().is_empty(),
        "the append must return before the delivery"
    );
    let record = harness
        .engine
        .store()
        .get("wh-stall")
        .expect("stored record");
    assert!(record.wake_id.is_some(), "the wake is pending");
    assert_eq!(record.status, SubscriptionStatus::Active);

    // The receiver answers only after the injected clock moves.
    clock.advance(5_000);
    block_on_future_scoped(harness.engine.tick());
    assert_eq!(sender.requests().len(), 1, "the worker delivers");

    let record = harness
        .engine
        .store()
        .get("wh-stall")
        .expect("stored record");
    assert_eq!(record.status, SubscriptionStatus::Active);
    assert_eq!(record.retry_count, 0);
}

#[test]
fn failed_receiver_stops_at_the_retry_bound() {
    let config = SubscriptionConfig::new("v1/stream")
        .with_retry_delay_range_ms(100, 100)
        .with_retry_jitter_percent(0)
        .with_delivery_retry_bound(3);
    let harness = harness_with(config, RecordingSender::new());
    for _ in 0..10 {
        harness.sender.push(WebhookOutcome::TransportError);
    }
    block_on_future_scoped(harness.engine.create_or_confirm(
        "wh-bound",
        webhook_request("events/*", "http://127.0.0.1:9999/hook", Some(30_000)),
        BASE,
    ))
    .expect("create");
    create_stream(&harness, "v1/stream/events/a", true, Some(b"{\"event\":1}"));

    for _ in 0..6 {
        block_on_future_scoped(harness.engine.tick());
        harness.clock.advance(1_000);
    }

    let record = harness
        .engine
        .store()
        .get("wh-bound")
        .expect("stored record");
    assert_eq!(record.retry_count, 3);
    assert_eq!(record.status, SubscriptionStatus::Failed);
    assert!(record.next_attempt_at_ms.is_none());
    assert!(record.wake_id.is_none());
    assert_eq!(harness.sender.requests().len(), 3);

    harness.clock.advance(60_000);
    block_on_future_scoped(harness.engine.tick());
    assert_eq!(
        harness.sender.requests().len(),
        3,
        "the engine must not retry past the bound"
    );
}

#[test]
fn two_deliveries_for_one_subscription_arrive_in_order() {
    let harness = harness();
    harness
        .sender
        .push(WebhookOutcome::Response(200, b"{\"done\":true}".to_vec()));
    harness
        .sender
        .push(WebhookOutcome::Response(200, b"{\"done\":true}".to_vec()));
    block_on_future_scoped(harness.engine.create_or_confirm(
        "wh-order",
        webhook_request("events/*", "http://127.0.0.1:9999/hook", Some(30_000)),
        BASE,
    ))
    .expect("create");
    create_stream(&harness, "v1/stream/events/a", true, Some(b"{\"event\":1}"));
    append_stream(&harness, "v1/stream/events/a", b"{\"event\":2}");

    block_on_future_scoped(harness.engine.tick());

    let requests = harness.sender.requests();
    assert_eq!(requests.len(), 2, "one delivery for each wake");
    let first: WakeNotification =
        serde_json::from_slice(&requests[0].body).expect("first wake body");
    let second: WakeNotification =
        serde_json::from_slice(&requests[1].body).expect("second wake body");
    assert_eq!(first.generation, 1);
    assert_eq!(second.generation, 2);
    assert_ne!(first.wake_id, second.wake_id);

    let first_acked = first
        .streams
        .iter()
        .find(|stream| stream.path == "events/a")
        .expect("first linked stream")
        .acked_offset
        .clone();
    let second_acked = second
        .streams
        .iter()
        .find(|stream| stream.path == "events/a")
        .expect("second linked stream")
        .acked_offset
        .clone();
    assert_eq!(
        compare_offsets(&second_acked, &first_acked),
        Ordering::Greater,
        "the second wake follows the first in stream order"
    );
}

#[test]
fn a_full_queue_delivers_the_dropped_wake_on_the_next_tick() {
    let config = SubscriptionConfig::new("v1/stream").with_delivery_queue_bound(1);
    let harness = harness_with(config, RecordingSender::new());
    for id in ["wh-first", "wh-second"] {
        block_on_future_scoped(harness.engine.create_or_confirm(
            id,
            webhook_request(
                "events/*",
                &format!("http://127.0.0.1:9999/{id}"),
                Some(30_000),
            ),
            BASE,
        ))
        .expect("create");
    }
    create_stream(&harness, "v1/stream/events/a", true, Some(b"{\"event\":1}"));

    assert_eq!(
        harness.engine.delivery_drops(),
        1,
        "the second wake meets the queue bound"
    );

    block_on_future_scoped(harness.engine.tick());
    let requests = harness.sender.requests();
    assert_eq!(requests.len(), 1, "the first tick delivers one wake");
    let first: WakeNotification = serde_json::from_slice(&requests[0].body).expect("first body");
    assert_eq!(first.subscription_id, "wh-first");

    // The clock does not move, so no lease expires. The sweep queued the dropped wake, and the
    // next tick delivers it.
    block_on_future_scoped(harness.engine.tick());
    let requests = harness.sender.requests();
    assert_eq!(requests.len(), 2, "the next tick delivers the dropped wake");
    let second: WakeNotification = serde_json::from_slice(&requests[1].body).expect("second body");
    assert_eq!(second.subscription_id, "wh-second");

    // Each open wake is delivered once. The sweep must not repeat a wake that waits for its
    // callback.
    block_on_future_scoped(harness.engine.tick());
    assert_eq!(
        harness.sender.requests().len(),
        2,
        "the sweep does not repeat an open wake"
    );
}

#[test]
fn the_sweep_waits_for_the_next_retry_time() {
    let config = SubscriptionConfig::new("v1/stream")
        .with_retry_delay_range_ms(10_000, 10_000)
        .with_retry_jitter_percent(0);
    let harness = harness_with(config, RecordingSender::new());
    harness.sender.push(WebhookOutcome::TransportError);
    harness.sender.push(WebhookOutcome::TransportError);
    block_on_future_scoped(harness.engine.create_or_confirm(
        "wh-retry",
        webhook_request("events/*", "http://127.0.0.1:9999/hook", Some(30_000)),
        BASE,
    ))
    .expect("create");
    create_stream(&harness, "v1/stream/events/a", true, Some(b"{\"event\":1}"));

    assert_eq!(harness.engine.delivery_drops(), 0);

    block_on_future_scoped(harness.engine.tick());
    assert_eq!(harness.sender.requests().len(), 1, "the first attempt runs");
    let record = harness
        .engine
        .store()
        .get("wh-retry")
        .expect("stored record");
    assert!(record.wake_id.is_some(), "the failed wake stays open");
    let next_attempt = record.next_attempt_at_ms.expect("scheduled retry");
    assert!(next_attempt > harness.clock.millis());

    block_on_future_scoped(harness.engine.tick());
    assert_eq!(
        harness.sender.requests().len(),
        1,
        "the sweep must not queue a wake before its next attempt time"
    );

    harness.clock.advance(10_000);
    block_on_future_scoped(harness.engine.tick());
    assert_eq!(
        harness.sender.requests().len(),
        2,
        "the due retry runs after the retry time"
    );
}

#[test]
fn stalled_send_times_out_on_the_injected_clock() {
    let clock = Arc::new(TestClock::new());
    let sender = Arc::new(StallingSender {
        clock: clock.clone(),
        attempts: AtomicUsize::new(0),
    });
    let config = SubscriptionConfig::new("v1/stream").with_delivery_timeout_ms(1_000);
    let harness = harness_with_clock(clock, config, sender.clone());
    block_on_future_scoped(harness.engine.create_or_confirm(
        "wh-timeout",
        webhook_request("events/*", "http://127.0.0.1:9999/hook", Some(30_000)),
        BASE,
    ))
    .expect("create");
    create_stream(&harness, "v1/stream/events/a", true, Some(b"{\"event\":1}"));

    block_on_future_scoped(harness.engine.tick());

    assert_eq!(sender.attempts.load(AtomicOrdering::SeqCst), 1);
    let record = harness
        .engine
        .store()
        .get("wh-timeout")
        .expect("stored record");
    assert_eq!(record.status, SubscriptionStatus::Failed);
    assert_eq!(record.retry_count, 1);
    assert!(record.next_attempt_at_ms.is_some());
}
