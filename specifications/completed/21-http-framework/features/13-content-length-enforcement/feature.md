---
feature: content-length-enforcement
description: Content-Length byte enforcement at body consumption time, error-propagating body collection, duplicate Content-Length rejection, SSE stream collection
status: completed
priority: high
depends_on: ["12-body-reader-streaming"]
estimated_effort: medium
created: 2026-05-14
last_updated: 2026-05-14
author: Claude Code
---

# Feature: Content-Length Enforcement and Error-Propagating Body Collection

## Overview

Add runtime enforcement that the total bytes consumed from a body stream match the declared `Content-Length` header. Unlike pre-parse-time checks, this fires at body consumption time — when the inner iterator returns `None`. Also adds `try_collect_bytes` / `try_collect_string` functions that propagate enforcement errors (unlike `collect_bytes_from_send_safe` which swallows them).

## Motivation

### Content-Length Enforcement Gap

`LimitedBatchStreamReader` caps at a byte limit but does not verify that the total bytes read equals the declared `Content-Length`. A server declaring `Content-Length: 100` but sending only 5 bytes would silently truncate. The enforcement must fire at consumption time, not header parse time, because the actual byte count is only known when the stream ends.

### Silent Error Swallowing

`collect_bytes_from_send_safe` returns `Vec<u8>` directly, swallowing any stream errors. Callers have no way to detect truncation, connection failures, or protocol violations. A `Result`-returning variant is needed.

### Duplicate Content-Length

HTTP/1.1 allows a single `Content-Length` header or duplicate headers with the same value, but conflicting or multiple values indicate request smuggling. We reject any response/request with more than one `Content-Length` header.

## Design

### ContentLengthEnforcingIterator

Iterator wrapper using `Option<I>` with `inner.take()` pattern:

```rust
pub struct ContentLengthEnforcingIterator<I> {
    inner: Option<I>,
    expected: usize,
    bytes_read: usize,
}

impl<I> ContentLengthEnforcingIterator<I> {
    pub fn new(inner: I, expected: usize) -> Self {
        Self { inner: Some(inner), expected, bytes_read: 0 }
    }
}

impl<I> Iterator for ContentLengthEnforcingIterator<I>
where I: Iterator<Item = Result<Data, BoxedError>>
{
    fn next(&mut self) -> Option<Self::Item> {
        // Take inner; None means already exhausted
        let mut inner = self.inner.take()?;
        match inner.next() {
            Some(Ok(Data::Bytes(bytes))) => {
                self.bytes_read += bytes.len();
                self.inner = Some(inner);  // put back for next call
                Some(Ok(Data::Bytes(bytes)))
            }
            Some(other) => {
                self.inner = Some(inner);  // Retry, errors, etc. pass through
                Some(other)
            }
            None => {
                // Inner exhausted — check if bytes match
                if self.bytes_read != self.expected {
                    Some(Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        format!("body truncated: expected {} bytes per Content-Length, got {}",
                            self.expected, self.bytes_read)
                    ))))
                } else {
                    None  // clean end
                }
            }
        }
    }
}
```

Key behavior:
- `Data::Retry` passes through without counting toward bytes
- Errors pass through immediately
- After inner returns `None`, if `bytes_read != expected`, returns `UnexpectedEof` error
- After the error is consumed, `inner` is `None` so subsequent `next()` returns `None`

### LineFeedContentLengthEnforcer

Same pattern for `LineFeed`-yielding iterators (used by `SendSafeBody::LineFeedStream`):

```rust
pub struct LineFeedContentLengthEnforcer<I> {
    inner: Option<I>,
    expected: usize,
    bytes_read: usize,
}
```

### try_collect_bytes / try_collect_string

`Result`-returning variants that handle all `SendSafeBody` variants and propagate errors:

```rust
pub fn try_collect_bytes(body: SendSafeBody) -> Result<Vec<u8>, BoxedError>;
pub fn try_collect_string(body: SendSafeBody) -> Result<String, BoxedError>;
```

| Variant | Behavior |
|---------|----------|
| `Text(s)` | `s.into_bytes()` |
| `Bytes(b)` | `b` |
| `None` | empty vec |
| `Stream(iter)` | iterate `Data::Bytes`, skip `Retry`, propagate errors |
| `ChunkedStream(iter)` | iterate `ChunkedData::Data`, skip `Trailers`, stop at `DataEnded` |
| `LineFeedStream(iter)` | iterate `LineFeed::Line`, append `\n` per line |
| `SseStream(iter)` | iterate `ParseResult`, extract `Event::Message { data }`, append `\n` per message, skip `Comment`/`Reconnect` |

### Duplicate Content-Length Rejection

In the request reader and response reader (in `impls.rs`), after header parsing:

```rust
if content_size_headers.len() > 1 {
    self.state = HttpReadState::Finished;
    return Some(Err(HttpReaderError::DuplicateContentLength));
}
```

### Content-Length Trimming

Per RFC 7230 section 3.2.6, leading/trailing whitespace around numeric values is allowed via the `OWS` rule. `Content-Length` values with surrounding whitespace are trimmed, not rejected.

### Empty Transfer-Encoding Values

Empty `Transfer-Encoding` headers are no longer silently skipped. They reach the `SimpleHeader` conversion step, producing an empty-value entry that doesn't match `TRANSFER-ENCODING`, allowing the TE+CL conflict detection to fire correctly when a non-empty `Transfer-Encoding` header is also present.

## Implementation Details

### Files Modified

| File | Changes |
|------|---------|
| `backends/foundation_core/src/wire/simple_http/client/body_reader.rs` | Added `ContentLengthEnforcingIterator`, `LineFeedContentLengthEnforcer`, `try_collect_bytes`, `try_collect_string`, 30+ unit tests |
| `backends/foundation_core/src/wire/simple_http/impls.rs` | Wrapped `LimitedBatchStreamReader` with `ContentLengthEnforcingIterator`; added duplicate Content-Length rejection in request and response readers; removed early empty-value skip for Transfer-Encoding |
| `backends/foundation_core/src/wire/simple_http/errors.rs` | Added `DuplicateContentLength`, `InvalidContentLengthFormat` to `HttpReaderError` |

### Integration Point

In `impls.rs`, the `SimpleHttpBody::extract` for `Body::LimitedBody`:

```rust
let limited = LimitedBatchStreamReader::new(batch, content_length as usize);
let enforcing = ContentLengthEnforcingIterator::new(limited, content_length as usize);
let stream_reader: BoxedSendableDataIterator<BoxedError> = Box::new(enforcing);
Ok(SendSafeBody::Stream(Some(stream_reader)))
```

## Tests Added

30+ unit tests in `body_reader.rs`:

### ContentLengthEnforcingIterator
| Test | What it verifies |
|------|-----------------|
| `test_content_length_enforcing_exact_match` | bytes_read == expected → None on inner exhaustion |
| `test_content_length_enforcing_truncated` | bytes_read < expected → UnexpectedEof error |
| `test_content_length_enforcing_retry_passthrough` | Data::Retry passes through, not counted |
| `test_content_length_enforcing_error_passthrough` | Stream errors pass through immediately |
| `test_content_length_enforcing_zero_expected_empty_body` | 0 expected, empty body → None |
| `test_content_length_enforcing_stops_after_exhaustion` | After error, inner is None → further next() returns None |

### try_collect_bytes
| Test | What it verifies |
|------|-----------------|
| `test_try_collect_bytes_text` | Text → bytes |
| `test_try_collect_bytes_bytes` | Bytes passthrough |
| `test_try_collect_bytes_none` | None → empty vec |
| `test_try_collect_bytes_stream_success` | Stream collects Data::Bytes |
| `test_try_collect_bytes_stream_error_propagates` | Stream error returns Err |
| `test_try_collect_bytes_stream_none` | Stream(None) → empty vec |
| `test_try_collect_bytes_chunked_stream_success` | ChunkedStream collects Data chunks |
| `test_try_collect_bytes_chunked_stream_none` | ChunkedStream(None) → empty vec |
| `test_try_collect_bytes_linefeed_stream_success` | LineFeedStream joins lines with \n |
| `test_try_collect_bytes_linefeed_stream_none` | LineFeedStream(None) → empty vec |
| `test_try_collect_bytes_sse_stream_success` | SSE extracts Message data with \n |
| `test_try_collect_bytes_sse_stream_skips_comments` | SSE skips Comment events |
| `test_try_collect_bytes_sse_stream_empty` | SseStream(None) → empty vec |

### try_collect_string
| Test | What it verifies |
|------|-----------------|
| `test_try_collect_string_valid_utf8` | Text → String |
| `test_try_collect_string_invalid_utf8` | Invalid UTF-8 → Err |
| `test_try_collect_string_sse_stream` | SSE → String with \n |

### Enforcement Integration
| Test | What it verifies |
|------|-----------------|
| `test_try_collect_bytes_propagates_content_length_enforcement` | try_collect_bytes through ContentLengthEnforcingIterator returns UnexpectedEof when truncated |

## Success Criteria

- [x] `ContentLengthEnforcingIterator` fires error when bytes_read != expected
- [x] `ContentLengthEnforcingIterator` returns None when bytes_read == expected
- [x] `ContentLengthEnforcingIterator` passes through Data::Retry without counting
- [x] `ContentLengthEnforcingIterator` passes through errors immediately
- [x] `ContentLengthEnforcingIterator` inner is None after error consumption
- [x] `try_collect_bytes` handles all SendSafeBody variants
- [x] `try_collect_string` handles all SendSafeBody variants and returns Err on invalid UTF-8
- [x] Duplicate Content-Length headers are rejected
- [x] Content-Length with surrounding whitespace is trimmed (RFC 7230)
- [x] Empty Transfer-Encoding values no longer skipped before SimpleHeader conversion
- [x] SSE stream data collected via Event::Message with \n separator
- [x] 220 compliance tests pass
- [x] 65 body_reader unit tests pass

## Related Files

| File | Lines | Changes |
|------|-------|---------|
| `backends/foundation_core/src/wire/simple_http/client/body_reader.rs` | 1280-1480 | Added ContentLengthEnforcingIterator, LineFeedContentLengthEnforcer |
| `backends/foundation_core/src/wire/simple_http/client/body_reader.rs` | 1480-1520 | Added try_collect_bytes, try_collect_string |
| `backends/foundation_core/src/wire/simple_http/client/body_reader.rs` | 2000-2389 | Added 30+ unit tests |
| `backends/foundation_core/src/wire/simple_http/impls.rs` | ~3520 | Duplicate Content-Length rejection |
| `backends/foundation_core/src/wire/simple_http/errors.rs` | 415-416 | DuplicateContentLength, InvalidContentLengthFormat |
