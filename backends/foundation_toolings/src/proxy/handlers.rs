// Proxy handlers: SseReloadHandler.

use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ConnectionResult, Serve, ServeFactory};
use foundation_netio::event_source::{SseEvent, EventWriter, SseResponse};
use foundation_netio::simple_http::shared::{Http11, RenderHttp};
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::netcap::RawStream;

use concurrent_queue::ConcurrentQueue;

use crate::watcher::FileChange;

// -- SseReloadHandler

pub struct SseReloadHandler {
    reload_rx: Arc<ConcurrentQueue<FileChange>>,
}

impl ServeFactory for SseReloadHandler {
    fn create(bag: &ContextBag) -> Self {
        let tx = bag.get::<Arc<ConcurrentQueue<FileChange>>>()
            .expect("reload queue in ContextBag");
        // bag.get() returns Arc<T>, T is already Arc<ConcurrentQueue>
        // We need to get the inner Arc via deref
        let queue: &Arc<ConcurrentQueue<FileChange>> = &tx;
        Self { reload_rx: (*queue).clone() }
    }
}

impl Serve for SseReloadHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        _req: foundation_netio::simple_http::shared::SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        // 1. Send SSE response headers
        let sse_response = SseResponse::new().build();
        if Http11::response(sse_response)
            .http_render_to_writer(&mut conn)
            .is_err()
        {
            return ConnectionResult::Close(None);
        }

        // 2. Create event writer
        let mut writer = EventWriter::new(&mut conn);
        let mut last_keepalive = Instant::now();

        // 3. Loop: send events, send keepalive every 1s
        loop {
            // Check for new reload events
            while let Ok(_change) = self.reload_rx.pop() {
                if writer
                    .send(&SseEvent::new().event("reload").data("ready").build())
                    .is_err()
                {
                    return ConnectionResult::Take;
                }
            }

            // Send keepalive every 1 second
            if Instant::now() - last_keepalive >= Duration::from_secs(1) {
                if writer.comment("keep-alive").is_err() {
                    return ConnectionResult::Take;
                }
                last_keepalive = Instant::now();
            }

            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
