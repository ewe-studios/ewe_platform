---
feature: "SSE Reload"
description: "Use foundation_netio::event_source (EventWriter, SseEvent, SseResponse) for SSE reload endpoint — no hand-rolled HTTP"
status: "pending"
priority: "high"
depends_on: ["06-native-proxy"]
estimated_effort: "small"
created: 2026-06-01
last_updated: 2026-06-01
---

# Feature: SSE Reload

## Problem

Current SSE reload in `crates/devserver/src/assets.rs`:
- Uses `axum::response::sse::{Event, KeepAlive, Sse}`
- Uses `tokio_stream::wrappers::BroadcastStream` to convert broadcast to stream
- Uses `axum::response::IntoResponse` trait
- Two SSE-related endpoints in the system:
  - `/static/sse/reloader.js` — served by the **upstream app** (proxy forwards it through)
  - `/static/sse/reload` — SSE endpoint served **locally by the proxy** (sends reload events)

## Solution

Only `/static/sse/reload` is handled locally by the proxy. `/static/sse/reloader.js` goes
upstream like any other request — the dev app serves the JS file itself. The proxy is a bridge
for everything except the reload endpoint.

The reloader.js (client-side) connects to `/static/sse/reload` on the proxy, and the proxy
locally writes SSE events from the reload queue into the client's connection.

### SSE Server-Side Types (already in foundation_netio)

```rust
// Build SSE events
use foundation_netio::event_source::{SseEvent, EventWriter};

// Build proper SSE HTTP response with correct headers
use foundation_netio::event_source::SseResponse;
// Content-Type: text/event-stream
// Cache-Control: no-cache
// Connection: keep-alive
```

### SSE Reload Endpoint

```rust
use foundation_netio::event_source::{SseEvent, EventWriter, SseResponse};
use foundation_netio::netcap::connection::Connection;
use std::time::Instant;

pub fn handle_sse_reload(
    mut client: Connection,
    reload_queue: Arc<ConcurrentQueue<FileChange>>,
) -> Result<()> {
    // 1. Send SSE response headers
    let sse_response = SseResponse::new().build();
    sse_response.write_to(&mut client)?;

    // 2. Create event writer
    let mut writer = EventWriter::new(&mut client);

    let mut last_keepalive = Instant::now();

    // 3. Loop: send events from queue, send keepalive every 1s
    loop {
        // Check for new reload events
        while let Ok(_change) = reload_queue.pop() {
            writer.send(&SseEvent::new()
                .event("reload")
                .data("ready")
                .build())?;
        }

        // Send keepalive every 1 second
        if Instant::now() - last_keepalive >= Duration::from_secs(1) {
            writer.comment("keep-alive")?;
            last_keepalive = Instant::now();
        }

        // Small sleep before next poll
        std::thread::sleep(Duration::from_millis(50));
    }
}
```

### Integration with Proxy

The SSE handler is called from the proxy's HTTP/1 request dispatch when the path matches `/static/sse/reload`. Because `Connection` implements `Read + Write`, the `EventWriter` writes directly to the socket. The loop runs as part of the proxy's connection handler — for long-lived SSE connections, the proxy dedicates the connection to SSE until the client disconnects.

### Reloader.js (Client-Side)

The existing `reloader.js` from `crates/devserver/src/reloader.js` is already embedded via `foundation_runtimes::AssetReloader`. No changes needed — it connects to `/static/sse/reload` and reloads on `event: reload`:

```javascript
const reload_signals = new EventSource("/static/sse/reload");
reload_signals.addEventListener("reload", (event) => {
    window.location.reload();
});
```

### Task Breakdown

1. [ ] Implement `handle_sse_reload` using `SseResponse` + `EventWriter`
3. [ ] Wire SSE routes into proxy HTTP/1 handler dispatch
4. [ ] Keep-alive timer (sync `Instant::now()`, no tokio::time::interval)
5. [ ] Write tests: SSE event format (via EventWriter), response headers (via SseResponse)

### Dependencies from foundation_netio

- `event_source::SseEvent` — server-side SSE event builder
- `event_source::EventWriter<W>` — writes SSE events to any `Write` stream
- `event_source::SseResponse` — builds SSE HTTP response with correct headers
- `foundation_runtimes::AssetReloader` — embedded reloader.js (already used by current devserver)

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/src/proxy/sse.rs` | Create — SSE endpoint + handler |

---

_Created: 2026-06-01_
