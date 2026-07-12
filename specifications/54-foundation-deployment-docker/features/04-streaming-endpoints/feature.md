---
feature: "Streaming endpoint handling for Docker logs, events, stats, build/pull progress"
description: "Hand-write streaming endpoint functions using split_exchange() pattern; implement LogFrameDecoder for Docker's 8-byte multiplexed log format; async Stream via .into_next_stream() for caller consumption"
status: "in-progress"
priority: "high"
phase: 1
depends_on: ["01-dynnetclient-codegen-alignment", "03-type-replication"]
estimated_effort: "small"
created: 2026-07-12
---
# Feature 04: Streaming endpoint handling

## Why

Docker has streaming endpoints (logs, events, stats, build progress, pull
progress) that return data incrementally — not single JSON responses. The code
generator (Feature 03) produces non-streaming `async fn` → `collect_exchange()`
for single-response endpoints. Streaming endpoints must be hand-written using
`split_exchange()`.

## What to do

### 1. Streaming endpoints to implement

| Endpoint | Method | Path | Response format |
|----------|--------|------|----------------|
| Container logs | GET | `/containers/{id}/logs` | 8-byte multiplexed frames |
| Container stats | GET | `/containers/{id}/stats` | JSON-line stream |
| Container attach | POST | `/containers/{id}/attach` | Raw multiplexed stream |
| System events | GET | `/events` | JSON-line stream |
| Image pull | POST | `/images/create` | JSON-line progress |
| Image push | POST | `/images/{name}/push` | JSON-line progress |
| Image build | POST | `/build` | JSON-line stream |

### 2. Pattern: `split_exchange()` + `into_next_stream()`

Every streaming endpoint follows the same pattern:

```rust
use foundation_netio::shared::client::body_reader;
use foundation_core::valtron::StreamIteratorExt;

pub fn container_logs(
    client: DynNetClient,
    container_id: &str,
    follow: bool,
    stdout: bool,
    stderr: bool,
) -> (
    CollectorStreamIterator<Result<(Status, SimpleHeaders), Arc<dyn Error + Send + Sync>>, ()>,
    CollectorStreamIterator<Result<Bytes, Arc<dyn Error + Send + Sync>>, ()>,
    Box<dyn TaskIterator<Ready = HttpExchange, Pending = HttpExchangePending,
                         Spawner = BoxedSendExecutionAction> + Send>,
) {
    let url = format!("http://localhost/v{API_VERSION}/containers/{container_id}/logs");
    let builder = PreparedRequestBuilder::get(&url)?
        .query("follow", Some(if follow { "1" } else { "0" }))
        .query("stdout", Some(if stdout { "1" } else { "0" }))
        .query("stderr", Some(if stderr { "1" } else { "0" }));

    body_reader::split_exchange(&client, builder)
}
```

Caller (async context):
```rust
let (head, mut body, task) = container_logs(&client, "abc123", false, true, true);

// Send the continuation to valtron.
valtron::send(task.map_ready(|_| ()))?;

// Check head.
let (status, _headers) = match head.next() {
    Some(Stream::Next(Ok(pair))) => pair,
    Some(Stream::Next(Err(e))) => return Err(e),
    _ => return Err(anyhow!("no response head")),
};

// Convert body to a Stream for async consumption.
let body_stream = body.into_next_stream();

// Decode multiplexed log frames.
let mut decoder = LogFrameDecoder::new();
while let Some(Stream::Next(Ok(chunk))) = body_stream.next().await {
    decoder.feed(&chunk);
    while let Some(frame) = decoder.decode() {
        handle_log_frame(frame);
    }
}
```

### 3. `LogFrameDecoder`

Docker's log format: `[stream_type: u8][padding: u8; 3][length: u32 BE][payload]`

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
        const HEADER_SIZE: usize = 8;
        if self.buffer.len() - self.cursor < HEADER_SIZE { return None; }

        let header = &self.buffer[self.cursor..self.cursor + HEADER_SIZE];
        let stream_type = header[0];
        let length = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;

        if self.buffer.len() - self.cursor < HEADER_SIZE + length { return None; }

        let payload_start = self.cursor + HEADER_SIZE;
        let payload = Bytes::copy_from_slice(
            &self.buffer[payload_start..payload_start + length]
        );
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

pub enum LogOutput {
    StdIn { message: Bytes },
    StdOut { message: Bytes },
    StdErr { message: Bytes },
    Console { message: Bytes },
}
```

~50 lines.

### 4. Events (JSON-line stream)

Each chunk is a JSON `EventMessage`. No frame decoder needed — just JSON lines:

```rust
pub fn events(
    client: DynNetClient,
    filters: Option<&str>,
) -> ( /* ... same signature as container_logs ... */ ) {
    let url = format!("http://localhost/v{API_VERSION}/events");
    let mut builder = PreparedRequestBuilder::get(&url)?;
    if let Some(f) = filters {
        builder = builder.query("filters", Some(f));
    }
    body_reader::split_exchange(&client, builder)
}
```

Caller: `serde_json::from_slice::<DockerEvent>(&chunk)` on each `Ok(chunk)`.

### 5. Build/pull progress (JSON-line stream)

Same pattern as events — each chunk is a JSON status object:

```rust
pub fn pull_image(
    client: DynNetClient,
    image: &str,
    tag: Option<&str>,
) -> ( /* ... */ ) {
    let url = format!("http://localhost/v{API_VERSION}/images/create");
    let builder = PreparedRequestBuilder::post(&url)?
        .query("fromImage", Some(image))
        .query("tag", tag);
    body_reader::split_exchange(&client, builder)
}
```

## Scope

| File | What |
|------|------|
| `src/streaming/mod.rs` | `split_exchange()`-based endpoint functions |
| `src/streaming/decoder.rs` | `LogFrameDecoder` + `LogOutput` |

## Verification

- `container_logs()` with stdout only returns stdout frames
- `container_logs()` with stderr only returns stderr frames
- `LogFrameDecoder` handles partial frames (buffers across `feed()` calls)
- `LogFrameDecoder` handles multiple complete frames in one chunk
- `events()` yields parseable `DockerEvent` JSON objects
- All streaming functions type-check with `cargo check`

## Acceptance criteria

1. All 7 streaming endpoints have `split_exchange()`-based functions
2. `LogFrameDecoder` correctly decodes Docker's multiplexed format (~50 lines)
3. Callers can consume body chunks as `Stream<Item = Stream<Result<Bytes, Error>, ()>>`
   via `into_next_stream()`
4. No `SendSafeBodyBytesIterator` used — body observer yields `Result<Bytes, Error>` directly
5. Error propagation works: `Failed` flows through the observer as `Err`
