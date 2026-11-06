// Proxy handlers: SseReloadHandler.

use std::sync::Arc;
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::{timer, Clock};
use foundation_http::native::upgrade::SseWriter;
use foundation_http::native::writer::{render_message, ResponseWriter, WriterConfig};
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ConnectionResult, Serve, ServeFactory, ServeOutcome};
use foundation_netio::event_source::{SseEvent, SseResponse};
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::SimpleIncomingRequest;

use concurrent_queue::ConcurrentQueue;

use crate::watcher::FileChange;

// -- SseReloadHandler

pub struct SseReloadHandler {
    reload_rx: Arc<ConcurrentQueue<FileChange>>,
    clock: Arc<dyn Clock>,
}

impl ServeFactory for SseReloadHandler {
    /// # Panics
    ///
    /// Panics when the bag has no reload queue or no clock. The host that
    /// builds the bag must store both; a missing clock is a design fault, not
    /// a reason to fall back to the real clock.
    fn create(bag: &ContextBag) -> Self {
        let tx = bag
            .get::<Arc<ConcurrentQueue<FileChange>>>()
            .expect("reload queue in ContextBag");
        // bag.get() returns Arc<T>, T is already Arc<ConcurrentQueue>
        // We need to get the inner Arc via deref
        let queue: &Arc<ConcurrentQueue<FileChange>> = &tx;

        // The bag carries the clock as a field, so `create` cannot miss it.
        let clock = bag.clock();

        Self {
            reload_rx: (*queue).clone(),
            clock,
        }
    }
}

impl Serve for SseReloadHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let reload_rx = Arc::clone(&self.reload_rx);
        let clock = Arc::clone(&self.clock);

        // The response head and every event go through the async
        // `ResponseWriter`: a slow client parks on write readiness instead of
        // truncating an event with `WouldBlock`. The old handler looped on this
        // worker thread with a synchronous `EventWriter`; it now runs as the
        // connection's async continuation, so no worker thread is blocked.
        ConnectionResult::Async(Box::pin(async move {
            let head = match render_message(SseResponse::new().build()) {
                Ok(head) => head,
                Err(e) => {
                    tracing::error!("Failed to render the SSE response head: {e}");
                    return ServeOutcome::Close(None);
                }
            };

            let mut response = ResponseWriter::new(conn, WriterConfig::default());
            if let Err(e) = response.write_all(&head).await {
                tracing::debug!("SSE head write failed: {e}");
                return ServeOutcome::Take;
            }
            if let Err(e) = response.flush().await {
                tracing::debug!("SSE head flush failed: {e}");
                return ServeOutcome::Take;
            }
            let mut writer = SseWriter::from_writer(response);

            let mut next_keepalive = clock.now() + Duration::from_secs(1);
            loop {
                // Send every queued reload event.
                while reload_rx.pop().is_ok() {
                    let event = SseEvent::new().event("reload").data("ready").build();
                    if writer.send(&event).await.is_err() {
                        return ServeOutcome::Take;
                    }
                }

                // Send a keep-alive every second, timed by the injected clock.
                if clock.now() >= next_keepalive {
                    if writer.comment("keep-alive").await.is_err() {
                        return ServeOutcome::Take;
                    }
                    next_keepalive = clock.now() + Duration::from_secs(1);
                }

                timer::sleep_for(Duration::from_millis(50)).await;
            }
        }))
    }
}
