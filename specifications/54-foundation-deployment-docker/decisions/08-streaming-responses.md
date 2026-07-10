# 08 — Streaming Responses

**Date:** 2026-07-10
**Status:** Resolved

## Decision

Use `StreamIteratorExt` combinators over `SendSafeBodyBytesIterator` to process Docker streaming
endpoints. No callback-based `process_streaming_body` — compose through the valtron combinator
chain.

## The streaming pipeline

For a Docker streaming endpoint (logs, events, stats, build progress, pull progress):

```
ClientRequestBuilder
    → build_send_request() → SendRequestTask (TaskIterator)
    → .map_ready(|intro| { extract SendSafeBody from stream })
    → execute(task, None) → StreamIterator
    → .flat_map_next(|body| SendSafeBodyBytesIterator::new(body))
        → StreamIterator<Stream<SendSafeBodyBytesItem, ()>>
    → .map_done(|chunk| decode_frame(chunk))
    → .filter_done(|frame| frame.is_some())
    → .collect() (at boundary)
```

### `SendSafeBodyBytesIterator`

From `body_reader.rs`:

```rust
pub struct SendSafeBodyBytesIterator(SendSafeBody);

impl Iterator for SendSafeBodyBytesIterator {
    type Item = Stream<SendSafeBodyBytesItem, ()>;
}

pub enum SendSafeBodyBytesItem {
    Chunk(Bytes),
    StreamError(BoxedError),
}
```

It wraps any `SendSafeBody` variant (Text, Bytes, Stream, ChunkedStream, LineFeedStream, SseStream)
and yields `Stream::Next(Chunk(bytes))` for each chunk. This is a `StreamIterator` — it works
with all `StreamIteratorExt` combinators.

### Example: Docker events (JSON-line stream)

Each line is a JSON `EventMessage` object:

```rust
fn events_stream<R: DnsResolver>(
    client: &SimpleHttpClient<R>,
    options: &EventsOptions,
) -> Result<impl TaskIterator<Ready = Result<Vec<DockerEvent>, DockerApiError>, Pending = DockerPending, Spawner = BoxedSendExecutionAction> + Send + 'static, DockerApiError> {
    let url = format!("http://docker/v{}/events?{}", API_VERSION, query_string(options));

    let task = client.get(&url)?
        .build_send_request()
        .map_err(|e| DockerApiError::Transport(e.to_string()))?
        .map_ready(|intro| match intro {
            RequestIntro::Success { stream, .. } => {
                // Extract SendSafeBody from the response stream
                extract_body_from_parts(stream)
            }
            RequestIntro::Failed(e) => Err(DockerApiError::Transport(e.to_string())),
        });

    let body_stream = execute(task, None)
        .map_err(|e| DockerApiError::Transport(e.to_string()))?;

    // Chain StreamIteratorExt combinators over the body chunks
    Ok(body_stream
        .flat_map_next(|body| SendSafeBodyBytesIterator::new(body))
        .map_done(|item| match item {
            SendSafeBodyBytesItem::Chunk(bytes) => {
                // Each chunk from LineFeedStream is one line
                serde_json::from_slice::<DockerEvent>(&bytes)
                    .map_err(|e| DockerApiError::JsonParse(e.to_string()))
            }
            SendSafeBodyBytesItem::StreamError(e) => {
                Err(DockerApiError::Transport(e.to_string()))
            }
        })
        .filter_done(|r| r.is_ok())
        .map_circuit(|item| match item {
            Stream::Next(Err(e)) => ShortCircuit::ReturnAndStop(Stream::Next(Err(e))),
            _ => ShortCircuit::Continue(item),
        })
    )
}
```

### Example: Container logs (multiplexed 8-byte frames)

Docker's log format: `[stream_type: u8][padding: u8; 3][length: u32 BE][payload]`

```rust
fn container_logs<R: DnsResolver>(...) -> impl TaskIterator<...> {
    // ... same setup as above ...

    Ok(body_stream
        .flat_map_next(|body| SendSafeBodyBytesIterator::new(body))
        .map_done(|item| {
            match item {
                SendSafeBodyBytesItem::Chunk(bytes) => {
                    // Feed chunk to LogFrameDecoder, return all complete frames
                    decoder.feed(&bytes);
                    let mut frames = Vec::new();
                    while let Some(frame) = decoder.decode() {
                        frames.push(frame);
                    }
                    frames
                }
                SendSafeBodyBytesItem::StreamError(e) => {
                    // Error case handled by map_circuit below
                    vec![]
                }
            }
        })
        // Decoder may produce partial frames — return Ignore if no complete frames
        .filter_done(|frames| !frames.is_empty())
        .map_circuit(|item| match item {
            Stream::Next(Err(e)) => ShortCircuit::ReturnAndStop(item),
            _ => ShortCircuit::Continue(item),
        })
    )
}
```

### `LogFrameDecoder` (the one custom type)

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
        if self.buffer.len() - self.cursor < 8 {
            return None;
        }
        let header = &self.buffer[self.cursor..self.cursor + 8];
        let stream_type = header[0];
        let length = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;

        if self.buffer.len() - self.cursor < 8 + length {
            return None;
        }

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

~50 lines. Handles buffering, partial frames, and frame extraction.

### Example: Image pull/build progress (JSON-line stream with collection)

```rust
fn pull_image<R: DnsResolver>(...) -> impl TaskIterator<Ready = Result<BuildProgress, DockerApiError>, ...> {
    // ...
    Ok(body_stream
        .flat_map_next(|body| SendSafeBodyBytesIterator::new(body))
        .map_done(|item| match item {
            SendSafeBodyBytesItem::Chunk(bytes) => {
                serde_json::from_slice::<PullImageInfo>(&bytes).ok()
            }
            _ => None,
        })
        .filter_done(|opt| opt.is_some())
        .map_done(|info| info.unwrap())
        // Accumulate all progress events, return final status
        .fold(Vec::new(), |mut acc, info| {
            acc.push(info);
            acc
        })
    )
}
```

## What the generated code does (non-streaming endpoints)

For single-response endpoints, `UnifiedGenerator` produces:

```rust
.map_ready(|intro| match intro {
    RequestIntro::Success { stream, intro, headers, .. } => {
        let body = body_reader::collect_string(stream);
        let parsed: T = serde_json::from_str(&body)?;
        Ok(ApiResponse { status, headers, body: parsed })
    }
    RequestIntro::Failed(e) => Err(ApiError::RequestSendFailed(e)),
})
```

This calls `collect_string(stream)` which eagerly drains the entire body. Correct for
single JSON responses (create, inspect, list, delete). No change needed.

For streaming endpoints, we replace the `collect_string` call with the
`SendSafeBodyBytesIterator` → `StreamIteratorExt` chain shown above.

## What we DON'T need

- Custom `SharedByteBufferStream` wrappers — `SendSafeBodyBytesIterator` already handles all `SendSafeBody` variants
- `process_streaming_body` callbacks — `StreamIteratorExt` combinators compose properly
- `JsonLineDecoder` — `serde_json::from_slice` per chunk is sufficient
- Async stream wrappers — valtron `StreamIterator` handles the async orchestration
- Custom transport layer — simple_http's `LineFeedStream`/`ChunkedStream`/`Stream` variants work

## Combinator choice guide for Docker streaming

| Goal | Combinator | Why |
|------|-----------|-----|
| Chunk → parse JSON | `.map_done(|chunk| serde_json::from_slice(...))` | Transform each chunk |
| Chunk → decode frames | `.map_done(|chunk| decoder.feed(); decoder.decode_all())` | Transform each chunk |
| Skip empty/partial | `.filter_done(|frames| !frames.is_empty())` | Filter out no-op chunks |
| Accumulate progress | `.fold(Vec::new(), |acc, item| { acc.push(item); acc })` | Collect over time |
| Stop on error | `.map_circuit(|item| match item { Err(e) => ReturnAndStop, _ => Continue })` | Short-circuit |
| Expand Vec → items | `.flat_map_next(|vec| vec.into_iter().map(Stream::Next))` | One-to-many |
