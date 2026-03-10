---
workspace_name: "ewe_platform"
spec_directory: "specifications/02-build-http-client"
feature_directory: "specifications/02-build-http-client/features/websocket"
this_file: "specifications/02-build-http-client/features/websocket/feature.md"

status: in_progress
priority: low
created: 2026-02-28
updated: 2026-03-10

depends_on:
  - connection
  - public-api

tasks:
  completed: 4
  uncompleted: 3
  total: 7
  completion_percentage: 57
---

# WebSocket Feature Specification (RFC 6455)

## Table of Contents

1. [Overview](#1-overview)
2. [Dependencies and Existing Infrastructure](#2-dependencies-and-existing-infrastructure)
3. [Architectural Principles](#3-architectural-principles)
4. [WebSocket Protocol Reference (RFC 6455)](#4-websocket-protocol-reference-rfc-6455)
5. [Client API](#5-client-api)
6. [Server API](#6-server-api)
7. [Type Definitions](#7-type-definitions)
8. [Implementation Details](#8-implementation-details)
9. [Implementation Phases](#9-implementation-phases)
10. [Testing Strategy](#10-testing-strategy)
11. [Notes for Implementation Agents](#11-notes-for-implementation-agents)
12. [References](#12-references)

---

## 1. Overview

Add WebSocket protocol support (RFC 6455) to the `foundation_core` wire module, including both client (connect to WebSocket servers) and server (accept WebSocket upgrades) capabilities. WebSocket provides full-duplex communication over a single TCP connection through:

1. **Opening Handshake** — HTTP/1.1 Upgrade mechanism with key exchange
2. **Data Transfer** — Frame-based binary protocol with masking and fragmentation
3. **Closing Handshake** — Bidirectional close frame exchange

### Key Characteristics

- **Full-duplex**: Both endpoints can send and receive simultaneously
- **Framed protocol**: Messages are split into frames with explicit boundaries
- **Masked**: Client-to-server frames MUST be masked (XOR with random 4-byte key)
- **Fragmentation support**: Large messages can span multiple frames
- **Control frames**: Ping/Pong for keepalive, Close for graceful shutdown
- **Subprotocol negotiation**: Optional application-layer protocol selection
- **Extension support**: Optional compression and other extensions

### RFC 6455 Compliance Requirements

| Requirement | Client | Server |
|-------------|--------|--------|
| Mask outgoing frames | MUST | MUST NOT |
| Reject unmasked incoming | MUST | MUST (close 1002) |
| Reject masked incoming | MUST (close 1002) | MUST NOT receive |
| Support fragmentation | MUST | MUST |
| Auto-respond to Ping | MUST | MUST |
| Send Pong with same data | MUST | MUST |
| Validate UTF-8 in Text | MUST | MUST |
| Close on invalid UTF-8 | MUST (1007) | MUST (1007) |
| Support close codes | MUST | MUST |
| Validate Upgrade header | MUST | MUST |
| Compute Accept key | Client computes & validates | Server computes |

---

## 2. Dependencies and Existing Infrastructure

### Feature Dependencies

- `connection` — Uses `HttpClientConnection` for TCP/TLS connections
- `public-api` — Uses client infrastructure

### Existing Infrastructure Analysis

The `foundation_core` wire module provides excellent infrastructure for WebSocket implementation. ~80% of required infrastructure already exists.

#### 2.1 Connection Stack

```
HttpClientConnection
    └─> SharedByteBufferStream<RawStream>
            └─> RawStream (enum)
                    ├─> TCP (std::net::TcpStream)
                    └─> TLS (ClientSSLStream)
```

**Key characteristics:**
- `SharedByteBufferStream` provides `Arc<Mutex<BufferedStream<T>>>` for cloneable, thread-safe access
- Implements `Read + Write + BufRead`
- Supports setting read/write timeouts via `ReadTimeoutOperations` trait
- Can be used directly for WebSocket frame I/O after handshake
- Clone stream allows parallel read/write operations

#### 2.2 Wire Module Structure

```
backends/foundation_core/src/wire/
├── mod.rs                  # Module exports
├── simple_http/            # HTTP/1.1 implementation
│   ├── mod.rs
│   ├── errors.rs           # Error types
│   ├── impls.rs            # Core types (SimpleHeader, SimpleMethod, Status, Body)
│   ├── url/                # URL parsing (Uri, Scheme, Authority)
│   └── client/             # HTTP client implementation
│       ├── connection.rs   # HttpClientConnection, TLS upgrade
│       ├── pool.rs         # Connection pooling
│       ├── dns.rs          # DNS resolution
│       ├── request.rs      # Request building
│       ├── tasks/          # TaskIterator-based request execution
│       │   ├── send_request.rs    # Main request task (CANONICAL EXAMPLE)
│       │   └── ...
│       └── tls_task.rs     # TLS handshake tasks
├── event_source/           # Server-Sent Events — REFERENCE IMPLEMENTATION
└── websocket/              # WebSocket protocol (NEW)
    ├── mod.rs              # Public API
    ├── frame.rs            # Frame encoding/decoding
    ├── handshake.rs        # HTTP upgrade handshake
    ├── task.rs             # WebSocketTask (TaskIterator)
    ├── connection.rs       # WebSocketConnection + WebSocketClient
    ├── message.rs          # WebSocketMessage + MessageAssembler
    └── error.rs            # WebSocketError + FrameError
```

#### 2.3 Reusable Components

| Component | Source File | WebSocket Usage |
|-----------|-------------|-----------------|
| `SimpleHeader::SEC_WEBSOCKET_ACCEPT` | `simple_http/impls.rs` | Handshake validation |
| `SimpleHeader::SEC_WEBSOCKET_EXTENSIONS` | `simple_http/impls.rs` | Extension negotiation |
| `SimpleHeader::SEC_WEBSOCKET_KEY` | `simple_http/impls.rs` | Client key header |
| `SimpleHeader::SEC_WEBSOCKET_PROTOCOL` | `simple_http/impls.rs` | Subprotocol negotiation |
| `SimpleHeader::SEC_WEBSOCKET_VERSION` | `simple_http/impls.rs` | Version header |
| `SimpleHeader::UPGRADE` | `simple_http/impls.rs` | Upgrade header |
| `SimpleHeader::CONNECTION` | `simple_http/impls.rs` | Connection header |
| `SimpleHeader::HOST` | `simple_http/impls.rs` | Host header |
| `Status::SwitchingProtocols` | `simple_http/impls.rs` | 101 status validation |
| `HttpClientConnection` | `simple_http/client/connection.rs` | TCP/TLS connection wrapper |
| `SharedByteBufferStream<RawStream>` | `io/ioutils/mod.rs` | Frame I/O after handshake |
| `SimpleIncomingRequestBuilder` | `simple_http/impls.rs` | Build HTTP upgrade request |
| `SimpleIncomingResponse` | `simple_http/impls.rs` | Build HTTP upgrade response (server) |
| `HttpResponseReader` | `simple_http/impls.rs` | Parse 101 response |
| `Http11::request().http_render()` | `simple_http/impls.rs` | Render HTTP request bytes |
| `Http11::response().http_render()` | `simple_http/impls.rs` | Render HTTP response bytes |
| `HttpConnectionPool` | `simple_http/client/pool.rs` | Connection management, DNS caching |
| `Uri` | `simple_http/url/mod.rs` | URL parsing (extend for `ws://`/`wss://`) |
| `Scheme` | `simple_http/url/scheme.rs` | Extend for Ws, Wss variants |
| `DnsResolver` | `simple_http/client/dns.rs` | Hostname resolution |
| `TaskIterator` | `valtron/task.rs` | Core state machine pattern |
| `TaskStatus` | `valtron/task.rs` | State machine return type |
| `Stream` | `synca/mpp.rs` | Simplified stream values |
| `execute_stream()` | `valtron/executors/unified.rs` | Executor boundary function |
| `DrivenStreamIterator` | `valtron/executors/drivers.rs` | Executor-driven iterator |
| `ExponentialBackoffDecider` | `retries/exponential.rs` | Reconnection backoff |
| `BoxedSendExecutionAction` | `valtron/task.rs` | Spawner type for sub-tasks |

#### 2.4 HTTP Response Reading

Responses are read as an iterator over `IncomingResponseParts`:

```rust
pub enum IncomingResponseParts {
    Intro(Status, Proto, String),  // Status line
    Headers(SimpleHeaders),         // Headers
    SizedBody(SendSafeBody),       // Content-Length body
    StreamedBody(SendSafeBody),    // Chunked/streamed body
    NoBody,
    SKIP,
}
```

**Critical insight for WebSocket**: After receiving `Status::SwitchingProtocols` (101) and validating headers, we take ownership of the underlying `SharedByteBufferStream<RawStream>` and use it directly for WebSocket frame I/O.

#### 2.5 TLS Support

From `client/connection.rs`:

```rust
#[cfg(any(feature = "ssl-rustls", feature = "ssl-openssl", feature = "ssl-native-tls"))]
fn upgrade_to_tls(
    connection: Connection,
    host: &str,
    port: u16,
) -> Result<HttpClientConnection, HttpClientError> {
    let connector = SSLConnector::new();
    let (tls_stream, _addr) = connector
        .from_tcp_stream(host.to_string(), connection)
        .map_err(|e| HttpClientError::TlsHandshakeFailed(e.to_string()))?;

    let stream = SharedByteBufferStream::rwrite(
        RawStream::from_client_tls(tls_stream)?
    );

    Ok(HttpClientConnection { stream, host, port })
}
```

**Key takeaway for `wss://`**: TLS handshake happens BEFORE WebSocket handshake:
1. Connect TCP
2. Upgrade to TLS (reuse existing `HttpClientConnection` infrastructure)
3. Perform HTTP upgrade handshake over TLS connection
4. Start WebSocket framing over the TLS stream

### New Dependencies Required

```toml
[dependencies]
sha1 = "0.10"          # For Sec-WebSocket-Accept computation (SHA-1 hash)
rand = "0.8"           # For masking key generation (client frames)
base64 = "0.21"        # For base64 encoding/decoding of keys
```

**Why these specific versions:**
- `sha1 = "0.10"`: Provides `Sha1::digest()` and `Digest` trait
- `rand = "0.8"`: Provides `rand::random()` for cryptographically-secure random bytes
- `base64 = "0.21"`: Provides `Engine` trait with `STANDARD` encoding

---

## 3. Architectural Principles

### 3.1 TaskIterator with Executor Boundary

**CRITICAL**: All WebSocket client implementations MUST use `TaskIterator` as the core pattern with `unified::execute_stream()` as the executor boundary. See `specifications/02-build-http-client/LEARNINGS.md` for full valtron pattern reference.

#### Exact Valtron Types (from source)

**TaskIterator trait** (`valtron/task.rs`):
```rust
pub trait TaskIterator {
    type Pending;                    // State during async operations
    type Ready;                      // Completed result type
    type Spawner: ExecutionAction;   // For spawning sub-tasks

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>;
}
```

**TaskStatus enum** (`valtron/task.rs`):
```rust
pub enum TaskStatus<D, P, S: ExecutionAction> {
    /// Delay continued operation.
    Delayed(time::Duration),
    /// Spawn a new sub-task.
    Spawn(S),
    /// Still processing, not ready yet.
    Pending(P),
    /// Middle-point state (e.g. reconnecting).
    Init,
    /// Result available.
    Ready(D),
}
```

**Stream enum** (`synca/mpp.rs`) — what `DrivenStreamIterator` yields:
```rust
pub enum Stream<D, P> {
    Init,                        // Stream is instantiating
    Ignore,                      // Internal system operations (mapped from Spawn)
    Delayed(std::time::Duration), // Next response delayed
    Pending(P),                  // Stream in pending state
    Next(D),                     // Stream issued its next value (mapped from Ready)
}
```

**TaskStatus → Stream conversion** (automatic in valtron):
```rust
impl<D, P, S: ExecutionAction> From<TaskStatus<D, P, S>> for Stream<D, P> {
    fn from(val: TaskStatus<D, P, S>) -> Self {
        match val {
            TaskStatus::Init => Stream::Init,
            TaskStatus::Spawn(_) => Stream::Ignore,  // Spawn → Ignore
            TaskStatus::Ready(inner) => Stream::Next(inner),
            TaskStatus::Delayed(inner) => Stream::Delayed(inner),
            TaskStatus::Pending(inner) => Stream::Pending(inner),
        }
    }
}
```

**execute_stream()** (`valtron/executors/unified.rs`):
```rust
pub fn execute_stream<T>(
    task: T,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<DrivenStreamIterator<T>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
```

**DrivenStreamIterator** (`valtron/executors/drivers.rs`):
```rust
pub struct DrivenStreamIterator<T>(Option<StreamRecvIterator<T::Ready, T::Pending>>)
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static;

// Yields Stream<T::Ready, T::Pending>
impl<T> Iterator for DrivenStreamIterator<T> { ... }
```

#### Architecture Flow

```
WebSocketTask (TaskIterator)
    │  Pure state machine, ONE step per next()
    │  Returns TaskStatus<Ready, Pending, Spawner>
    ↓
unified::execute_stream(task, None)  ← EXECUTOR BOUNDARY
    │  Schedules task into valtron executor
    │  Handles Spawn, Delayed, threading
    │  Returns DrivenStreamIterator<WebSocketTask>
    ↓
DrivenStreamIterator
    │  Yields Stream<Ready, Pending>
    │  Spawn → Ignore (handled by executor)
    │  Ready → Next
    ↓
WebSocketClient / WebSocketConnection (consumer wrapper)
    │  Wraps DrivenStreamIterator
    │  Filters Init/Ignore/Pending/Delayed
    │  Returns only meaningful events (Next)
    ↓
User code: for msg in ws.messages() { ... }
```

#### Reference: SseStream Consumer Pattern (from `wire/event_source/consumer.rs`)

This is the canonical consumer wrapper pattern used in SSE and MUST be followed for WebSocket:

```rust
use crate::valtron::{execute_stream, DrivenStreamIterator, Stream};

pub struct SseStream<R: DnsResolver + Send + 'static> {
    inner: DrivenStreamIterator<EventSourceTask<R>>,
}

impl<R: DnsResolver + Send + 'static> SseStream<R> {
    pub fn connect(resolver: R, url: impl Into<String>) -> Result<Self, EventSourceError> {
        let task = EventSourceTask::connect(resolver, url)?;
        let inner = execute_stream(task, None)
            .map_err(|e| EventSourceError::Http(format!("Executor error: {}", e)))?;
        Ok(Self { inner })
    }
}

impl<R: DnsResolver + Send + 'static> Iterator for SseStream<R> {
    type Item = Result<SseStreamEvent, EventSourceError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(Stream::Next(parse_result)) => Some(Ok(SseStreamEvent::Event(parse_result.event))),
            Some(Stream::Pending(_)) => Some(Ok(SseStreamEvent::Skip)),
            Some(Stream::Delayed(_)) => Some(Ok(SseStreamEvent::Skip)),
            Some(Stream::Init) | Some(Stream::Ignore) => Some(Ok(SseStreamEvent::Skip)),
            None => None,
        }
    }
}
```

### 3.2 What NOT to Do

**WRONG — wrapping TaskIterator directly bypasses executor:**
```rust
// DO NOT: This bypasses executor's Spawn/Delayed/threading handling
pub struct BrokenWrapper {
    task: WebSocketTask,
}

impl Iterator for BrokenWrapper {
    fn next(&mut self) -> Option<Self::Item> {
        match self.task.next() {  // BYPASSES EXECUTOR
            Some(TaskStatus::Ready(r)) => Some(r),
            Some(TaskStatus::Spawn(_)) => self.next(), // Can't handle spawns
            Some(TaskStatus::Delayed(_)) => self.next(), // Busy-loops instead of waiting
            _ => None,
        }
    }
}
```

**Why this matters:**
- `TaskStatus::Spawn` — executor schedules sub-tasks; manual wrappers can't handle this
- `TaskStatus::Delayed(duration)` — executor respects timing; manual wrappers busy-loop or ignore
- `TaskStatus::Pending` — executor polls efficiently; manual wrappers waste CPU
- Threading — executor manages task across threads; direct wrapping is single-threaded

### 3.3 TaskIterator State Machine Pattern

From `simple_http/client/tasks/send_request.rs` (canonical example):

```rust
// 1. Task wraps state in Option for termination
pub struct SendRequestTask<R> {
    state: Option<SendRequestState<R>>,
}

// 2. State enum carries ALL data between transitions
pub enum SendRequestState<R> {
    Init(Option<Box<SendRequest<R>>>),
    Connecting(DrivenRecvIterator<GetHttpRequestRedirectTask<R>>),
    Reading(DrivenRecvIterator<GetRequestIntroTask>),
    SkipReading(Box<Option<RequestIntro>>),
    Done,
}

// 3. TaskIterator impl — ONE step per next(), NO loops
impl<R: DnsResolver + Send + 'static> TaskIterator for SendRequestTask<R> {
    type Ready = RequestIntro;
    type Pending = HttpRequestPending;
    type Spawner = BoxedSendExecutionAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // Pattern: take state, process ONE step, restore state
        match self.state.take()? {
            SendRequestState::Init(data) => { /* transition */ }
            SendRequestState::Connecting(recv) => { /* ONE step */ }
            // ...
        }
    }
}
```

**Key rules:**
1. `struct Task(Option<State>)` or `struct Task { state: Option<State> }` — Option wrapper for termination
2. State variants hold `Option<Box<...>>` — data carried between states
3. `self.state.take()?` or `self.0.take()?` — take state, return None if done
4. NO LOOPS in `next()` — state machine is step-wise, ONE transition per call
5. Data MOVED between states — not cloned
6. Sub-tasks composed via `DrivenRecvIterator` — receive results from spawned tasks

### 3.4 Connection Failure Handling

Connection failures must transition through intermediate states so tests can observe the failure progression:

```rust
// WRONG: Return None immediately on failure
let Ok(conn) = Connection::without_timeout(*addr) else {
    return None;  // Test can't observe the attempt
};

// CORRECT: Transition to intermediate state, return Pending first
let Ok(conn) = Connection::without_timeout(*addr) else {
    self.state = Some(MyState::Connecting);
    return Some(TaskStatus::Pending(MyPending::Connecting));
};
```

---

## 4. WebSocket Protocol Reference (RFC 6455)

### 4.1 Opening Handshake (Sections 4.1, 4.2)

#### Client Request

The client sends an HTTP/1.1 GET request with upgrade headers:

```http
GET /chat HTTP/1.1
Host: server.example.com
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
Sec-WebSocket-Version: 13
Sec-WebSocket-Protocol: chat, superchat  (optional)
Origin: http://example.com               (optional)
```

**Required headers (Section 4.1):**
- `Host` — MUST be present (HTTP/1.1 requirement)
- `Upgrade: websocket` — MUST be present, case-insensitive value
- `Connection: Upgrade` — MUST include "Upgrade" token
- `Sec-WebSocket-Key` — MUST be a base64-encoded 16-byte random value (nonce)
- `Sec-WebSocket-Version: 13` — MUST be exactly "13"

**Optional headers:**
- `Sec-WebSocket-Protocol` — comma-separated list of requested subprotocols
- `Sec-WebSocket-Extensions` — requested extensions (e.g., per-message deflate)
- `Origin` — browser origin for CORS-like checking

**Requirements:**
- HTTP method MUST be GET
- HTTP version MUST be >= 1.1
- The `Sec-WebSocket-Key` value MUST be a nonce consisting of a randomly selected 16-byte value, base64-encoded (RFC 4648). A new random value MUST be chosen for each connection.

#### Server Response

```http
HTTP/1.1 101 Switching Protocols
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=
Sec-WebSocket-Protocol: chat  (optional — MUST be a value from client's list)
```

**Required response headers (Section 4.2.2):**
- Status MUST be `101 Switching Protocols`
- `Upgrade: websocket` — MUST be present
- `Connection: Upgrade` — MUST be present
- `Sec-WebSocket-Accept` — MUST be the correctly computed accept value

**Sec-WebSocket-Accept computation:**
```rust
use sha1::{Digest, Sha1};
use base64::Engine;

fn compute_accept_key(client_key: &str) -> String {
    // The GUID is defined in RFC 6455 Section 4.2.2
    const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let mut hasher = Sha1::new();
    hasher.update(client_key.as_bytes());
    hasher.update(GUID.as_bytes());
    let hash = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(hash)
}
```

**Test vector (Section 1.3):**
```
Client key: dGhlIHNhbXBsZSBub25jZQ==
Expected accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=
```

**Client validation after receiving response:**
1. Status code MUST be 101
2. `Upgrade` header MUST be present with case-insensitive value "websocket"
3. `Connection` header MUST contain "Upgrade" token (case-insensitive)
4. `Sec-WebSocket-Accept` MUST match `compute_accept_key(client_key)`
5. If subprotocol was requested, `Sec-WebSocket-Protocol` MUST be one of the requested values
6. If the server returns a different subprotocol or extension, client MUST fail the connection

### 4.2 Frame Structure (Section 5.2)

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-------+-+-------------+-------------------------------+
|F|R|R|R| opcode|M| Payload len |    Extended payload length    |
|I|S|S|S|  (4)  |A|     (7)     |             (16/64)           |
|N|V|V|V|       |S|             |   (if payload len==126/127)   |
| |1|2|3|       |K|             |                               |
+-+-+-+-+-------+-+-------------+ - - - - - - - - - - - - - - - +
|     Extended payload length continued, if payload len == 127  |
+ - - - - - - - - - - - - - - - +-------------------------------+
|                               |Masking-key, if MASK set to 1  |
+-------------------------------+-------------------------------+
| Masking-key (continued)       |          Payload Data         |
+-------------------------------- - - - - - - - - - - - - - - - +
:                     Payload Data continued ...                :
+ - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - +
|                     Payload Data continued ...                |
+---------------------------------------------------------------+
```

**Field details:**

- **FIN** (1 bit): `1` if this is the final fragment of a message, `0` for continuation
- **RSV1, RSV2, RSV3** (1 bit each): MUST be `0` unless an extension is negotiated that defines non-zero values. If a non-zero value is received and no extension defines it, the receiving endpoint MUST fail the connection.
- **Opcode** (4 bits): Frame type (see opcodes table)
- **MASK** (1 bit): `1` if payload is masked. Client-to-server frames MUST set this to `1`. Server-to-client frames MUST set this to `0`.
- **Payload length** (7 bits): Determines how to read the actual length:
  - `0-125`: This IS the payload length
  - `126`: The following 2 bytes (unsigned, big-endian) are the payload length
  - `127`: The following 8 bytes (unsigned, big-endian) are the payload length. The most significant bit MUST be 0 (max length is 2^63).
- **Masking-key** (4 bytes): Only present if MASK is `1`. Used to XOR the payload.
- **Payload Data**: Extension data (if any) followed by Application data.

### 4.3 Opcodes (Section 5.2)

| Opcode | Name | Type | Description |
|--------|------|------|-------------|
| `0x0` | Continuation | Data | Continuation frame for fragmented message |
| `0x1` | Text | Data | UTF-8 text message (MUST be valid UTF-8) |
| `0x2` | Binary | Data | Binary data message |
| `0x3-0x7` | Reserved | Data | Reserved for future non-control frames |
| `0x8` | Close | Control | Connection close |
| `0x9` | Ping | Control | Ping (heartbeat/keepalive) |
| `0xA` | Pong | Control | Pong (response to Ping) |
| `0xB-0xF` | Reserved | Control | Reserved for future control frames |

### 4.4 Masking (Section 5.3)

**Why masking exists:** To prevent cache poisoning attacks where a malicious client could trick intermediary proxies into caching WebSocket frames as HTTP responses.

**Algorithm:**
```rust
fn apply_mask(data: &mut [u8], mask: [u8; 4]) {
    for (i, byte) in data.iter_mut().enumerate() {
        *byte ^= mask[i % 4];
    }
}
```

**Rules:**
- Client-to-server frames **MUST** be masked with a fresh random 4-byte mask per frame
- Server-to-client frames **MUST NOT** be masked
- The masking key MUST be unpredictable; implementations SHOULD use a strong source of randomness
- If a server receives an unmasked frame from a client: **MUST fail the connection** and send Close with status code 1002 (Protocol Error)
- If a client receives a masked frame from a server: **MUST fail the connection**
- The masking operation is its own inverse — same function is used for masking and unmasking

### 4.5 Message Fragmentation (Section 5.4)

Large messages can be split across multiple frames:

```
Message = Frame₁(FIN=0, op=Text/Binary) + Frame₂(FIN=0, op=Continuation) + ... + FrameN(FIN=1, op=Continuation)
```

**Rules:**
- First frame: `FIN=0`, opcode = `Text` (0x1) or `Binary` (0x2)
- Middle frames: `FIN=0`, opcode = `Continuation` (0x0)
- Last frame: `FIN=1`, opcode = `Continuation` (0x0)
- Single-frame (unfragmented) message: `FIN=1`, opcode = `Text` or `Binary`
- Control frames (Ping/Pong/Close) **MUST NOT** be fragmented
- Control frames **CAN** be interleaved between data fragments — receivers MUST handle this
- An intermediary MAY coalesce or split data fragments (but MUST NOT change the opcode)
- An endpoint MUST NOT send a new data message until the current fragmented message is complete

### 4.6 Control Frames (Section 5.5)

**General rules for ALL control frames:**
- MUST NOT be fragmented (FIN MUST be 1)
- MUST have payload length ≤ 125 bytes
- CAN be injected between fragments of a data message

#### Close Frame (Section 5.5.1)

- MAY contain a body with a 2-byte unsigned big-endian status code followed by optional UTF-8 reason text
- Total body ≤ 125 bytes, so reason text ≤ 123 bytes
- After sending a Close frame, the endpoint MUST NOT send any more data frames
- After receiving a Close frame, the peer MUST send a Close frame in response (echo the status code)
- After both sides have sent and received Close, the TCP connection SHOULD be closed
- The endpoint that initiated the Close (first sender) SHOULD let the other side close the TCP connection
- If a Close frame is not received within a reasonable time after sending one, the endpoint MAY close the TCP connection

#### Ping Frame (Section 5.5.2)

- MAY include application data (≤ 125 bytes)
- Upon receiving a Ping, the endpoint MUST send a Pong as soon as practical with the same application data
- If an endpoint receives a Ping before sending Pong for a previous Ping, it MAY send Pong only for the most recent Ping
- Used for keep-alive and RTT measurement

#### Pong Frame (Section 5.5.3)

- MUST contain the same application data as the Ping it responds to
- Unsolicited Pong frames (sent without a preceding Ping) serve as unidirectional heartbeats — the recipient MUST ignore them (no error, no response)

### 4.7 Close Status Codes (Section 7.4)

| Code | Name | Description | Sendable? |
|------|------|-------------|-----------|
| 1000 | Normal Closure | Normal shutdown | Yes |
| 1001 | Going Away | Server shutting down or browser navigating away | Yes |
| 1002 | Protocol Error | Terminated due to protocol error | Yes |
| 1003 | Unsupported Data | Received data type it cannot accept (e.g., text-only endpoint got binary) | Yes |
| 1004 | Reserved | Reserved for future use | **NO** — MUST NOT be set as status code |
| 1005 | No Status Received | Indicates no status code was present in Close frame | **NO** — MUST NOT be sent |
| 1006 | Abnormal Closure | Connection closed without a Close frame | **NO** — MUST NOT be sent |
| 1007 | Invalid Frame Payload Data | Received non-UTF-8 data in a text message | Yes |
| 1008 | Policy Violation | Generic policy violation when no other code applies | Yes |
| 1009 | Message Too Big | Message too large to process | Yes |
| 1010 | Mandatory Extension | Client expected server to negotiate extension(s) | Yes |
| 1011 | Internal Error | Server encountered unexpected condition | Yes |
| 1015 | TLS Handshake | TLS handshake failure (e.g., certificate error) | **NO** — MUST NOT be sent |
| 3000-3999 | Library/Framework | Reserved for libraries, frameworks, and applications. Must be registered with IANA. | Context-dependent |
| 4000-4999 | Private Use | Available for applications. NOT registered. | Yes |

### 4.8 Security Considerations (Section 10)

- **Origin checking**: Servers SHOULD verify the `Origin` header to prevent cross-site WebSocket hijacking
- **Masking rationale**: Prevents cache poisoning via intermediary confusion (attacker can't predict frame bytes to craft cacheable responses)
- **UTF-8 validation**: Text frames MUST contain valid UTF-8 — invalid data MUST trigger close with code 1007
- **Frame size limits**: Implementations SHOULD enforce maximum frame/message size to prevent DoS (recommend configurable, default 16MB)
- **Close codes**: Use standard codes (1000-1011) — don't expose internal errors. Reserved codes (1004, 1005, 1006, 1015) MUST NOT be sent in Close frames.
- **Random masking**: Use cryptographically secure random for mask generation (client-side) to prevent prediction attacks

---

## 5. Client API

### 5.1 Core API (TaskIterator via Executor)

```rust
use foundation_core::wire::websocket::WebSocketTask;
use foundation_core::valtron::{TaskIterator, TaskStatus, Stream};
use foundation_core::valtron::executors::unified;

// Create task and schedule into executor
let task = WebSocketTask::connect("wss://example.com/ws")?;
let iter = unified::execute_stream(task, None)?;

for status in iter {
    match status {
        Stream::Next(Ok(WebSocketMessage::Text(text))) => println!("{}", text),
        Stream::Next(Ok(WebSocketMessage::Binary(data))) => handle_binary(data),
        Stream::Next(Ok(WebSocketMessage::Ping(data))) => { /* auto-pong handled internally */ }
        Stream::Next(Ok(WebSocketMessage::Close(code, reason))) => {
            println!("Closed: {} - {}", code, reason);
            break;
        }
        Stream::Pending(_) | Stream::Delayed(_) | Stream::Init | Stream::Ignore => continue,
    }
}
```

### 5.2 Consumer API (Blocking Convenience Wrapper with Send/Receive)

```rust
use foundation_core::wire::websocket::{WebSocketClient, WebSocketClientBuilder, WebSocketMessage, MessageDelivery};
use std::time::Duration;

// Simple connect - returns BOTH client and delivery handle for sending messages
let (mut ws, delivery) = WebSocketClient::connect("wss://example.com/ws")?;

// OR use builder for customization (read timeout, etc.)
let (mut ws, delivery) = WebSocketClient::builder("wss://example.com/ws")
    .read_timeout(Duration::from_secs(5))  // Timeout for recv() calls
    .subprotocol("graphql-ws")
    .connect()?;

// Send messages via delivery handle (Clone + Send + 'static, can be shared)
delivery.send(WebSocketMessage::Text("hello".into()))?;
delivery.send(WebSocketMessage::Binary(vec![1, 2, 3]))?;

// Receive messages via client iterator
for event in ws.messages() {
    match event? {
        WebSocketMessage::Text(text) => println!("{}", text),
        WebSocketMessage::Binary(data) => handle_binary(data),
        WebSocketMessage::Ping(data) => {
            // Auto-pong handled internally, but user sees the Ping
            delivery.pong(data)?;  // Manual pong if needed
        }
        WebSocketMessage::Pong(_) => {},
        WebSocketMessage::Close(code, reason) => {
            println!("Closed: {} - {}", code, reason);
            break;
        }
    }
}

// Graceful close
delivery.close(1000, "goodbye")?;
```

**Architecture (WHY this design):**

The `WebSocketClient` uses an internal `ConcurrentQueue` channel to decouple sending from receiving while maintaining the TaskIterator pattern:

```
User Code
    │
    ├─> delivery.send(msg) ──> MessageDelivery ──> ConcurrentQueue<WebSocketMessage>
    │                                                      │
    │                                                      v
    │                                          WebSocketTask reads queue
    │                                                      │
    │                                                      v
    │                                          sends frame to connection
    │
    └─> ws.messages() <── WebSocketClient iterator <── receives from connection
```

**MessageDelivery design:**
- Wrapper around `Arc<ConcurrentQueue<WebSocketMessage>>`
- `Clone + Send + 'static` - can be shared across threads
- Provides `send()`, `pong()`, and `close()` methods
- Created together with `WebSocketClient` - cannot exist independently

**WebSocketClientBuilder:**
```rust
pub struct WebSocketClientBuilder {
    url: String,
    subprotocols: Option<String>,
    extra_headers: Vec<(SimpleHeader, String)>,
    read_timeout: Duration,  // Default: 1 second
}

impl WebSocketClientBuilder {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            subprotocols: None,
            extra_headers: Vec::new(),
            read_timeout: Duration::from_secs(1),  // Default
        }
    }

    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout;
        self
    }

    pub fn subprotocol(mut self, protocol: impl Into<String>) -> Self {
        self.subprotocols = Some(protocol.into());
        self
    }

    pub fn header(mut self, key: SimpleHeader, value: impl Into<String>) -> Self {
        self.extra_headers.push((key, value.into()));
        self
    }

    pub fn connect<R: DnsResolver + Send + 'static>(
        self,
    ) -> Result<(WebSocketClient<R>, MessageDelivery), WebSocketError> {
        WebSocketClient::with_options(self.url, self.subprotocols, self.extra_headers, self.read_timeout)
    }
}
```

**WebSocketClient internal loop:**
```rust
use concurrent_queue::ConcurrentQueue;
use std::sync::Arc;

pub struct WebSocketClient<R: DnsResolver + Send + 'static> {
    inner: DrivenStreamIterator<WebSocketTask<R>>,
    delivery: MessageDelivery,
    read_timeout: Duration,
}

pub struct MessageDelivery {
    queue: Arc<ConcurrentQueue<WebSocketMessage>>,
}

impl MessageDelivery {
    pub fn send(&self, msg: WebSocketMessage) -> Result<(), WebSocketError> {
        self.queue.push(msg).map_err(|_| WebSocketError::ConnectionClosed)?;
        Ok(())
    }

    pub fn close(&self, code: u16, reason: &str) -> Result<(), WebSocketError> {
        self.send(WebSocketMessage::Close(code, reason.to_string()))
    }

    pub fn pong(&self, data: Vec<u8>) -> Result<(), WebSocketError> {
        self.send(WebSocketMessage::Pong(data))
    }
}
```

**WebSocketTask reads from delivery queue:**
```rust
impl TaskIterator for WebSocketTask<R> {
    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.state {
            // ... handshake states ...

            WebSocketState::Open { ref mut stream, ref delivery_queue, read_timeout } => {
                // 1. Check delivery queue for outgoing messages (non-blocking)
                if let Ok(msg) = delivery_queue.pop() {
                    // Encode and send frame
                    let frame = msg.into_frame(Role::Client);
                    match stream.write_all(frame.encode()) {
                        Ok(_) => {
                            self.state = Some(WebSocketState::Open { stream, delivery_queue, read_timeout });
                            return Some(TaskStatus::Pending(())); // Yield, try again next call
                        }
                        Err(e) => {
                            self.state = Some(WebSocketState::Closed(Some(e.into())));
                            return None;
                        }
                    }
                }

                // 2. Read incoming frame from connection (uses configurable timeout)
                stream.set_read_timeout(read_timeout).ok()?;
                match WebSocketFrame::decode(&mut *stream) {
                    Ok(frame) => {
                        // Validate and process frame
                        let result = frame.validate()
                            .map_err(Into::into)
                            .and_then(|_| Ok(frame.into_message()?));
                        self.state = Some(WebSocketState::Open { stream, delivery_queue, read_timeout });
                        return Some(TaskStatus::Ready(result));
                    }
                    Err(WebSocketError::IoError(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No incoming message, continue checking delivery next call
                        self.state = Some(WebSocketState::Open { stream, delivery_queue, read_timeout });
                        return Some(TaskStatus::Pending(()));
                    }
                    Err(e) => {
                        self.state = Some(WebSocketState::Closed(Some(e)));
                        return None;
                    }
                }
            }
        }
    }
}
```

**Key benefits:**
1. Uses existing `ConcurrentQueue` from valtron infrastructure - no new dependencies
2. Maintains TaskIterator purity - no loops, one step per `next()` call
3. Full-duplex simulation - delivery queue checked each iteration
4. Send handle is shareable - multiple threads can send to same connection
5. Clean API separation - send via `MessageDelivery`, receive via iterator
6. Configurable read timeout - user can tune for their use case
7. No executor bypass - all through `unified::execute_stream()`

### 5.3 With Subprotocol and Options

```rust
let mut ws = WebSocketConnection::builder("wss://example.com/ws")
    .subprotocol("graphql-ws")
    .subprotocols(&["chat", "superchat"])
    .header("Authorization", "Bearer token123")
    .connect()?;
```

### 5.4 Connection Pooling Integration

**WHY:** WebSocket connections can benefit from the existing `HttpConnectionPool` infrastructure for:
- DNS resolution caching
- TCP connection reuse (for rapid reconnections)
- Unified connection management

**WHAT:** `HttpConnectionPool` provides the underlying TCP/TLS connection, and WebSocket performs its handshake on top.

**HOW:** The pool's `create_http_connection()` method returns an `HttpClientConnection` with an established TCP/TLS connection. WebSocket then performs its HTTP upgrade handshake over this connection.

```rust
use std::sync::Arc;
use crate::wire::simple_http::client::HttpConnectionPool;

// Using connection pool
let pool = Arc::new(HttpConnectionPool::default());
let (mut ws, delivery) = WebSocketClient::with_pool("ws://example.com/ws", pool)?;
```

### 5.5 Pool Architecture for WebSocket Connections

**Design Rationale:**

The existing `HttpConnectionPool` is designed for short-lived HTTP request/response cycles. WebSocket connections are long-lived, full-duplex connections that stay open for extended periods. This requires a different pooling strategy:

1. **HTTP Pooling Pattern** (existing):
   - Connections are checked out, used for a request, and returned to pool
   - Idle timeout ~300s, max 10 per host
   - Many short-lived connections per host

2. **WebSocket Pooling Pattern** (new):
   - Connections are checked out and NOT returned (long-lived)
   - Pool serves as connection factory with DNS caching
   - Few long-lived connections per host

**Implementation Strategy:**

The `HttpConnectionPool` is reused as-is for WebSocket connections. The key difference is semantic:
- For HTTP: `checkout()` → request → `checkin()`
- For WebSocket: `checkout()` → upgrade to WebSocket → connection stays open (never checked back in)

This means WebSocket connections effectively bypass the pooling aspect but still benefit from:
- DNS resolution caching via the pool's resolver
- Any existing pooled connections (if WebSocket upgrades quickly)
- Unified connection creation logic

**Future Enhancement (Optional):**

A dedicated `WebSocketConnectionPool` could be added for managing pools of idle WebSocket connections (e.g., for connection warm-up or multiplexing scenarios). This is out of scope for Phase 1.

---

## 6. Server API

### 6.1 Accept WebSocket Upgrade

```rust
fn handle_request(request: IncomingRequest, conn: &mut HttpClientConnection) -> Result<(), Error> {
    if WebSocketUpgrade::is_upgrade_request(&request) {
        let ws = WebSocketUpgrade::accept(request, conn)?;
        handle_websocket(ws)
    } else {
        handle_http(request, conn)
    }
}

fn handle_websocket(mut ws: WebSocketConnection) -> Result<(), Error> {
    for message in ws.messages() {
        ws.send(message?)?; // Echo server
    }
    Ok(())
}
```

### 6.2 Server Upgrade Implementation

```rust
pub struct WebSocketUpgrade;

impl WebSocketUpgrade {
    pub fn is_upgrade_request(request: &IncomingRequest) -> bool {
        request.headers.get(SimpleHeader::UPGRADE)
            .map(|v| v.eq_ignore_ascii_case("websocket"))
            .unwrap_or(false)
            && request.headers.get(SimpleHeader::CONNECTION)
                .map(|v| v.to_lowercase().contains("upgrade"))
                .unwrap_or(false)
    }

    pub fn accept(
        request: IncomingRequest,
        conn: &mut HttpClientConnection,
    ) -> Result<WebSocketConnection, WebSocketError> {
        // Extract client key
        let key = request.headers.get(SimpleHeader::SEC_WEBSOCKET_KEY)
            .ok_or(WebSocketError::MissingKey)?;

        // Compute accept key
        let accept = compute_accept_key(key);

        // Get negotiated subprotocol (pick first supported)
        let subprotocol = request.headers.get(SimpleHeader::SEC_WEBSOCKET_PROTOCOL);

        // Build 101 Switching Protocols response
        let mut response = SimpleIncomingResponse::new(Status::SwitchingProtocols);
        response.headers.insert(SimpleHeader::UPGRADE, "websocket");
        response.headers.insert(SimpleHeader::CONNECTION, "Upgrade");
        response.headers.insert(SimpleHeader::SEC_WEBSOCKET_ACCEPT, accept);

        if let Some(protocol) = subprotocol {
            response.headers.insert(SimpleHeader::SEC_WEBSOCKET_PROTOCOL, protocol);
        }

        // Render and send response using RenderHttp
        let response_bytes = Http11::response(&response).http_render()?;
        for chunk in response_bytes {
            conn.write_all(&chunk?)?;
        }
        conn.flush()?;

        // Server MUST NOT mask outgoing frames
        Ok(WebSocketConnection::new(conn, Role::Server))
    }
}
```

---

## 7. Type Definitions

### 7.1 Message Types

```rust
pub enum WebSocketMessage {
    /// UTF-8 text message (MUST be valid UTF-8, validated on receive)
    Text(String),
    /// Binary data message
    Binary(Vec<u8>),
    /// Ping frame (receiver must respond with Pong containing same payload)
    Ping(Vec<u8>),
    /// Pong frame (response to Ping, or unsolicited heartbeat)
    Pong(Vec<u8>),
    /// Close frame with status code and reason
    Close(u16, String),
}
```

### 7.2 Frame Types

```rust
pub struct WebSocketFrame {
    pub fin: bool,
    pub rsv1: bool,       // MUST be 0 unless extension defines it
    pub rsv2: bool,       // MUST be 0 unless extension defines it
    pub rsv3: bool,       // MUST be 0 unless extension defines it
    pub opcode: Opcode,
    pub mask: Option<[u8; 4]>,  // Some for client frames, None for server frames
    pub payload: Vec<u8>,
}

#[repr(u8)]
pub enum Opcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

impl Opcode {
    pub fn is_control(&self) -> bool {
        matches!(self, Self::Close | Self::Ping | Self::Pong)
    }

    pub fn is_data(&self) -> bool {
        matches!(self, Self::Text | Self::Binary | Self::Continuation)
    }

    pub fn from_u8(value: u8) -> Result<Self, WebSocketError> {
        match value {
            0x0 => Ok(Self::Continuation),
            0x1 => Ok(Self::Text),
            0x2 => Ok(Self::Binary),
            0x8 => Ok(Self::Close),
            0x9 => Ok(Self::Ping),
            0xA => Ok(Self::Pong),
            other => Err(WebSocketError::InvalidOpcode(other)),
        }
    }
}
```

### 7.3 Connection Types

```rust
pub struct WebSocketConnection {
    stream: SharedByteBufferStream<RawStream>,
    role: Role,
    state: ConnectionState,
    assembler: MessageAssembler,
}

pub enum Role {
    Client,  // MUST mask outgoing frames
    Server,  // MUST NOT mask outgoing frames
}

enum ConnectionState {
    Open,
    Closing { close_sent: bool, close_received: bool },
    Closed,
}
```

### 7.4 Subprotocol and Options

```rust
pub struct WebSocketOptions {
    pub subprotocols: Vec<String>,
    pub extensions: Vec<String>,
    pub extra_headers: Vec<(String, String)>,
}
```

### 7.5 Error Types

```rust
#[derive(Debug)]
pub enum WebSocketError {
    /// Upgrade request failed (non-101 status)
    UpgradeFailed(u16),
    /// Invalid Sec-WebSocket-Accept header value
    InvalidAcceptKey,
    /// Missing Sec-WebSocket-Accept header in response
    MissingAcceptKey,
    /// Missing Sec-WebSocket-Key header in request (server-side)
    MissingKey,
    /// Missing Upgrade header in response
    MissingUpgradeHeader,
    /// Invalid Upgrade header value (not "websocket")
    InvalidUpgradeHeader,
    /// Missing Connection header in response
    MissingConnectionHeader,
    /// Invalid Connection header (doesn't contain "Upgrade")
    InvalidConnectionHeader,
    /// Missing response headers entirely
    MissingHeaders,
    /// Invalid URL scheme (not ws:// or wss://)
    InvalidScheme,
    /// Invalid frame format
    InvalidFrame(String),
    /// Invalid opcode received
    InvalidOpcode(u8),
    /// Control frame too large (> 125 bytes)
    ControlFrameTooLarge,
    /// Control frame is fragmented (FIN=0)
    FragmentedControlFrame,
    /// Unexpected continuation frame (no fragmented message in progress)
    UnexpectedContinuation,
    /// Expected continuation but got new data frame
    ExpectedContinuation,
    /// Invalid UTF-8 in text message
    InvalidUtf8(std::string::FromUtf8Error),
    /// Received unmasked frame from client (server-side)
    UnmaskedClientFrame,
    /// Received masked frame from server (client-side)
    MaskedServerFrame,
    /// Non-zero RSV bit without negotiated extension
    UnexpectedRsvBit,
    /// Connection closed unexpectedly
    ConnectionClosed,
    /// Handshake failed
    HandshakeFailed(String),
    /// Protocol error
    ProtocolError(String),
    /// HTTP-level error during handshake
    HttpError(String),
    /// IO error
    IoError(std::io::Error),
    /// Connection establishment failed
    ConnectionFailed(String),
    /// Message exceeds maximum size
    MessageTooLarge(usize),
}

pub enum FrameError {
    InvalidOpcode(u8),
    ControlFrameTooLarge,
    FragmentedControlFrame,
    UnexpectedContinuation,
    ExpectedContinuation,
    InvalidUtf8,
    InvalidClosePayload,
    IoError(std::io::Error),
}
```

---

## 8. Implementation Details

### 8.1 Frame Encoding

```rust
impl WebSocketFrame {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Byte 0: FIN, RSV, Opcode
        let byte0 = (self.fin as u8) << 7
                  | (self.rsv1 as u8) << 6
                  | (self.rsv2 as u8) << 5
                  | (self.rsv3 as u8) << 4
                  | (self.opcode as u8);
        buf.push(byte0);

        // Byte 1+: MASK, Payload length
        let payload_len = self.payload.len();
        let mask_bit = if self.mask.is_some() { 0x80 } else { 0x00 };

        if payload_len < 126 {
            buf.push(mask_bit | (payload_len as u8));
        } else if payload_len <= 65535 {
            buf.push(mask_bit | 126);
            buf.extend_from_slice(&(payload_len as u16).to_be_bytes());
        } else {
            buf.push(mask_bit | 127);
            buf.extend_from_slice(&(payload_len as u64).to_be_bytes());
        }

        // Masking key (if present)
        if let Some(mask) = self.mask {
            buf.extend_from_slice(&mask);
        }

        // Payload (apply mask if needed)
        if let Some(mask) = self.mask {
            let mut masked_payload = self.payload.clone();
            apply_mask(&mut masked_payload, mask);
            buf.extend_from_slice(&masked_payload);
        } else {
            buf.extend_from_slice(&self.payload);
        }

        buf
    }
}
```

### 8.2 Frame Decoding

```rust
impl WebSocketFrame {
    pub fn decode(reader: &mut impl Read) -> Result<Self, WebSocketError> {
        // Read byte 0 (FIN, RSV, Opcode)
        let byte0 = read_u8(reader)?;
        let fin = (byte0 & 0x80) != 0;
        let rsv1 = (byte0 & 0x40) != 0;
        let rsv2 = (byte0 & 0x20) != 0;
        let rsv3 = (byte0 & 0x10) != 0;
        let opcode = Opcode::from_u8(byte0 & 0x0F)?;

        // Read byte 1 (MASK, Payload length)
        let byte1 = read_u8(reader)?;
        let mask_bit = (byte1 & 0x80) != 0;
        let mut payload_len = (byte1 & 0x7F) as u64;

        // Extended payload length
        if payload_len == 126 {
            payload_len = read_u16(reader)? as u64;
        } else if payload_len == 127 {
            payload_len = read_u64(reader)?;
            // MSB MUST be 0 (Section 5.2)
            if payload_len >> 63 != 0 {
                return Err(FrameError::InvalidPayloadLength);
            }
        }

        // Validate control frame payload length
        if opcode.is_control() && payload_len > 125 {
            return Err(FrameError::ControlFrameTooLarge);
        }

        // Validate control frame FIN bit
        if opcode.is_control() && !fin {
            return Err(FrameError::FragmentedControlFrame);
        }

        // Read masking key
        let mask = if mask_bit {
            let mut mask_bytes = [0u8; 4];
            reader.read_exact(&mut mask_bytes)?;
            Some(mask_bytes)
        } else {
            None
        };

        // Read payload
        let mut payload = vec![0u8; payload_len as usize];
        reader.read_exact(&mut payload)?;

        // Unmask payload
        if let Some(mask) = mask {
            apply_mask(&mut payload, mask);
        }

        Ok(WebSocketFrame { fin, rsv1, rsv2, rsv3, opcode, mask, payload })
    }
}
```

### 8.3 Message Assembly

```rust
pub struct MessageAssembler {
    fragments: Vec<Vec<u8>>,
    opcode: Option<Opcode>,
}

impl MessageAssembler {
    pub fn push_frame(&mut self, frame: WebSocketFrame) -> Result<Option<WebSocketMessage>, WebSocketError> {
        // Control frames are NEVER fragmented — handle immediately
        if frame.opcode.is_control() {
            return Ok(Some(WebSocketMessage::from_control_frame(frame)?));
        }

        // First data frame
        if self.opcode.is_none() {
            if frame.opcode == Opcode::Continuation {
                return Err(WebSocketError::UnexpectedContinuation);
            }
            self.opcode = Some(frame.opcode);
        } else {
            // Continuation frame
            if frame.opcode != Opcode::Continuation {
                return Err(WebSocketError::ExpectedContinuation);
            }
        }

        self.fragments.push(frame.payload);

        // Final frame?
        if frame.fin {
            let opcode = self.opcode.take().unwrap();
            let data: Vec<u8> = self.fragments.drain(..).flatten().collect();

            let message = match opcode {
                Opcode::Text => {
                    let text = String::from_utf8(data)
                        .map_err(|_| WebSocketError::InvalidUtf8)?;
                    WebSocketMessage::Text(text)
                }
                Opcode::Binary => WebSocketMessage::Binary(data),
                _ => return Err(WebSocketError::InvalidOpcode(opcode as u8)),
            };

            Ok(Some(message))
        } else {
            Ok(None)  // More fragments needed
        }
    }
}
```

### 8.4 Close Payload Parsing

```rust
fn parse_close_payload(payload: &[u8]) -> Result<(u16, String), WebSocketError> {
    if payload.is_empty() {
        return Ok((1005, String::new()));  // No status code present
    }
    if payload.len() == 1 {
        return Err(WebSocketError::InvalidClosePayload);  // Must be 0 or >= 2 bytes
    }

    let code = u16::from_be_bytes([payload[0], payload[1]]);
    let reason = if payload.len() > 2 {
        String::from_utf8(payload[2..].to_vec())
            .map_err(|_| WebSocketError::InvalidUtf8)?
    } else {
        String::new()
    };

    Ok((code, reason))
}
```

### 8.5 WebSocket Handshake (Client)

Using `SimpleIncomingRequestBuilder` (NOT `ClientRequestBuilder` — connection is already established):

```rust
pub fn handshake(
    conn: &mut HttpClientConnection,
    uri: &Uri,
    options: &WebSocketOptions,
) -> Result<(), WebSocketError> {
    // Generate random 16-byte key, base64 encoded
    let key = generate_websocket_key();

    // Build HTTP upgrade request using SimpleIncomingRequestBuilder
    let mut builder = SimpleIncomingRequestBuilder::get(uri.path_and_query())
        .header(SimpleHeader::HOST, uri.host_with_port())
        .header(SimpleHeader::UPGRADE, "websocket")
        .header(SimpleHeader::CONNECTION, "Upgrade")
        .header(SimpleHeader::SEC_WEBSOCKET_KEY, &key)
        .header(SimpleHeader::SEC_WEBSOCKET_VERSION, "13");

    // Add subprotocol if requested
    if !options.subprotocols.is_empty() {
        let protocols = options.subprotocols.join(", ");
        builder = builder.header(SimpleHeader::SEC_WEBSOCKET_PROTOCOL, &protocols);
    }

    // Add extra headers
    for (name, value) in &options.extra_headers {
        builder = builder.header(SimpleHeader::custom(name), value);
    }

    let request = builder.build()?;

    // Render and send using RenderHttp
    let request_bytes = Http11::request(&request).http_render()?;
    for chunk in request_bytes {
        conn.write_all(&chunk?)?;
    }
    conn.flush()?;

    // Read response using HttpResponseReader
    let reader = HttpResponseReader::new(conn.clone_stream(), SimpleHttpBody);
    let mut headers = None;

    for part in reader {
        match part? {
            IncomingResponseParts::Intro(status, _proto, _text) => {
                if status != Status::SwitchingProtocols {
                    return Err(WebSocketError::UpgradeFailed(status.into_usize()));
                }
            }
            IncomingResponseParts::Headers(h) => {
                headers = Some(h);
                break;
            }
            _ => {}
        }
    }

    let headers = headers.ok_or(WebSocketError::MissingHeaders)?;

    // Validate response headers
    validate_upgrade_response(&headers, &key)?;

    Ok(())
}

fn generate_websocket_key() -> String {
    use base64::Engine;
    let random_bytes: [u8; 16] = rand::random();
    base64::engine::general_purpose::STANDARD.encode(random_bytes)
}

fn validate_upgrade_response(headers: &SimpleHeaders, key: &str) -> Result<(), WebSocketError> {
    // Check Upgrade header
    let upgrade = headers.get(&SimpleHeader::UPGRADE)
        .and_then(|v| v.first())
        .ok_or(WebSocketError::MissingUpgradeHeader)?;
    if !upgrade.eq_ignore_ascii_case("websocket") {
        return Err(WebSocketError::InvalidUpgradeHeader);
    }

    // Check Connection header
    let connection = headers.get(&SimpleHeader::CONNECTION)
        .and_then(|v| v.first())
        .ok_or(WebSocketError::MissingConnectionHeader)?;
    if !connection.to_lowercase().contains("upgrade") {
        return Err(WebSocketError::InvalidConnectionHeader);
    }

    // Validate Sec-WebSocket-Accept
    let accept = headers.get(&SimpleHeader::SEC_WEBSOCKET_ACCEPT)
        .and_then(|v| v.first())
        .ok_or(WebSocketError::MissingAcceptKey)?;
    let expected_accept = compute_accept_key(key);
    if accept != &expected_accept {
        return Err(WebSocketError::InvalidAcceptKey);
    }

    Ok(())
}
```

### 8.6 WebSocket Connection API (Blocking)

```rust
impl WebSocketConnection {
    pub fn send(&mut self, message: WebSocketMessage) -> Result<(), WebSocketError> {
        let frame = match message {
            WebSocketMessage::Text(text) => WebSocketFrame {
                fin: true, rsv1: false, rsv2: false, rsv3: false,
                opcode: Opcode::Text,
                mask: self.generate_mask(),
                payload: text.into_bytes(),
            },
            WebSocketMessage::Binary(data) => WebSocketFrame {
                fin: true, rsv1: false, rsv2: false, rsv3: false,
                opcode: Opcode::Binary,
                mask: self.generate_mask(),
                payload: data,
            },
            WebSocketMessage::Ping(data) => WebSocketFrame {
                fin: true, rsv1: false, rsv2: false, rsv3: false,
                opcode: Opcode::Ping,
                mask: self.generate_mask(),
                payload: data,
            },
            WebSocketMessage::Pong(data) => WebSocketFrame {
                fin: true, rsv1: false, rsv2: false, rsv3: false,
                opcode: Opcode::Pong,
                mask: self.generate_mask(),
                payload: data,
            },
            WebSocketMessage::Close(code, reason) => {
                let mut payload = code.to_be_bytes().to_vec();
                payload.extend_from_slice(reason.as_bytes());
                WebSocketFrame {
                    fin: true, rsv1: false, rsv2: false, rsv3: false,
                    opcode: Opcode::Close,
                    mask: self.generate_mask(),
                    payload,
                }
            }
        };
        self.send_frame(frame)
    }

    pub fn recv(&mut self) -> Result<WebSocketMessage, WebSocketError> {
        loop {
            let frame = WebSocketFrame::decode(&mut self.stream)?;

            // Validate masking based on role
            match self.role {
                Role::Client if frame.mask.is_some() => {
                    return Err(WebSocketError::MaskedServerFrame);
                }
                Role::Server if frame.mask.is_none() => {
                    return Err(WebSocketError::UnmaskedClientFrame);
                }
                _ => {}
            }

            // Validate RSV bits (no extensions negotiated)
            if frame.rsv1 || frame.rsv2 || frame.rsv3 {
                return Err(WebSocketError::UnexpectedRsvBit);
            }

            // Handle control frames immediately (even between data fragments)
            if frame.opcode.is_control() {
                return self.handle_control_frame(frame);
            }

            // Assemble message from data frames
            if let Some(message) = self.assembler.push_frame(frame)? {
                return Ok(message);
            }
        }
    }

    fn handle_control_frame(&mut self, frame: WebSocketFrame) -> Result<WebSocketMessage, WebSocketError> {
        match frame.opcode {
            Opcode::Ping => {
                // Auto-respond with Pong (same payload)
                let pong_frame = WebSocketFrame {
                    fin: true, rsv1: false, rsv2: false, rsv3: false,
                    opcode: Opcode::Pong,
                    mask: self.generate_mask(),
                    payload: frame.payload.clone(),
                };
                self.send_frame(pong_frame)?;
                Ok(WebSocketMessage::Ping(frame.payload))
            }
            Opcode::Pong => Ok(WebSocketMessage::Pong(frame.payload)),
            Opcode::Close => {
                let (code, reason) = parse_close_payload(&frame.payload)?;

                // Send Close response if we haven't already
                if let ConnectionState::Open = self.state {
                    self.state = ConnectionState::Closing {
                        close_sent: false,
                        close_received: true,
                    };
                    self.close(code, &reason)?;
                }

                Ok(WebSocketMessage::Close(code, reason))
            }
            _ => Err(WebSocketError::InvalidOpcode(frame.opcode as u8)),
        }
    }

    pub fn close(&mut self, code: u16, reason: &str) -> Result<(), WebSocketError> {
        match self.state {
            ConnectionState::Closed => return Ok(()),
            ConnectionState::Closing { close_sent: true, .. } => return Ok(()),
            _ => {}
        }

        let mut payload = code.to_be_bytes().to_vec();
        payload.extend_from_slice(reason.as_bytes());

        let frame = WebSocketFrame {
            fin: true, rsv1: false, rsv2: false, rsv3: false,
            opcode: Opcode::Close,
            mask: self.generate_mask(),
            payload,
        };
        self.send_frame(frame)?;

        self.state = match self.state {
            ConnectionState::Open => ConnectionState::Closing {
                close_sent: true,
                close_received: false,
            },
            ConnectionState::Closing { close_received, .. } => ConnectionState::Closing {
                close_sent: true,
                close_received,
            },
            ConnectionState::Closed => ConnectionState::Closed,
        };

        Ok(())
    }

    fn send_frame(&mut self, frame: WebSocketFrame) -> Result<(), WebSocketError> {
        let encoded = frame.encode()?;
        self.stream.write_all(&encoded)?;
        self.stream.flush()?;
        Ok(())
    }

    fn generate_mask(&self) -> Option<[u8; 4]> {
        match self.role {
            Role::Client => Some(rand::random()),   // Clients MUST mask
            Role::Server => None,                     // Servers MUST NOT mask
        }
    }

    pub fn messages(&mut self) -> MessageIterator<'_> {
        MessageIterator { ws: self }
    }
}

pub struct MessageIterator<'a> {
    ws: &'a mut WebSocketConnection,
}

impl<'a> Iterator for MessageIterator<'a> {
    type Item = Result<WebSocketMessage, WebSocketError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let ConnectionState::Closed = self.ws.state {
            return None;
        }
        Some(self.ws.recv())
    }
}
```

### 8.7 WebSocket Task (TaskIterator)

The core TaskIterator-based state machine. Follows `send_request.rs` pattern exactly.

```rust
pub struct WebSocketTask(Option<WebSocketState>);

enum WebSocketState {
    Init(Option<Box<WebSocketConnectInfo>>),
    Connecting(Box<ConnectingData>),
    Handshake(Box<HandshakeState>),
    Open(WebSocketStream),
    Closed,
}

// Handshake sub-states for step-wise HTTP upgrade
enum HandshakeState {
    BuildingRequest(Box<WebSocketConnectInfo>),
    SendingRequest {
        connection: HttpClientConnection,
        request_bytes: Vec<Vec<u8>>,
        current_chunk: usize,
        ws_key: String,
    },
    ReadingResponse {
        connection: HttpClientConnection,
        reader: HttpResponseReader<SimpleHttpBody, RawStream>,
        ws_key: String,
    },
    ValidatingResponse {
        connection: HttpClientConnection,
        headers: SimpleHeaders,
        ws_key: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebSocketPending {
    Connecting,
    HandshakeSending,
    HandshakeReading,
    Reading,
}

impl TaskIterator for WebSocketTask {
    type Ready = Result<WebSocketMessage, WebSocketError>;
    type Pending = WebSocketPending;
    type Spawner = BoxedSendExecutionAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // Pattern: take state, process ONE step, restore state
        match self.0.take()? {
            WebSocketState::Init(mut info_opt) => {
                match info_opt.take() {
                    Some(info) => {
                        // Use HttpConnectionPool to create connection
                        // Transition to Connecting
                        // Return Pending(Connecting)
                    }
                    None => {
                        self.0 = Some(WebSocketState::Closed);
                        None
                    }
                }
            }

            WebSocketState::Connecting(data) => {
                // ONE step: check if connection established
                // On success → Handshake::BuildingRequest
                // On failure → return Pending then Closed (intermediate state pattern)
            }

            WebSocketState::Handshake(state) => {
                match *state {
                    HandshakeState::BuildingRequest(info) => {
                        // Generate WebSocket key
                        // Build upgrade request via SimpleIncomingRequestBuilder
                        // Render to bytes via Http11::request().http_render()
                        // Transition to SendingRequest
                        // Return Pending(HandshakeSending)
                    }

                    HandshakeState::SendingRequest {
                        mut connection, request_bytes, mut current_chunk, ws_key,
                    } => {
                        // Send ONE chunk per next() call
                        if current_chunk < request_bytes.len() {
                            // Write chunk, increment index
                            // Return Pending(HandshakeSending)
                        } else {
                            // All chunks sent, flush
                            // Create HttpResponseReader
                            // Transition to ReadingResponse
                            // Return Pending(HandshakeReading)
                        }
                    }

                    HandshakeState::ReadingResponse {
                        mut connection, mut reader, ws_key,
                    } => {
                        // Read ONE IncomingResponseParts per next() call
                        // On Intro: validate Status::SwitchingProtocols
                        // On Headers: transition to ValidatingResponse
                        // Return Pending(HandshakeReading)
                    }

                    HandshakeState::ValidatingResponse {
                        connection, headers, ws_key,
                    } => {
                        // Validate Sec-WebSocket-Accept
                        // On success: create WebSocketStream, transition to Open
                        // Return Ready(Ok(WebSocketMessage::ConnectionEstablished))
                        // On failure: transition to Closed
                        // Return Ready(Err(WebSocketError::...))
                    }
                }
            }

            WebSocketState::Open(mut stream) => {
                // ONE step: read ONE frame
                match stream.next_frame() {
                    Ok(frame) => {
                        self.0 = Some(WebSocketState::Open(stream));
                        Some(TaskStatus::Ready(Ok(frame.to_message()?)))
                    }
                    Err(WebSocketError::ConnectionClosed) => {
                        self.0 = Some(WebSocketState::Closed);
                        None
                    }
                    Err(e) => {
                        self.0 = Some(WebSocketState::Open(stream));
                        Some(TaskStatus::Ready(Err(e)))
                    }
                }
            }

            WebSocketState::Closed => None,
        }
    }
}
```

### 8.8 Consumer Wrapper (Executor Boundary)

Following the `SseStream` pattern from `wire/event_source/consumer.rs`:

```rust
use crate::valtron::{execute_stream, DrivenStreamIterator, Stream};

pub struct WebSocketClient {
    inner: DrivenStreamIterator<WebSocketTask>,
}

impl WebSocketClient {
    pub fn connect(url: impl Into<String>) -> Result<Self, WebSocketError> {
        let task = WebSocketTask::connect(url)?;
        let inner = execute_stream(task, None)
            .map_err(|e| WebSocketError::ConnectionFailed(format!("Executor error: {}", e)))?;
        Ok(Self { inner })
    }

    pub fn with_pool(
        url: impl Into<String>,
        pool: Arc<HttpConnectionPool<impl DnsResolver + Send + 'static>>,
    ) -> Result<Self, WebSocketError> {
        let task = WebSocketTask::connect_with_pool(url, pool)?;
        let inner = execute_stream(task, None)
            .map_err(|e| WebSocketError::ConnectionFailed(format!("Executor error: {}", e)))?;
        Ok(Self { inner })
    }

    pub fn send(&mut self, message: WebSocketMessage) -> Result<(), WebSocketError> {
        // Send via underlying stream (requires access to the stream inside the task)
        todo!()
    }

    pub fn messages(&mut self) -> WebSocketMessageIterator<'_> {
        WebSocketMessageIterator { client: self }
    }
}

pub struct WebSocketMessageIterator<'a> {
    client: &'a mut WebSocketClient,
}

// VALID: wrapping DrivenStreamIterator (already executor-driven)
// Caller controls the loop - Skip variant signals to call next() again
impl<'a> Iterator for WebSocketMessageIterator<'a> {
    type Item = Result<WebSocketEvent, WebSocketError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.client.inner.next()? {
            Stream::Next(result) => {
                match result {
                    Ok(WebSocketMessage::ConnectionEstablished) => Some(Ok(WebSocketEvent::Skip)),
                    other => Some(other.map(WebSocketEvent::Message)),
                }
            }
            Stream::Init | Stream::Ignore | Stream::Pending(_) | Stream::Delayed(_) => {
                Some(Ok(WebSocketEvent::Skip))
            }
        }
    }
}
```

### 8.9 URL Scheme Extension

Add to `simple_http/url/scheme.rs`:

```rust
pub enum Scheme {
    Http,
    Https,
    Ws,    // NEW
    Wss,   // NEW
    Custom(String),
}

impl Scheme {
    pub fn is_websocket(&self) -> bool {
        matches!(self, Self::Ws | Self::Wss)
    }

    pub fn is_secure_websocket(&self) -> bool {
        matches!(self, Self::Wss)
    }

    /// Returns true if the scheme requires TLS (https:// or wss://)
    pub fn requires_tls(&self) -> bool {
        matches!(self, Self::Https | Self::Wss)
    }

    pub fn default_port(&self) -> u16 {
        match self {
            Self::Http | Self::Ws => 80,
            Self::Https | Self::Wss => 443,
            Self::Custom(_) => 0,
        }
    }
}
```

### 8.10 Dependency Graph

```
WebSocketClient (consumer wrapper)
    └─> DrivenStreamIterator<WebSocketTask> (via execute_stream)
            └─> WebSocketTask (TaskIterator)
                    ├─> WebSocketHandshake
                    │       ├─> SimpleIncomingRequestBuilder  (for HTTP upgrade request)
                    │       ├─> Http11::request().http_render() (render request bytes)
                    │       └─> HttpResponseReader             (parse 101 response)
                    │
                    ├─> WebSocketFrame (encode/decode WebSocket frames)
                    │       └─> MessageAssembler (fragment reassembly)
                    │
                    └─> HttpClientConnection (established via HttpConnectionPool)
                            └─> SharedByteBufferStream<RawStream> (direct Read/Write for frames)
```

---

## 9. Implementation Phases

### 9.1 Tracing and Logging Requirements

All WebSocket code MUST use the `tracing` crate for structured logging. This is mandatory for debugging production issues and test failures.

**Tracing levels:**
- `trace!` - Frame-level details (bytes sent/received, opcode, payload length)
- `debug!` - State transitions, handshake steps
- `info!` - Connection lifecycle (connected, closed, errors)
- `warn!` - Protocol violations, recoverable errors
- `error!` - Unrecoverable errors, connection failures

**Instrumentation requirements:**

1. **All public methods** must be instrumented with `#[instrument]` macro:
```rust
use tracing::{debug, error, info, instrument, trace, warn};

impl WebSocketClient<R: DnsResolver + Send + 'static> {
    #[instrument(name = "websocket_connect", skip(resolver), fields(url = %url))]
    pub fn connect(resolver: R, url: impl Into<String>) -> Result<(Self, MessageDelivery), WebSocketError> {
        // Implementation
    }

    #[instrument(name = "websocket_send", skip(self), fields(message_type = %message.variant_name()))]
    pub fn send(&mut self, message: WebSocketMessage) -> Result<(), WebSocketError> {
        // Implementation
    }
}
```

2. **State machine transitions** must log state changes:
```rust
impl TaskIterator for WebSocketTask<R> {
    fn next(&mut self) -> Option<TaskStatus<...>> {
        let old_state = format!("{:?}", self.state);
        // ... state transition ...
        debug!(from = %old_state, to = format!("{:?}", self.state), "State transition");
    }
}
```

3. **Frame-level tracing** for debugging:
```rust
#[instrument(name = "websocket_frame_encode", skip(frame),
             fields(fin = frame.fin, opcode = %frame.opcode, payload_len = frame.payload.len()))]
pub fn encode(&self) -> Vec<u8> {
    trace!("Encoding WebSocket frame");
    // ... frame encoding ...
}
```

4. **Handshake tracing**:
```rust
#[instrument(name = "websocket_handshake", skip(conn, options),
             fields(host = %uri.host(), path = %uri.path()))]
pub fn handshake(...) -> Result<(), WebSocketError> {
    debug!("Sending WebSocket upgrade request");
    // ... handshake ...
    info!(accept_key = %accept_key, "Handshake completed");
}
```

5. **Error context** must include relevant fields:
```rust
Err(e) => {
    error!(error = ?e, state = ?self.state, "WebSocket error");
    self.state = Some(WebSocketState::Closed(Some(e)));
    None
}
```

**Test tracing requirements:**
- All tests must use `#[traced_test]` attribute
- Tests should assert on log output where appropriate
- Integration tests should enable `trace!` level for debugging

---

### 9.2 Test Server Enhancements (foundation_testing)

The WebSocket test server in `foundation_testing` MUST support the following for integration testing:

**Pre-prepared message delivery:**
```rust
/// WebSocket test server with pre-prepared messages.
///
/// WHEN: A client connects and handshake completes
/// THEN: Server immediately sends pre-configured messages
/// USE: Testing client receive logic without manual send
pub struct WebSocketServerWithMessages {
    addr: String,
    _handle: thread::JoinHandle<()>,
    running: Arc<AtomicBool>,
}

impl WebSocketServerWithMessages {
    /// Create server that sends predefined messages to each new connection.
    ///
    /// # Arguments
    ///
    /// * `messages` - Vec of messages to send (in order) after handshake
    ///
    /// # Example
    ///
    /// ```rust
    /// use foundation_testing::http::WebSocketServerWithMessages;
    /// use foundation_core::wire::websocket::WebSocketMessage;
    ///
    /// let messages = vec![
    ///     WebSocketMessage::Text("hello".to_string()),
    ///     WebSocketMessage::Binary(vec![1, 2, 3]),
    /// ];
    ///
    /// let server = WebSocketServerWithMessages::new(messages);
    /// let url = server.ws_url("/test");
    ///
    /// // Connect client - it will receive messages automatically
    /// let (mut client, _delivery) = WebSocketClient::connect(url)?;
    ///
    /// for event in client.messages() {
    ///     // Receives "hello", then binary [1,2,3]
    /// }
    /// ```
    pub fn new(messages: Vec<WebSocketMessage>) -> Self {
        // Implementation
    }

    /// Get WebSocket URL for a path on this test server.
    #[must_use]
    pub fn ws_url(&self, path: &str) -> String {
        format!("{}{}", self.addr, path)
    }
}
```

**Echo server (existing):**
- Echoes received messages back to client
- Responds to Ping with Pong
- Handles close handshake

**Custom handler server:**
```rust
/// Server with custom message handler callback.
pub struct WebSocketServerWithHandler<F>
where
    F: Fn(WebSocketMessage) -> Option<WebSocketMessage> + Send + 'static,
{
    // ...
    handler: Arc<F>,
}

impl<F> WebSocketServerWithHandler<F>
where
    F: Fn(WebSocketMessage) -> Option<WebSocketMessage> + Send + 'static,
{
    /// Create server with custom handler.
    ///
    /// Handler is called for each received message.
    /// If handler returns Some(msg), that message is sent back.
    /// If handler returns None, no response is sent.
    pub fn with_handler(handler: F) -> Self {
        // Implementation
    }
}
```

**Test server requirements:**
1. Must run on random available port (no port conflicts)
2. Must handle multiple concurrent connections
3. Must support graceful shutdown (Drop impl)
4. Must log all received frames via `tracing`
5. Pre-prepared messages sent AFTER handshake completes
6. Must support subprotocol negotiation for testing
7. Must echo close frames for proper close handshake testing

---

### Phase 1: Core WebSocket with TaskIterator

**Goal:** Working WebSocket client with TaskIterator as core API and consumer wrapper.

**File structure:**
```
backends/foundation_core/src/wire/websocket/
├── mod.rs              # Public API and re-exports
├── frame.rs            # Frame encoding/decoding + apply_mask
├── handshake.rs        # HTTP upgrade handshake (compute_accept_key, generate_key, validate)
├── task.rs             # WebSocketTask (TaskIterator — core API)
├── connection.rs       # WebSocketConnection (blocking API) + WebSocketClient (consumer wrapper)
├── message.rs          # WebSocketMessage enum + MessageAssembler
└── error.rs            # WebSocketError + FrameError
```

**Tasks (in TDD order):**

1. **Frame encoding/decoding** (`frame.rs`)
   - Implement `WebSocketFrame` struct with FIN, RSV1-3, opcode, mask, payload
   - Implement `Opcode` enum with `from_u8()`, `is_control()`, `is_data()`
   - Implement `apply_mask()` helper
   - Implement `encode()` — handle 3 payload length formats
   - Implement `decode(reader: &impl Read)` — with all validations
   - Unit tests: all opcodes, payload lengths (<126, 126-65535, ≥65536), masking/unmasking, control frame size validation, RSV bit validation

2. **Message types and assembly** (`message.rs`)
   - Define `WebSocketMessage` enum (Text, Binary, Ping, Pong, Close)
   - Implement `MessageAssembler` with `push_frame()` for fragment handling
   - Implement `parse_close_payload()` for Close frame body
   - Unit tests: single-frame messages, fragmented messages, control frame interleaving, UTF-8 validation, error cases

3. **Error handling** (`error.rs`)
   - Define `WebSocketError` enum with all variants
   - Define `FrameError` enum
   - `From<std::io::Error>`, `From<FromUtf8Error>` conversions
   - `Display` and `Error` implementations

4. **URL scheme extension**
   - Extend `simple_http/url/scheme.rs` for `ws://` and `wss://`
   - Add `is_websocket()`, `is_secure_websocket()`, `requires_tls()`
   - Map default ports (ws → 80, wss → 443)

5. **WebSocket handshake** (`handshake.rs`)
   - Implement `compute_accept_key()` using SHA-1 + base64
   - Implement `generate_websocket_key()` (16 random bytes, base64)
   - Implement `validate_upgrade_response()` (check Upgrade, Connection, Accept)
   - Build upgrade request via `SimpleIncomingRequestBuilder`
   - Render via `Http11::request().http_render()`
   - Parse response via `HttpResponseReader`
   - Unit tests: accept key computation (RFC test vector), header validation

6. **WebSocket Task** (`task.rs`) — Core TaskIterator
   - State machine: Init → Connecting → Handshake → Open → Closed
   - Handshake sub-states: BuildingRequest → SendingRequest → ReadingResponse → ValidatingResponse
   - Follow `send_request.rs` pattern exactly
   - NO loops in `next()` — ONE step per call
   - Use `HttpConnectionPool` for connection establishment

7. **WebSocket Connection + Client** (`connection.rs`)
   - `WebSocketConnection` — blocking send/recv/close/messages API
   - `WebSocketClient` — consumer wrapper via `execute_stream()`
   - `MessageIterator` — wraps `DrivenStreamIterator`, filters Stream variants

**Phase 1 Success Criteria:**

- [ ] `WebSocketTask` implements `TaskIterator` correctly
- [ ] State machine: Init → Connecting → Handshake → Open → Closed
- [ ] State carries ALL data (Option<Box<...>> pattern)
- [ ] NO LOOPS in `next()` — each call does ONE step
- [ ] `TaskStatus` variants used correctly (Ready, Pending, Delayed, Spawn)
- [ ] Consumer wrapper uses `unified::execute_stream()` (NOT raw TaskIterator)
- [ ] Frame encoding handles all payload lengths (<126, 126-65535, ≥65536)
- [ ] Frame decoding validates: control frame size, control frame FIN, RSV bits, payload length MSB
- [ ] Client masking correct (clients MUST mask, servers MUST NOT)
- [ ] Masking validation: server rejects unmasked client frames (1002), client rejects masked server frames
- [ ] `compute_accept_key()` passes RFC 6455 test vector
- [ ] Client handshake works with `ws://` (plain TCP)
- [ ] Client handshake works with `wss://` (TLS via existing infrastructure)
- [ ] Send/receive Text and Binary messages
- [ ] UTF-8 validation on Text messages
- [ ] Ping/Pong auto-response works (Pong echoes Ping payload)
- [ ] Close handshake works (bidirectional — send Close, receive Close response)
- [ ] Close frame payload correctly encoded/decoded (2-byte big-endian code + UTF-8 reason)
- [ ] Message fragmentation works (multi-frame messages with FIN flag)
- [ ] Control frames never fragmented
- [ ] Control frames handled between data fragments
- [ ] All tests pass, `cargo fmt`, `cargo clippy` clean
- [ ] All tests use `#[traced_test]` attribute
- [ ] Tracing instrumentation on all public methods

### Phase 2: Reconnection and Server Support

**Goal:** Auto-reconnection, subprotocol negotiation, server-side WebSocket.

**Additional files:**
```
backends/foundation_core/src/wire/websocket/
├── reconnecting_task.rs   # ReconnectingWebSocketTask (TaskIterator)
└── server.rs              # Server-side WebSocket upgrade
```

**Tasks:**

1. **ReconnectingWebSocketTask** (`reconnecting_task.rs`)
   - TaskIterator wrapping `WebSocketTask` with reconnection logic
   - Use `ExponentialBackoffDecider` for backoff with jitter
   - State: Init → Connecting → Open → Reconnecting → Exhausted
   - Auto-reconnect on connection loss
   - Max retries and max reconnect duration

2. **Subprotocol negotiation**
   - `Sec-WebSocket-Protocol` in handshake request
   - Validate server's chosen subprotocol is from client's list
   - Expose negotiated protocol in connection info

3. **Server-side WebSocket** (`server.rs`)
   - `WebSocketUpgrade::is_upgrade_request()` — check Upgrade/Connection headers
   - `WebSocketUpgrade::accept()` — compute accept key, send 101, return connection
   - Server Role: MUST NOT mask outgoing frames

4. **Performance optimizations**
   - Zero-copy frame parsing where possible
   - Buffer pooling for frames
   - Batch frame writing

**Phase 2 Success Criteria:**

- [ ] `ReconnectingWebSocketTask` implements `TaskIterator` correctly
- [ ] Auto-reconnects on connection loss with exponential backoff
- [ ] Max retries and max duration honored
- [ ] Subprotocol negotiation works correctly
- [ ] Server-side upgrade handling works
- [ ] Server does NOT mask outgoing frames
- [ ] Server rejects unmasked client frames with Close 1002
- [ ] Full integration with valtron executors

---

## 10. Testing Strategy

### Unit Tests

Tests live in `ewe_platform_tests` crate, NOT inline. All tests MUST use `#[traced_test]`.

```
tests/backends/foundation_core/units/websocket/
├── mod.rs                # Module registration
├── frame_tests.rs        # Frame encoding/decoding
├── message_tests.rs      # Message types and assembly
├── handshake_tests.rs    # Handshake key computation and validation
├── error_tests.rs        # Error Display tests
└── task_tests.rs         # WebSocketTask state machine tests
```

**Frame tests:**
- Encode/decode all opcodes (Text, Binary, Close, Ping, Pong, Continuation)
- Payload lengths: 0, 1, 125 (max control), 126, 127, 65535, 65536, large
- Masking/unmasking roundtrip
- Reject control frame > 125 bytes
- Reject fragmented control frame (FIN=0 + control opcode)
- Reject non-zero RSV bits without extension
- 64-bit payload length MSB must be 0

**Message assembly tests:**
- Single-frame Text message
- Single-frame Binary message
- Fragmented Text message (3 frames)
- Control frame interleaved between data fragments
- Reject unexpected Continuation frame
- Reject new data frame during fragmentation
- UTF-8 validation on reassembled Text message

**Handshake tests:**
- `compute_accept_key()` with RFC 6455 test vector
- `generate_websocket_key()` produces 24-char base64 string
- Validate correct response headers
- Reject missing Upgrade header
- Reject wrong Upgrade value
- Reject missing Connection header
- Reject missing Sec-WebSocket-Accept
- Reject incorrect Sec-WebSocket-Accept value
- Reject non-101 status code

**TaskIterator tests:**
- Init → Connecting transition
- Connection failure → Pending then Closed (intermediate state pattern)
- Handshake state progression
- Open state frame reading
- Close handshake via task

### Integration Tests

```
tests/backends/foundation_core/integration/websocket/
├── echo_tests.rs         # Echo server tests
└── tls_tests.rs          # TLS connection tests
```

1. Connect to WebSocket echo server, send text, verify echo
2. Send binary message, verify echo
3. Test ping/pong exchange
4. Test close handshake (client-initiated, server-initiated)
5. Test large message fragmentation
6. Connect via `wss://` — verify TLS works
7. Test subprotocol negotiation

---

## 11. Notes for Implementation Agents

### Critical Pre-Checks

Before starting, **MUST**:
1. Verify `connection` and `public-api` features are complete
2. Read `specifications/02-build-http-client/LEARNINGS.md` — especially valtron patterns
3. Read RFC 6455 (The WebSocket Protocol)
4. Study existing TaskIterator implementations:
   - `wire/simple_http/client/tasks/send_request.rs` — canonical TaskIterator pattern
   - `wire/event_source/task.rs` — recent TaskIterator implementation
   - `wire/event_source/consumer.rs` — consumer wrapper pattern with `execute_stream()`
   - `valtron/executors/unified.rs` — `execute_stream()` function
   - `valtron/task.rs` — `TaskIterator` trait, `TaskStatus` enum

### DO NOT Re-implement

- HTTP request building → use `SimpleIncomingRequestBuilder` + `Http11::request().http_render()`
- HTTP response parsing → use `HttpResponseReader`
- TLS handshake → use `HttpClientConnection` / `HttpConnectionPool`
- DNS resolution → use `DnsResolver`
- Executor boundary → use `unified::execute_stream()`
- Reconnection backoff → use `ExponentialBackoffDecider`

### Common Pitfalls

1. **Masking errors**: Forgetting to mask client frames or masking server frames. Both are protocol violations.
2. **Missing fragmentation support**: Assuming all messages fit in one frame. Must handle Continuation frames.
3. **Not responding to Ping**: Must auto-respond with Pong containing same payload.
4. **Not validating UTF-8**: Text frames MUST contain valid UTF-8. Invalid data → Close with code 1007.
5. **Fragmenting control frames**: Control frames MUST be single-frame (FIN=1, payload ≤ 125).
6. **Wrong byte order**: Extended payload length MUST be big-endian (network byte order).
7. **Wrong close handshake**: Both sides MUST send Close frame. After sending Close, MUST NOT send data frames.
8. **Re-implementing HTTP parsing**: Use existing `SimpleIncomingRequestBuilder` + `HttpResponseReader`.
9. **Using loops in TaskIterator::next()**: State machine must be step-wise, one transition per call.
10. **Wrapping TaskIterator directly**: Use `unified::execute_stream()` as executor boundary, not raw wrapping.
11. **Sending reserved close codes**: Codes 1004, 1005, 1006, 1015 MUST NOT be sent in Close frames.
12. **Ignoring RSV bits**: Non-zero RSV without negotiated extension MUST fail the connection.

---

## 12. References

### Primary Specifications

- [RFC 6455 — The WebSocket Protocol](https://datatracker.ietf.org/doc/html/rfc6455) — Core protocol specification
- [RFC 7692 — Compression Extensions for WebSocket](https://datatracker.ietf.org/doc/html/rfc7692) — Per-message deflate extension
- [IANA WebSocket Close Codes](https://www.iana.org/assignments/websocket/websocket.xml) — Registered close codes
- [IANA WebSocket Subprotocols](https://www.iana.org/assignments/websocket/websocket.xml#subprotocol) — Registered subprotocols

### RFC 6455 Section Reference

| Section | Topic | Relevance |
|---------|-------|-----------|
| 1.1 | Background | Context on HTTP upgrade mechanism |
| 1.2 | Terminology | Defines client, server, frame, message, endpoint |
| 1.3 | Opening Handshake | Test vectors for key exchange |
| 1.4 | Upgrading from HTTP | Connection establishment flow |
| 1.5 | Subprotocols | Application protocol negotiation |
| 1.6 | Compression | Extension framework overview |
| 1.7 | Using WebSocket | JavaScript API (for reference) |
| 2 | Conformance Requirements | MUST/SHOULD/MAY definitions |
| 3 | WebSocket URIs | `ws://` and `wss://` scheme definition |
| 4 | Opening Handshake | Client request and server response format |
| 4.1 | Client Requirements | Required client headers and validation |
| 4.2 | Server Requirements | Required server response and validation |
| 5 | Framing | Frame format, opcodes, masking |
| 5.1 | Overview | Frame structure diagram |
| 5.2 | Base Framing Protocol | FIN, RSV, opcode, payload length, mask |
| 5.3 | Client-to-Server Masking | XOR masking algorithm |
| 5.4 | Fragmentation | Message fragmentation and reassembly |
| 5.5 | Control Frames | Ping, Pong, Close frame rules |
| 5.6 | Data Frames | Text and Binary frame handling |
| 5.7 | Examples | Frame encoding examples |
| 6 | Data Integrity | UTF-8 validation, masking security |
| 7 | Closing | Close handshake and status codes |
| 7.1 | Close Definition | Close frame format |
| 7.2 | Status Codes | Registered close codes |
| 7.3 | Abnormal Closure | Connection without close frame |
| 7.4 | Status Code Ranges | Code ranges and registration |
| 8 | Error Handling | Error recovery and reporting |
| 9 | Extensibility | Extensions and subprotocols |
| 10 | Security | Origin, masking, DoS prevention |
| 11 | IANA | Registry considerations |

### Implementation Resources

- [WebSocket Frame Visualizer](https://www.websocket.org/echo.html) — Test frame encoding/decoding
- [MDN WebSocket API](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket) — JavaScript reference
- [canihazwebsocket.com](https://canihazwebsocket.com) — WebSocket test servers

### Test Vectors (RFC 6455 Section 1.3)

**Handshake:**
```
Client Request:
GET /chat HTTP/1.1
Host: server.example.com
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
Sec-WebSocket-Version: 13

Server Response:
HTTP/1.1 101 Switching Protocols
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=
```

**Masking Example (Section 5.3):**
```
Masking-key: 0x37 0xFA 0x21 0x3D
Unmasked payload: 0x48 0x65 0x6C 0x6C 0x6F ("Hello")
Masked payload:   0x7F 0x9F 0x4D 0x51 0x5B
```

**Frame Encoding Examples (Section 5.7):**

Single-frame text message "Hello" (unmasked, from server):
```
0x81 0x05 0x48 0x65 0x6C 0x6C 0x6F
│    │    └─────────────────────── payload ("Hello")
│    └──────────────────────────── payload length (5)
└───────────────────────────────── FIN=1, opcode=Text (0x1)
```

126-byte text message (unmasked):
```
0x81 0x7E 0x00 0x7E [126 bytes...]
│    │    └─────── 16-bit length (126, big-endian)
│    └──────────── extended length marker
└───────────────── FIN=1, opcode=Text
```

66KB text message (unmasked):
```
0x81 0x7F 0x00 0x00 0x00 0x00 0x00 0x01 0x02 0x20 [66560 bytes...]
│    │    └──────────────────────────── 64-bit length (big-endian)
│    └───────────────────────────────── extended length marker
└────────────────────────────────────── FIN=1, opcode=Text
```

Ping frame with "Hello" payload (unmasked):
```
0x89 0x05 0x48 0x65 0x6C 0x6C 0x6F
│    │    └─────────────────────── payload ("Hello")
│    └──────────────────────────── payload length (5)
└───────────────────────────────── FIN=1, opcode=Ping (0x9)
```

Text message "Hello" masked with key 0x37FA213D:
```
0x81 0x85 0x37 0xFA 0x21 0x3D 0x7F 0x9F 0x4D 0x51 0x5B
│    │    └───────────────┐    └────────────────────── masked payload
│    └───────────┐        │
│                └── mask bit set (0x80)              │
└───────────────────────────────────────────────────── FIN=1, opcode=Text
         masking-key (4 bytes)
```

---

## 13. Appendix: Complete Byte-Level Specification

### 13.1 Frame Header Byte Layout

```
Byte 0: [FIN:1][RSV1:1][RSV2:1][RSV3:1][Opcode:4]
  - Bit 7: FIN (1 = final fragment, 0 = more fragments coming)
  - Bit 6: RSV1 (must be 0 unless extension negotiated)
  - Bit 5: RSV2 (must be 0 unless extension negotiated)
  - Bit 4: RSV3 (must be 0 unless extension negotiated)
  - Bits 0-3: Opcode (frame type)

Byte 1: [MASK:1][Payload Length:7]
  - Bit 7: MASK (1 = masked payload, 0 = unmasked)
  - Bits 0-6: Payload length (0-125) or marker (126/127)

Bytes 2-N: Extended Payload Length (if applicable)
  - If length == 126: next 2 bytes = 16-bit unsigned length (big-endian)
  - If length == 127: next 8 bytes = 64-bit unsigned length (big-endian, MSB must be 0)

After extended length: Masking Key (if MASK=1)
  - Always 4 bytes

After masking key: Payload Data
  - Length determined by payload length field
  - XOR'd with masking key if MASK=1
```

### 13.2 Opcode Values

| Binary | Hex | Decimal | Name | Type |
|--------|-----|---------|------|------|
| 0000 | 0x0 | 0 | Continuation | Data |
| 0001 | 0x1 | 1 | Text | Data |
| 0010 | 0x2 | 2 | Binary | Data |
| 0011 | 0x3 | 3 | Reserved | Data |
| 0100 | 0x4 | 4 | Reserved | Data |
| 0101 | 0x5 | 5 | Reserved | Data |
| 0110 | 0x6 | 6 | Reserved | Data |
| 0111 | 0x7 | 7 | Reserved | Data |
| 1000 | 0x8 | 8 | Close | Control |
| 1001 | 0x9 | 9 | Ping | Control |
| 1010 | 0xA | 10 | Pong | Control |
| 1011 | 0xB | 11 | Reserved | Control |
| 1100 | 0xC | 12 | Reserved | Control |
| 1101 | 0xD | 13 | Reserved | Control |
| 1110 | 0xE | 14 | Reserved | Control |
| 1111 | 0xF | 15 | Reserved | Control |

### 13.3 Close Code Ranges

| Range | Meaning | Status |
|-------|---------|--------|
| 0-999 | Unused | Reserved |
| 1000-1999 | Standard codes | Defined by RFC 6455 and IANA |
| 1000 | Normal closure | Sendable |
| 1001 | Going away | Sendable |
| 1002 | Protocol error | Sendable |
| 1003 | Unsupported data | Sendable |
| 1004 | Reserved | MUST NOT be sent |
| 1005 | No status received | MUST NOT be sent (internal use only) |
| 1006 | Abnormal closure | MUST NOT be sent (internal use only) |
| 1007 | Invalid payload data | Sendable |
| 1008 | Policy violation | Sendable |
| 1009 | Message too big | Sendable |
| 1010 | Mandatory extension | Sendable |
| 1011 | Internal server error | Sendable |
| 1012-1014 | Reserved | IANA |
| 1015 | TLS handshake failure | MUST NOT be sent |
| 2000-2999 | Reserved | IANA |
| 3000-3999 | Library/Framework | Registered with IANA |
| 4000-4999 | Private use | Application-specific |
| 5000+ | Unused | Reserved |

### 13.4 Valid State Transitions

**Client State Machine:**
```
[New] → [Connecting] → [HandshakeSending] → [HandshakeReading] → [Open] → [Closing] → [Closed]
                                    ↓              ↓
                              [HandshakeFailed]  [Closed]
```

**Server State Machine:**
```
[New] → [UpgradeReceived] → [Open] → [Closing] → [Closed]
                                 ↓
                           [UpgradeFailed]
```

### 13.5 Complete Error Recovery Matrix

| Error Type | Action | Close Code | Send Close? | Wait for Response? |
|------------|--------|------------|-------------|-------------------|
| Invalid UTF-8 in Text | Close immediately | 1007 | Yes | No |
| Control frame > 125 bytes | Close immediately | 1009 | Yes | No |
| Fragmented control frame | Close immediately | 1002 | Yes | No |
| Reserved opcode | Close immediately | 1002 | Yes | No |
| Non-zero RSV (no extension) | Close immediately | 1002 | Yes | No |
| Unmasked client frame (server receives) | Close immediately | 1002 | Yes | No |
| Masked server frame (client receives) | Close immediately | 1002 | Yes | No |
| Invalid close payload (1 byte) | Close immediately | 1002 | Yes | No |
| Unexpected continuation | Close immediately | 1002 | Yes | No |
| Message too large | Close | 1009 | Yes | Optional |
| Policy violation | Close | 1008 | Yes | Optional |
| Normal shutdown | Close | 1000 | Yes | Yes |
| Going away (server shutdown) | Close | 1001 | Yes | No |
| TCP connection lost | Cleanup | 1006 (internal) | No | N/A |
| TLS handshake failure | Cleanup | 1015 (internal) | No | N/A |

### 13.6 Threading Model

```
Thread 1 (Main/Event Loop)
    └─> DrivenStreamIterator (executor-managed)
            └─> WebSocketTask (state machine)

Thread 2 (I/O Worker - optional, executor-managed)
    └─> TcpStream read/write operations

Shared State:
    └─> SharedByteBufferStream<RawStream> (Arc<Mutex<BufferedStream<T>>>)
            └─> Thread-safe via Mutex
```

**Key Points:**
- `SharedByteBufferStream` is `Clone` — clones share the same underlying `Arc<Mutex<...>>`
- Reader and writer can operate concurrently on cloned handles
- `TaskIterator` itself is NOT `Send` by default — executor manages thread boundaries
- Consumer wrappers (`WebSocketClient`) are typically single-threaded
- For multi-threaded access, use `Arc<Mutex<WebSocketClient>>`

---

*Created: 2026-02-28*
*Last Updated: 2026-03-10 (Comprehensive update with full protocol details, byte-level specs, and test vectors)*

---

## Verification Commands

```bash
# Format check
cargo fmt -- --check

# Clippy linting
cargo clippy --package foundation_core --features ssl-rustls -- -D warnings

# Unit tests
cargo test --package ewe_platform_tests -- websocket

# Build without TLS
cargo build --package foundation_core

# Build with TLS (all backends)
cargo build --package foundation_core --features ssl-rustls
cargo build --package foundation_core --features ssl-openssl
cargo build --package foundation_core --features ssl-native-tls

# Integration tests (requires network)
cargo test --package ewe_platform_tests --features ssl-rustls -- websocket::integration --ignored
```

---

*Created: 2026-02-28*
*Last Updated: 2026-03-10 (Comprehensive update with full protocol details)*
