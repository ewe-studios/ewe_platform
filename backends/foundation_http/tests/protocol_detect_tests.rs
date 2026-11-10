//! Protocol detection reports a refusal that never reached the peer.
//!
//! WHY: the refusal write discarded its outcome (`let _ = write.await`), so a
//! client that never received the 505 left no trace of the failure. The write
//! now logs the outcome with the peer (GitHub issue #376).
//!
//! HOW: a server that speaks HTTP/2 only refuses an HTTP/1.1 request with 505.
//! The fake overlay client is closed first, so that refusal cannot be written.
//! The refusal runs on a pool worker, so the test records events with its own
//! global subscriber: `tracing-test` keeps only the events of the test thread.

#![cfg(not(target_family = "wasm"))]

mod fake_overlay;

use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use foundation_core::synca::WaitGroup;
use foundation_core::valtron::{initialize_pool, Ctx, MonotonicClock, TaskIterator};
use foundation_http::native::app::ServerApp;
use foundation_http::native::server::protocol_detect::ProtocolDetectHandler;
use foundation_http::native::server::KeepAliveConfig;
use foundation_http::shared::app::HttpApp;
use foundation_http::ShutdownSignal;
use foundation_netio::netcap::ConnectionContext;
use tracing::field::{Field, Visit};
use tracing::{Event, Metadata, Subscriber};

/// Recorded events, each as `target: field=value ...`.
static EVENTS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Installs [`Recording`] once, since the global subscriber can be set only once.
static INSTALL: Once = Once::new();

/// Records the crate's events, including those on pool worker threads.
struct Recording;

impl Subscriber for Recording {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target().starts_with("foundation_http")
    }

    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut fields = Fields(Vec::new());
        event.record(&mut fields);
        let line = fields
            .0
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(" ");
        EVENTS
            .lock()
            .expect("event store")
            .push(format!("{}: {line}", event.metadata().target()));
    }

    fn enter(&self, _: &tracing::span::Id) {}

    fn exit(&self, _: &tracing::span::Id) {}
}

struct Fields(Vec<(String, String)>);

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0
            .push((field.name().to_string(), format!("{value:?}")));
    }
}

fn events_contain_all(needles: [&str; 2]) -> bool {
    EVENTS
        .lock()
        .expect("event store")
        .iter()
        .any(|line| needles.iter().all(|needle| line.contains(needle)))
}

#[test]
fn a_failed_refusal_write_is_logged_with_its_peer() {
    INSTALL.call_once(|| {
        tracing::subscriber::set_global_default(Recording).expect("install the event recorder");
    });
    EVENTS.lock().expect("event store").clear();

    let _pool = initialize_pool(912, Some(2));

    let (stream, client) = fake_overlay::overlay_pair(1024);
    // HTTP/2 only: an HTTP/1.1 request is refused with 505.
    let app = ServerApp::http2(HttpApp::new_h2_serve());
    let shutdown = Arc::new(ShutdownSignal::new());
    let waits = WaitGroup::new();
    waits.add(1);
    let mut detector = ProtocolDetectHandler::new(
        stream,
        app,
        "127.0.0.1:0".to_string(),
        Arc::new(ConnectionContext::default()),
        shutdown,
        waits.guard(),
        KeepAliveConfig::defaults(),
        None,
        Ctx::fixed(Arc::new(MonotonicClock::new())),
    );

    client.queue_request(b"GET / HTTP/1.1\r\nHost: test\r\n\r\n");
    // The peer is gone before the refusal is written, so the write fails.
    client.close();

    // The detector decides the request is HTTP/1.1 and submits the refusal.
    assert!(
        detector.next_status(Ctx::host()).is_none(),
        "the detector hands the connection to the refusal writer"
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    while !events_contain_all(["refusal response write failed", "client=127.0.0.1:0"]) {
        assert!(
            Instant::now() < deadline,
            "the failed refusal write was not logged with its peer"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
