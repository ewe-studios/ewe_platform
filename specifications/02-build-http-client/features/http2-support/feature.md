# Feature Specification: HTTP/2 Support for simple_http

## Overview

**Status**: Draft
**Priority**: Medium
**Target Version**: 0.1.0
**Last Updated**: 2026-03-12

This document specifies HTTP/2 protocol support for the `simple_http` framework, building upon the existing HTTP/1.1 implementation.

---

## Motivation

HTTP/2 provides significant improvements over HTTP/1.1:

1. **Multiplexing**: Multiple requests/responses over a single connection
2. **Header Compression**: HPACK compression reduces overhead
3. **Server Push**: Proactive resource delivery
4. **Binary Protocol**: More efficient parsing, less ambiguity
5. **Stream Prioritization**: Better resource allocation

---

## Goals

### In Scope (Phase 1)

- [ ] HTTP/2 binary frame parsing
- [ ] Connection preface handling
- [ ] HEADERS frame encoding/decoding
- [ ] DATA frame handling
- [ ] SETTINGS frame negotiation
- [ ] PING/PONG for keepalive
- [ ] GOAWAY graceful shutdown
- [ ] HPACK header compression
- [ ] Stream multiplexing
- [ ] Flow control

### Out of Scope (Phase 2+)

- [ ] Server Push (PUSH_PROMISE frames)
- [ ] Stream prioritization weights
- [ ] Connection migration
- [ ] ALPN negotiation (handled at TLS layer)
- [ ] h2c cleartext upgrade from HTTP/1.1

---

## Architecture

### Component Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    simple_http client                        │
├─────────────────────────────────────────────────────────────┤
│  HTTP/2 Layer                                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │ Http2Client │  │ Http2Server │  │ Http2Connection     │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │ FrameCodec  │  │ HPACKCodec  │  │ StreamManager       │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│  Transport Layer (existing)                                 │
│  ┌─────────────┐  ┌─────────────┐                           │
│  │ TcpStream   │  │ TlsStream   │                           │
│  └─────────────┘  └─────────────┘                           │
└─────────────────────────────────────────────────────────────┘
```

### Module Structure

```
backends/foundation_core/src/wire/simple_http/
├── http2/
│   ├── mod.rs              # Module exports
│   ├── connection.rs       # Connection state machine
│   ├── codec.rs            # Frame encoding/decoding
│   ├── frames.rs           # Frame types
│   ├── hpack/
│   │   ├── mod.rs          # HPACK module
│   │   ├── decoder.rs      # Header decompression
│   │   ├── encoder.rs      # header compression
│   │   ├── static_table.rs # Static table
│   │   └── huffman.rs      # Huffman coding
│   ├── stream.rs           # Stream management
│   ├── error.rs            # HTTP/2 errors
│   └── client.rs           # HTTP/2 client
│   └── server.rs           # HTTP/2 server
```

---

## Technical Specification

### Frame Format (RFC 7540 Section 6)

```
+-----------------------------------------------+
|                 Length (24)                   |
+---------------+---------------+---------------+
|   Type (8)    |   Flags (8)   |
+-+-------------+---------------+-------------------------------+
|R|                 Stream Identifier (31)                      |
+=+=============================================================+
|                   Frame Payload (0...)
+---------------------------------------------------------------+
```

### Frame Types

| Type | Code | Description |
|------|------|-------------|
| DATA | 0x0 | Payload data |
| HEADERS | 0x1 | Request/response headers |
| PRIORITY | 0x2 | Stream priority (deferred) |
| RST_STREAM | 0x3 | Stream termination |
| SETTINGS | 0x4 | Connection parameters |
| PUSH_PROMISE | 0x5 | Server push (deferred) |
| PING | 0x6 | Connection liveness |
| GOAWAY | 0x7 | Connection shutdown |
| WINDOW_UPDATE | 0x8 | Flow control |
| CONTINUATION | 0x9 | Header continuation |

### Connection Preface

Client preface (24 bytes):
```
PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n
```

Followed by SETTINGS frame.

### HPACK Compression (RFC 7541)

- Static table: 61 predefined header fields
- Dynamic table: Connection-specific, up to 4096 bytes default
- Huffman coding for literal values

---

## API Design

### Client Usage

```rust
use foundation_core::wire::simple_http::http2::Http2Client;

// Create HTTP/2 client with TLS
let client = Http2Client::builder()
    .h2_settings(Settings {
        max_concurrent_streams: Some(100),
        initial_window_size: Some(65535),
        max_frame_size: Some(16384),
        ..Default::default()
    })
    .connect_tls("https://example.com")
    .await?;

// Multiplexed requests
let req1 = client.get("/api/users").send().await?;
let req2 = client.get("/api/posts").send().await?;
let req3 = client.get("/api/comments").send().await?;

// All three requests share the same connection
let (resp1, resp2, resp3) = tokio::join!(req1, req2, req3);
```

### Server Usage

```rust
use foundation_core::wire::simple_http::http2::Http2Server;

let server = Http2Server::builder()
    .h2_settings(Settings {
        max_concurrent_streams: Some(100),
        ..Default::default()
    })
    .serve(bind_addr, |req| async move {
        match req.path() {
            "/api/users" => handle_users(req).await,
            "/api/posts" => handle_posts(req).await,
            _ => Ok(Response::not_found()),
        }
    })
    .await?;
```

---

## Error Handling

### HTTP/2 Error Codes (RFC 7540 Section 7)

```rust
pub enum Http2Error {
    NoError,
    ProtocolError,
    InternalError,
    FlowControlError,
    SettingsTimeout,
    StreamClosed,
    FrameSizeError,
    RefusedStream,
    Cancel,
    CompressionError,
    ConnectError,
    EnhanceYourCalm,
    InadequateSecurity,
    Http11Required,
}
```

### Error Propagation

```rust
pub enum Http2ConnectionError {
    /// Framing error
    FrameError(Http2FrameError),
    /// HPACK compression error
    HpackError(HpackError),
    /// Protocol violation
    ProtocolViolation(Http2Error),
    /// Connection closed
    Closed,
    /// I/O error
    Io(std::io::Error),
}
```

---

## Implementation Phases

### Phase 1: Core Protocol (Current)

- Frame encoding/decoding
- Connection establishment
- Basic request/response
- HPACK compression
- Flow control

### Phase 2: Advanced Features

- Server push
- Stream prioritization
- Connection coalescing
- ALPN negotiation helpers

### Phase 3: Optimization

- Zero-copy frame parsing
- Header table optimization
- Write coalescing
- Connection pooling

---

## Dependencies

### New Crates

```toml
[dependencies]
# HPACK compression
hpack = { version = "0.3", optional = true }

# HTTP/2 specific
http = "1.0"  # HTTP semantics types
```

### Existing Crates

- `bytes` - Buffer management
- `tokio` - Async runtime
- `tokio-rustls` / `tokio-native-tls` - TLS

---

## Testing Strategy

### Unit Tests

- Frame encoding/decoding
- HPACK encode/decode roundtrip
- State machine transitions
- Flow control window management

### Integration Tests

- HTTP/2 server/client communication
- Multiplexing validation
- Interop with major HTTP/2 implementations (nghttp2, h2, hyper)

### Compliance Tests

- RFC 7540 conformance
- HTTP/2 test suite (https://github.com/http2/http2-test)

---

## Migration Path

### From HTTP/1.1

The existing `simple_http` API remains unchanged. HTTP/2 support is additive:

```rust
// Existing HTTP/1.1 code continues to work
let client = HttpClient::new();

// HTTP/2 is opt-in via builder
let client = HttpClient::builder()
    .http2()  // Enable HTTP/2
    .build();
```

### Feature Flag

```toml
[dependencies]
foundation_core = { version = "0.0.3", features = ["http2"] }
```

---

## Security Considerations

1. **TLS Required**: HTTP/2 over TLS only (no h2c cleartext in initial release)
2. **HPACK Bomb**: Limit dynamic table size
3. **Stream Exhaustion**: Limit concurrent streams
4. **Flow Control Attacks**: Monitor window sizes
5. **Frame Size Limits**: Enforce max frame size

---

## Performance Targets

| Metric | Target |
|--------|--------|
| Requests/sec (single connection) | 10,000+ |
| Latency (p99, local) | < 1ms |
| Memory per stream | < 4KB |
| HPACK compression ratio | > 50% |

---

## Open Questions

1. **Buffer pooling**: Should frames use pooled buffers?
2. **Task execution**: Use existing valtron task system or dedicated executor?
3. **Zero-copy**: How much zero-copy is feasible with HPACK?
4. **API compatibility**: Should HTTP/2 types be transparent or explicit?

---

## References

- [RFC 7540: HTTP/2 Protocol](https://datatracker.ietf.org/doc/html/rfc7540)
- [RFC 7541: HPACK Compression](https://datatracker.ietf.org/doc/html/rfc7541)
- [HTTP/2 Explained](https://http2-explained.haxx.se/)
- [nghttp2](https://nghttp2.org/)
- [tokio-rs/h2](https://github.com/hyperium/h2)

---

_Created: 2026-03-12_
_Author: Claude (AI Assistant)_
