---
feature: "SSE Reload"
description: "Replace axum SSE + tokio_stream with raw HTTP SSE response generator + embedded reloader.js from foundation_runtimes"
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
- Two endpoints:
  - `/static/sse/reloader.js` — serves embedded JS from `foundation_runtimes::AssetReloader`
  - `/static/sse/reload` — SSE endpoint that sends reload events

The reloader.js (`crates/devserver/src/reloader.js`) is an EventSource client that:
1. Connects to `/static/sse/reload`
2. Listens for `reload` events
3. Reloads the page when received

## Solution

Replace with raw HTTP response generators:

### Static Script Endpoint

Already works via `foundation_runtimes::AssetReloader` — keep as-is:

```rust
pub fn sse_endpoint_script(request: &HttpRequest) -> HttpResponse {
    let instance = AssetReloader;
    if let Some(data) = instance.read_utf8_for("reloader.js") {
        HttpResponse::ok()
            .header("Content-Type", "text/javascript")
            .body(data.as_bytes())
    } else {
        HttpResponse::not_found()
    }
}
```

### SSE Endpoint (raw HTTP)

```
HTTP/1.1 200 OK
Content-Type: text/event-stream
Cache-Control: no-cache
Connection: keep-alive

retry: 1000
keep-alive

event: reload
data: ready
comment: indicates we should reload page

```

```rust
pub fn sse_endpoint_reloader(
    request: &HttpRequest,
    reload_queue: Arc<ConcurrentQueue<FileChange>>,
) -> HttpResponse {
    // Build SSE response as a stream
    // Headers: Content-Type: text/event-stream, Cache-Control: no-cache
    // Body: stream of events from reload_queue
    HttpResponse::streaming()
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(SseStream::new(reload_queue))
}
```

### SSE Stream (poll-layer driven)

The SSE stream is served by the proxy's HTTP/1 handler. Since we have raw socket I/O, we need to:
1. Send the HTTP headers
2. On each file change event, write the SSE event to the socket
3. Send keep-alive comments every 1 second

```rust
struct SseStream {
    queue: Arc<ConcurrentQueue<FileChange>>,
}

impl SseStream {
    fn write_event<W: io::Write>(&self, writer: &mut W, event: &FileChange) -> io::Result<()> {
        write!(writer, "event: reload\r\n")?;
        write!(writer, "data: ready\r\n")?;
        write!(writer, "comment: indicates we should reload page\r\n")?;
        write!(writer, "\r\n")?;
        writer.flush()
    }

    fn write_keepalive<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        write!(writer, ": keep-alive\r\n\r\n")?;
        writer.flush()
    }

    /// Called from proxy TaskIterator tick
    fn tick<W: io::Write>(&self, writer: &mut W, last_keepalive: Instant) -> (Instant, bool) {
        // Check for new events
        while let Ok(change) = self.queue.pop() {
            self.write_event(writer, &change)?;
        }

        // Send keepalive every 1s
        if Instant::now() - last_keepalive >= Duration::from_secs(1) {
            self.write_keepalive(writer)?;
            return (Instant::now(), true);
        }

        (last_keepalive, false)
    }
}
```

### Integration with Proxy

The SSE connection is a long-lived HTTP/1 connection. The proxy's HTTP/1 handler needs to:
1. Detect the `/static/sse/reload` path
2. Send SSE headers
3. Enter a loop: poll the SSE stream, write events, send keepalive
4. Exit when client disconnects (write error) or cancel signal

This is the one place where the proxy needs to "block" on a single connection — but it's still driven by the poll-layer, so other connections are not blocked.

### Task Breakdown

1. [ ] Define `HttpRequest` / `HttpResponse` with streaming support
2. [ ] Implement `sse_endpoint_script` handler (reuses AssetReloader)
3. [ ] Implement `sse_endpoint_reloader` handler
4. [ ] Implement `SseStream` with tick-based event writing
5. [ ] Integrate SSE endpoint into proxy HTTP/1 handler (long-lived connection)
6. [ ] Keep-alive timer (sync `Instant::now()`, no tokio::time::interval)
7. [ ] Write tests: SSE event format, keepalive timing, reloader.js content

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/src/proxy/sse.rs` | Create — SSE endpoint + stream |
| `backends/foundation_toolings/src/reloader.js` | Copy from devserver (or confirm in foundation_runtimes) |

---

_Created: 2026-06-01_
