---
feature: "SSE Reload"
description: "Serve both /static/sse/reloader.js (StaticAssetHandler) and /static/sse/reload (SseReloadHandler) — foundation_toolings owns the complete reload chain"
status: "complete"
priority: "high"
depends_on: ["06-native-proxy"]
estimated_effort: "small"
created: "2026-06-01"
last_updated: "2026-06-15"
---

# Feature: SSE Reload

## Problem

Current SSE reload in `crates/devserver/src/assets.rs`:
- Uses `axum::response::sse::{Event, KeepAlive, Sse}`
- Uses `tokio_stream::wrappers::BroadcastStream` to convert broadcast to stream
- Uses `axum::response::IntoResponse` trait
- Two SSE-related endpoints:
  - `/static/sse/reloader.js` — served by the **upstream app** (proxy forwards it through)
  - `/static/sse/reload` — SSE endpoint served **locally by the proxy** (sends reload events)

## Solution

**`foundation_toolings` owns both endpoints.** The devserver crate serves the reloader script AND the SSE stream — no upstream dependency. Users don't need to add anything to their app.

### Both routes registered in HttpApp

```rust
// 1. Serve reloader.js from embedded asset (foundation_toolings owns it)
let reloader = StaticAssetHandler::new(
    include_bytes!("reloader.js"),
    "text/javascript; charset=utf-8",
);
app.router().add_route_any("/static/sse/reloader.js", Arc::new(reloader));

// 2. Serve SSE reload endpoint (sends reload events to browser)
app.route_any::<SseReloadHandler>("/static/sse/reload");
```

### Reloader.js (embedded, served locally)

The existing `crates/devserver/src/reloader.js` is embedded into `foundation_toolings` as a static asset. No `AssetReloader` indirection needed — it's just bytes served directly:

```javascript
// reloader.js (embedded in foundation_toolings)
const reload_signals = new EventSource("/static/sse/reload");

reload_signals.addEventListener("reload", (event) => {
    window.location.reload();
});

reload_signals.addEventListener("message", (event) => {
    console.log("Received generic event check your setup", event);
});
```

### SseReloadHandler — foundation_http Serve impl

```rust
use foundation_http::{
    shared::{
        context::ContextBag,
        serve::{ConnectionResult, Serve, ServeFactory},
    },
};
use foundation_netio::event_source::{SseEvent, EventWriter, SseResponse};
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::netcap::RawStream;

/// Registered via `app.route_any::<SseReloadHandler>("/static/sse/reload")`.
/// Takes ownership of the connection (ConnectionResult::Take) and writes SSE events
/// until the client disconnects.
pub struct SseReloadHandler {
    reload_rx: mpp::Receiver<FileChange>,
}

impl ServeFactory for SseReloadHandler {
    fn create(bag: &ContextBag) -> Self {
        let tx = bag.get::<mpp::Sender<FileChange>>()
            .expect("reload Sender in ContextBag");
        Self { reload_rx: tx.subscribe() }
    }
}

impl Serve for SseReloadHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        // 1. Send SSE response headers (Content-Type: text/event-stream, etc.)
        let sse_response = SseResponse::new().build();
        if sse_response.write_to(&mut conn).is_err() {
            return ConnectionResult::Close(None);
        }

        // 2. Create event writer
        let mut writer = EventWriter::new(&mut conn);
        let mut last_keepalive = Instant::now();

        // 3. Loop: send events from queue, send keepalive every 1s
        loop {
            // Check for new reload events
            while let Ok(_change) = self.reload_rx.try_recv() {
                if writer.send(&SseEvent::new()
                    .event("reload")
                    .data("ready")
                    .build()).is_err() {
                    return ConnectionResult::Take; // client disconnected
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
```

### The `ConnectionResult::Take` contract

When the handler returns `ConnectionResult::Take`, the `ConnectionHandler` exits its
valtron TaskIterator — the SSE handler owns the connection until the client disconnects.
No thread handoff, no separate SSE loop. The valtron thread that handled the HTTP request
stays with this connection for its entire lifetime.

### Task Breakdown

1. [ ] Embed `reloader.js` as a static asset in `foundation_toolings`
2. [ ] Wire `StaticAssetHandler` for `/static/sse/reloader.js`
3. [ ] Implement `SseReloadHandler` as `Serve` + `ServeFactory`
4. [ ] Register reload channel in `ContextBag` so handler can subscribe via `mpp`
5. [ ] Wire route: `app.route_any::<SseReloadHandler>("/static/sse/reload")`
6. [ ] Keep-alive timer (sync `Instant::now()`, no tokio::time::interval)
7. [ ] Write tests: SSE event format (via EventWriter), response headers (via SseResponse), ConnectionResult::Take behavior

### Dependencies from foundation crates

- `foundation_http::shared::serve::{Serve, ServeFactory, ConnectionResult}` — handler interface
- `foundation_http::shared::context::ContextBag` — shared state (reload channel)
- `foundation_http::native::handlers::static_asset::StaticAssetHandler` — serve reloader.js
- `foundation_netio::event_source::{SseEvent, EventWriter, SseResponse}` — SSE protocol
- `crates/devserver/src/reloader.js` — embedded JS file (moved into foundation_toolings)

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/src/proxy/handlers.rs` | Create — SseReloadHandler Serve impl |
| `backends/foundation_toolings/src/proxy/reloader.js` | Copy from `crates/devserver/src/reloader.js` |

---

_Created: 2026-06-01_
