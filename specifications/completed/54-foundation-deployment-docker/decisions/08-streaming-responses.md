# 08 — Streaming Responses

**Date:** 2026-07-10
**Updated:** 2026-07-12 (F51 alignment — Feature 01)
**Status:** Resolved

## Decision

Use `split_exchange()` to handle streaming endpoints. The `(head, body, task)` tuple separates
status/headers from the byte stream. The body observer yields `Result<Bytes, Error>` per chunk
— callers compose decoders on top of the body `StreamIterator`.

## The streaming pipeline

For a Docker streaming endpoint (logs, events, stats, build progress, pull progress):

```
PreparedRequestBuilder
    → .send(&client) → HttpExchangeClientTask (unsent TaskIterator)
    → split_exchange() → (head, body, task) tuple
    → valtron::send(task) → task runs in valtron
    → head.next() → one (Status, SimpleHeaders)
    → body.next() → Stream::Next(Ok(Bytes)) per chunk
    → decode frames / parse JSON on each chunk
```

### Non-streaming vs. streaming

| Pattern | Function | Returns |
|---------|----------|---------|
| Single JSON response | `collect_exchange()` | `Result<SimpleResponse<SendSafeBody>, Error>` |
| Streaming body | `split_exchange()` | `(head, body, task)` — caller sends task, drains observers |

### Example: Docker events (JSON-line stream)

Each line is a JSON `EventMessage` object. The body observer yields raw `Bytes` chunks;
the caller decodes each chunk:

```rust
fn events_stream(
    client: DynNetClient,
    args: &EventsOptions,
) -> (
    CollectorStreamIterator<Result<(Status, SimpleHeaders), Arc<dyn Error + Send + Sync>>, ()>,
    CollectorStreamIterator<Result<Bytes, Arc<dyn Error + Send + Sync>>, ()>,
    Box<dyn TaskIterator<Ready = HttpExchange, Pending = HttpExchangePending,
                         Spawner = BoxedSendExecutionAction> + Send>,
) {
    let endpoint_url = format!("http://docker/v{}/events", API_VERSION);
    let builder = PreparedRequestBuilder::get(&endpoint_url)?
        .query("filters", args.filters.as_deref());

    body_reader::split_exchange(&client, builder)
}
```

Caller usage:
```rust
let (mut head, mut body, task) = events_stream(&client, &args);

// Send the continuation.
valtron::send(task.map_ready(|_| ()))?;

// Check head.
let (status, headers) = match head.next() {
    Some(Stream::Next(Ok(pair))) => pair,
    Some(Stream::Next(Err(e))) => return Err(e),
    _ => return Err(...),
};
if !status.is_success() { return Err(...); }

// Stream body chunks — decode each JSON line.
while let Some(item) = body.next() {
    match item {
        Stream::Next(Ok(chunk)) => {
            let event: DockerEvent = serde_json::from_slice(&chunk)?;
            handle_event(event);
        }
        Stream::Next(Err(e)) => return Err(e),
        _ => {}
    }
}
```

In an async context, convert the body `StreamIterator` to a `Stream` via
`.into_next_stream()` and await chunks with `StreamExt::next().await` — same
pattern as `H1Transport` at lines 146-147. No blocking loop needed.

### Example: Container logs (multiplexed 8-byte frames)

Docker's log format: `[stream_type: u8][padding: u8; 3][length: u32 BE][payload]`

```rust
fn container_logs(
    client: DynNetClient,
    container_id: &str,
    follow: bool,
) -> (...) {
    let endpoint_url = format!(
        "http://docker/v{}/containers/{}/logs",
        API_VERSION, container_id,
    );
    let builder = PreparedRequestBuilder::get(&endpoint_url)?
        .query("stdout", Some("1"))
        .query("stderr", Some("1"))
        .query("follow", Some(if follow { "1" } else { "0" }));

    body_reader::split_exchange(&client, builder)
}
```

Caller decodes frames from the body stream:
```rust
let mut decoder = LogFrameDecoder::new();

while let Some(item) = body.next() {
    match item {
        Stream::Next(Ok(chunk)) => {
            decoder.feed(&chunk);
            while let Some(frame) = decoder.decode() {
                handle_log_frame(frame);
            }
        }
        Stream::Next(Err(e)) => return Err(e),
        _ => {}
    }
}
```

### `LogFrameDecoder` (~50 lines, unchanged)

```rust
pub struct LogFrameDecoder {
    buffer: Vec<u8>,
    cursor: usize,
}

impl LogFrameDecoder {
    pub fn new() -> Self { Self { buffer: Vec::new(), cursor: 0 } }

    pub fn feed(&mut self, chunk: &[u8]) {
        self.buffer.extend_from_slice(chunk);
    }

    pub fn decode(&mut self) -> Option<LogOutput> {
        if self.buffer.len() - self.cursor < 8 { return None; }
        let header = &self.buffer[self.cursor..self.cursor + 8];
        let stream_type = header[0];
        let length = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;
        if self.buffer.len() - self.cursor < 8 + length { return None; }

        let payload_start = self.cursor + 8;
        let payload = Bytes::copy_from_slice(&self.buffer[payload_start..payload_start + length]);
        self.cursor = payload_start + length;

        match stream_type {
            0 => Some(LogOutput::StdOut { message: payload }),
            1 => Some(LogOutput::StdIn { message: payload }),
            2 => Some(LogOutput::StdErr { message: payload }),
            3 => Some(LogOutput::Console { message: payload }),
            _ => None,
        }
    }
}
```

## What the generated code does (non-streaming endpoints)

For single-response endpoints, the generator emits `collect_exchange()`:

```rust
let result = body_reader::collect_exchange(&client, builder);

Ok(SingleReadyTask::new(result.map(|response| {
    let body: T = serde_json::from_slice(response.body().as_bytes())?;
    ApiResponse { status: response.status().into(), headers: response.headers().clone(), body }
}).map_err(|e| ApiError::RequestSendFailed(e.to_string()))))
```

## What we DON'T need

- `SendSafeBodyBytesIterator` — `split_exchange()` returns `Result<Bytes, Error>` per chunk directly, unwrapping `HttpExchange::BodyChunk`
- `RequestIntro` matching — the split predicates handle `Head`/`BodyChunk`/`Failed` enum matching internally
- `process_streaming_body` callbacks — no callbacks, just `StreamIterator` draining
- `SharedByteBufferStream` wrappers — the body observer IS the stream
- Custom transport layer — `DynNetClient` handles the transport, the body observer handles the bytes

## Related

- **[04 — HTTP via DynNetClient + PreparedRequestBuilder](04-http-via-simple-http-client.md)** — HTTP client pattern
- **[05 — Valtron TaskIterator format](05-valtron-task-iterator.md)** — async fn pattern
- **[Feature 05 — Streaming endpoint handling](../features/05-streaming-endpoints/feature.md)** — Implementation plan
