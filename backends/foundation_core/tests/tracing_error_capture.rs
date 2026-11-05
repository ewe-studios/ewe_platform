#![cfg(not(target_family = "wasm"))]
// This test binary holds exactly one test on purpose. `tracing` keeps each
// callsite's interest cache process-wide. Another test thread in the same
// binary can reach the `tracing::error!` callsite with no subscriber and cache
// it as `never` again after this test calls `rebuild_interest_cache`, so the
// capture sees nothing. With one test and one thread, no other test can change
// the cache.

use foundation_core::io::readers::*;
use std::io::{self, Read};
use std::sync::{Arc, Mutex};
use tracing::Level;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Layer;

/// Records the target of every `ERROR` event it sees. Used to prove that a
/// `WouldBlock` read does not emit an error log.
struct ErrorCapture {
    targets: Arc<Mutex<Vec<String>>>,
}

impl<S> Layer<S> for ErrorCapture
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if event.metadata().level() == &Level::ERROR {
            self.targets
                .lock()
                .unwrap()
                .push(event.metadata().target().to_string());
        }
    }
}

/// WHY: Feature 09 moved `BatchReader`'s `WouldBlock` log from `error!` to
/// `trace!`. `WouldBlock` is normal back-pressure, not a failure, so it must not
/// emit an ERROR record.
/// WHAT: Read from a source that always returns `WouldBlock`, capture logs, and
/// assert no ERROR event fired. A source that returns a real error is then used
/// to prove the capture records ERROR events.
#[test]
fn batch_reader_would_block_is_not_an_error_log() {
    struct AlwaysWouldBlock;
    impl Read for AlwaysWouldBlock {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"))
        }
    }
    struct RealError;
    impl Read for RealError {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken"))
        }
    }

    let targets: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    tracing::subscriber::with_default(
        tracing_subscriber::registry().with(ErrorCapture {
            targets: targets.clone(),
        }),
        || {
            // `tracing` caches each callsite's interest process-wide. If another
            // test thread reached these callsites first with no subscriber, they
            // are cached as "never" and this thread-local capture sees nothing.
            // Re-evaluate the cache now that this subscriber is the default.
            tracing::callsite::rebuild_interest_cache();
            let reader = BatchReader::new(AlwaysWouldBlock).max_consecutive_retries(3);
            let _ = reader.count();
        },
    );
    assert!(
        targets.lock().unwrap().is_empty(),
        "WouldBlock must not emit ERROR records"
    );

    tracing::subscriber::with_default(
        tracing_subscriber::registry().with(ErrorCapture {
            targets: targets.clone(),
        }),
        || {
            tracing::callsite::rebuild_interest_cache();
            let reader = BatchReader::new(RealError);
            let _ = reader.count();
        },
    );
    assert!(
        !targets.lock().unwrap().is_empty(),
        "a real read error must emit an ERROR record (proves capture works)"
    );
}
