# WebSocket Feature Architecture

## Document Overview

This document provides a comprehensive architectural analysis for implementing WebSocket support in the `foundation_core` wire module. It is based on thorough exploration of the existing codebase infrastructure, particularly the `simple_http` module and `valtron` task execution framework.

**Status**: Draft
**Last Updated**: 2026-03-03
**Version**: 1.0

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Existing Infrastructure Analysis](#existing-infrastructure-analysis)
3. [WebSocket Protocol Overview](#websocket-protocol-overview)
4. [Integration Strategy](#integration-strategy)
5. [State Machine Design](#state-machine-design)
6. [Frame Handling Approach](#frame-handling-approach)
7. [Implementation Phases](#implementation-phases)
8. [Technical Design Details](#technical-design-details)

---

## 1. Executive Summary

### Key Findings

The `foundation_core` wire module provides **excellent infrastructure** for WebSocket implementation:

1. **WebSocket headers already defined** in `SimpleHeader` enum:
   - `SEC_WEBSOCKET_ACCEPT`
   - `SEC_WEBSOCKET_EXTENSIONS`
   - `SEC_WEBSOCKET_KEY`
   - `SEC_WEBSOCKET_PROTOCOL`
   - `SEC_WEBSOCKET_VERSION`
   - `UPGRADE`
   - `CONNECTION`

2. **Robust connection infrastructure**:
   - `HttpClientConnection` wraps `SharedByteBufferStream<RawStream>`
   - Supports both TCP and TLS (via `ssl-rustls`, `ssl-openssl`, `ssl-native-tls`)
   - Connection pooling via `HttpConnectionPool`
   - DNS resolution abstraction via `DnsResolver` trait

3. **TaskIterator pattern** for non-blocking operations:
   - State machine-based execution
   - Composable async-like behavior without async/await
   - Integration with `valtron` executor system
   - Supports spawning sub-tasks

4. **Stream abstractions**:
   - `RawStream` (low-level TCP/TLS)
   - `SharedByteBufferStream` (buffered Read/Write wrapper)
   - `BufferedReader`/`BufferedWriter` for I/O buffering

### Recommended Approach: TaskIterator with DrivenSendTaskIterator Wrappers

**CRITICAL ARCHITECTURAL PRINCIPLE:** All WebSocket client implementations MUST use TaskIterator as the core pattern with `DrivenSendTaskIterator` for blocking wrappers. The TaskIterator is a pure state machine with NO loops - execution driving happens in the wrapper.

**Design Hierarchy:**
1. **TaskIterator (Core)** - Pure state machine, ONE step per `next()` call, NO loops
2. **Plain Iterator (Internal)** - Used internally for frame parsing, wrapped by TaskIterator
3. **DrivenSendTaskIterator (Wrapper)** - Handles execution driving via `run_until_next_state()`
4. **Blocking Iterator API (Convenience)** - Wraps DrivenSendTaskIterator, extracts Ready values

**Phase 1: Core WebSocket with TaskIterator (1-2 weeks)**
- Implement `WebSocketTask` following valtron TaskIterator pattern
- State machine: Init → Connecting → Handshake → Open → Closed
- Frame parser is internal (plain iterator), wrapped by TaskIterator
- Blocking wrapper uses `DrivenSendTaskIterator` - execution driving in wrapper, not TaskIterator

**Phase 2: Advanced Features (1 week)**
- `ReconnectingWebSocketTask` with auto-reconnection
- Subprotocol negotiation
- Server-side WebSocket support
- Performance optimizations

---

## 2. Existing Infrastructure Analysis

### 2.1 Wire Module Structure

```
wire/
├── mod.rs              # Module exports (simple_http, event_source, http_stream)
├── simple_http/        # HTTP/1.1 implementation
│   ├── mod.rs
│   ├── errors.rs       # Error types (HttpClientError, HttpReaderError)
│   ├── impls.rs        # Core types (SimpleHeader, SimpleMethod, Status, Body, etc.)
│   ├── url/            # URL parsing (Uri, Scheme, Authority, etc.)
│   └── client/         # HTTP client implementation
│       ├── connection.rs    # HttpClientConnection, TLS upgrade
│       ├── pool.rs          # Connection pooling
│       ├── dns.rs           # DNS resolution
│       ├── request.rs       # Request building
│       ├── tasks/           # TaskIterator-based request execution
│       │   ├── state.rs     # State types and enums
│       │   ├── send_request.rs        # Main request task
│       │   ├── request_intro.rs       # Read response intro
│       │   ├── request_redirect.rs    # Handle redirects
│       │   └── request_stream.rs      # Stream handling
│       ├── proxy.rs         # Proxy support (HTTP, SOCKS5)
│       ├── middleware.rs    # Request/response middleware
│       ├── cookie.rs        # Cookie handling
│       ├── redirects.rs     # Redirect logic
│       ├── compression.rs   # Response decompression
│       └── tls_task.rs      # TLS handshake tasks
├── event_source/       # Server-Sent Events (SSE)
└── http_stream/        # HTTP stream utilities (ReconnectingStream)
```

### 2.2 Core Infrastructure Components

#### 2.2.1 Connection Stack

```
HttpClientConnection
    └─> SharedByteBufferStream<RawStream>
            └─> RawStream (enum)
                    ├─> TCP (std::net::TcpStream)
                    └─> TLS (ClientSSLStream)
```

**Key characteristics**:
- `SharedByteBufferStream` provides `Arc<Mutex<BufferedStream<T>>>` for cloneable, thread-safe access
- Implements `Read + Write + BufRead`
- Supports setting read/write timeouts via `ReadTimeoutOperations` trait
- Can be used directly for WebSocket frame I/O after handshake

#### 2.2.2 HTTP Headers Support

From `simple_http/impls.rs` (lines 629-640):

```rust
pub enum SimpleHeader {
    // ... other headers
    SEC_WEBSOCKET_ACCEPT,
    SEC_WEBSOCKET_EXTENSIONS,
    SEC_WEBSOCKET_KEY,
    SEC_WEBSOCKET_PROTOCOL,
    SEC_WEBSOCKET_VERSION,
    UPGRADE,
    // ...
}
```

**Implications**:
- No need to add new header types
- Can use existing `SimpleHeaders` (BTreeMap<SimpleHeader, Vec<String>>)
- Already parsed and normalized by HTTP client

#### 2.2.3 HTTP Status Codes

From `simple_http/impls.rs` (lines 936-938):

```rust
pub enum Status {
    SwitchingProtocols = 101,  // Required for WebSocket handshake
    UpgradeRequired = 426,
    // ...
}
```

### 2.3 TaskIterator Pattern

The valtron `TaskIterator` trait (from `valtron/task.rs`):

```rust
pub trait TaskIterator {
    type Pending;   // State during async operations
    type Ready;     // Completed result type
    type Spawner: ExecutionAction;  // For spawning sub-tasks

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>;
}
```

**TaskStatus variants**:
- `Init` - Initial state
- `Pending(P)` - Waiting (with progress indicator)
- `Ready(R)` - Result available
- `Spawn(S)` - Request to spawn sub-task
- `Delayed(Duration)` - Schedule retry after delay

**Example usage in HTTP client** (`client/tasks/send_request.rs`):

```rust
pub struct SendRequestTask<R> {
    state: Option<SendRequestState<R>>,
}

pub enum SendRequestState<R> {
    Init(Option<Box<SendRequest<R>>>),
    Connecting(DrivenRecvIterator<GetHttpRequestRedirectTask<R>>),
    Reading(DrivenRecvIterator<GetRequestIntroTask>),
    SkipReading(Box<Option<RequestIntro>>),
    Done,
}

impl<R: DnsResolver + Send + 'static> TaskIterator for SendRequestTask<R> {
    type Ready = RequestIntro;
    type Pending = HttpRequestPending;
    type Spawner = BoxedSendExecutionAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // State machine transitions...
    }
}
```

**Key patterns observed**:
1. **State enum** holds all possible states
2. **State transitions** in `next()` method
3. **Spawning sub-tasks** via `TaskStatus::Spawn`
4. **Receiving sub-task results** via `DrivenRecvIterator`
5. **Non-blocking** - each `next()` call does minimal work
6. **Option wrapper** - `struct Task(Option<State>)` for termination
7. **Data carrying** - State variants hold `Option<Box<...>>`

**Blocking Wrapper Pattern (DrivenSendTaskIterator)**:

For blocking Iterator API, use `DrivenSendTaskIterator` which handles execution driving externally:

```rust
use crate::valtron::{drive_iterator, DrivenSendTaskIterator};

// Blocking wrapper using DrivenSendTaskIterator
pub struct WebSocketConnection {
    driven: DrivenSendTaskIterator<WebSocketTask>,
}

impl WebSocketConnection {
    pub fn connect(url: &str) -> Result<Self, WebSocketError> {
        let task = WebSocketTask::connect(url)?;
        Ok(Self {
            driven: drive_iterator(task),
        })
    }
}

impl Iterator for WebSocketConnection {
    type Item = Result<WebSocketMessage, WebSocketError>;

    fn next(&mut self) -> Option<Self::Item> {
        // DrivenSendTaskIterator handles execution driving
        // Calls run_until_next_state() then task.next() ONCE
        // NO LOOPS in TaskIterator - execution driving is external
        match self.driven.next() {
            Some(TaskStatus::Ready(result)) => Some(result),
            Some(TaskStatus::Delayed(_)) => None,  // Reconnection backoff
            Some(TaskStatus::Pending(_)) => self.next(),  // Continue driving (tail recursion)
            Some(TaskStatus::Spawn(_)) => self.next(),  // Spawn handled by executor
            Some(TaskStatus::Init) => self.next(),
            None => None,
        }
    }
}
```

**DrivenSendTaskIterator implementation** (from `valtron/executors/task_iters.rs`):

```rust
pub struct DrivenSendTaskIterator<T>(Option<T>)
where
    T: TaskIterator + Send + 'static;

impl<T> Iterator for DrivenSendTaskIterator<T> {
    type Item = TaskStatus<T::Ready, T::Pending, T::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(mut task_iterator) = self.0.take() {
            // Drive execution externally - calls run_until_next_state()
            run_until_next_state();

            // Get ONE result from TaskIterator - NO LOOPS
            let next_value = task_iterator.next();

            // Restore task for next call
            if next_value.is_some() {
                self.0.replace(task_iterator);
            }
            next_value
        } else {
            None
        }
    }
}
```

**Key Points**:
- TaskIterator remains pure - NO loops in `next()`
- DrivenSendTaskIterator handles execution driving via `run_until_next_state()`
- Blocking Iterator wrapper extracts Ready values from TaskStatus
- Tail recursion for Pending/Spawn/Init - not a loop in TaskIterator

### 2.4 HTTP Request/Response Flow

#### 2.4.1 Request Building

```rust
PreparedRequest {
    method: SimpleMethod,
    url: ParsedUrl,
    headers: SimpleHeaders,
    body: SendSafeBody,
    extensions: Extensions,
}
```

#### 2.4.2 Response Reading

Responses are read as iterator over `IncomingResponseParts`:

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

**Critical insight for WebSocket**:
After receiving `Status::SwitchingProtocols` (101), we can take ownership of the underlying `RawStream` and use it for WebSocket frame I/O.

### 2.5 TLS Support

From `client/connection.rs` (lines 143-171):

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

**Key takeaway**: TLS handshake happens **before** HTTP/WebSocket handshake. For `wss://` URLs:
1. Connect TCP
2. Upgrade to TLS
3. Perform HTTP upgrade handshake
4. Start WebSocket framing

---

## 3. WebSocket Protocol Overview

### 3.1 RFC 6455 Summary

WebSocket provides full-duplex communication over a single TCP connection:

1. **Opening Handshake** (HTTP/1.1 Upgrade)
2. **Data Transfer** (Frame-based protocol)
3. **Closing Handshake** (Close frame exchange)

### 3.2 Opening Handshake

#### Client Request

```http
GET /chat HTTP/1.1
Host: server.example.com
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
Sec-WebSocket-Version: 13
Sec-WebSocket-Protocol: chat, superchat  (optional)
```

#### Server Response

```http
HTTP/1.1 101 Switching Protocols
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=
Sec-WebSocket-Protocol: chat  (optional)
```

**`Sec-WebSocket-Accept` computation**:
```rust
use sha1::{Digest, Sha1};
use base64::Engine;

fn compute_accept_key(client_key: &str) -> String {
    const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let combined = format!("{}{}", client_key, GUID);
    let hash = Sha1::digest(combined.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(hash)
}
```

### 3.3 WebSocket Frame Structure

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

**Fields**:
- **FIN** (1 bit): Final fragment flag
- **RSV1-3** (3 bits): Reserved (must be 0 unless extension negotiated)
- **Opcode** (4 bits): Frame type
- **MASK** (1 bit): Payload is masked (clients MUST mask, servers MUST NOT)
- **Payload len** (7 bits): Length (or 126/127 for extended length)
- **Masking-key** (4 bytes): XOR mask (only if MASK=1)
- **Payload Data**: Application data

### 3.4 Opcodes

| Opcode | Name         | Description                          |
|--------|--------------|--------------------------------------|
| 0x0    | Continuation | Continuation frame                   |
| 0x1    | Text         | UTF-8 text message                   |
| 0x2    | Binary       | Binary data message                  |
| 0x3-0x7| Reserved     | Reserved for future use              |
| 0x8    | Close        | Connection close                     |
| 0x9    | Ping         | Ping control frame                   |
| 0xA    | Pong         | Pong control frame (ping response)   |
| 0xB-0xF| Reserved     | Reserved for future control frames   |

### 3.5 Masking

**Why masking?** To prevent cache poisoning attacks.

**Algorithm**:
```rust
fn apply_mask(data: &mut [u8], mask: [u8; 4]) {
    for (i, byte) in data.iter_mut().enumerate() {
        *byte ^= mask[i % 4];
    }
}
```

**Rules**:
- Client-to-server frames **MUST** be masked
- Server-to-client frames **MUST NOT** be masked
- Mask is randomly generated for each frame

### 3.6 Message Fragmentation

Large messages can be split across multiple frames:

```
Message = Frame₁(FIN=0, op=Text) + Frame₂(FIN=0, op=Continuation) + Frame₃(FIN=1, op=Continuation)
```

**Rules**:
- First frame has opcode (Text/Binary)
- Subsequent frames have opcode=Continuation
- Last frame has FIN=1
- Control frames (Ping/Pong/Close) MUST NOT be fragmented

### 3.7 Control Frames

#### Ping/Pong
- Ping can contain application data (≤125 bytes)
- Receiver MUST respond with Pong containing same data
- Used for keep-alive and RTT measurement

#### Close
- Signals connection close
- Optionally includes close code (2 bytes) and reason (UTF-8 text)
- Both sides should send Close frame for graceful shutdown

**Close codes**:
- 1000: Normal closure
- 1001: Going away (e.g., server shutdown)
- 1002: Protocol error
- 1003: Unsupported data (e.g., binary when expecting text)
- 1006: Abnormal closure (no close frame received)
- 1007: Invalid UTF-8
- 1008: Policy violation
- 1009: Message too big
- 1010: Extension negotiation failed
- 1011: Unexpected condition

---

## 4. Integration Strategy

### 4.1 Module Location

```
backends/foundation_core/src/wire/
└── websocket/                  # NEW module
    ├── mod.rs                  # Public API and re-exports
    ├── frame.rs                # Frame encoding/decoding
    ├── handshake.rs            # HTTP upgrade handshake
    ├── connection.rs           # WebSocketConnection
    ├── message.rs              # WebSocketMessage enum
    ├── error.rs                # WebSocketError
    ├── task.rs                 # TaskIterator implementation (Phase 2)
    └── server.rs               # Server-side support (Phase 3)
```

### 4.2 Dependency Graph

```
websocket::WebSocketConnection
    ├─> websocket::WebSocketHandshake
    │       ├─> simple_http::SimpleIncomingRequestBuilder  (for HTTP upgrade)
    │       └─> simple_http::HttpResponseReader            (for reading 101 response)
    │
    ├─> websocket::Frame                                   (encode/decode frames)
    │
    └─> connection::HttpClientConnection
            └─> SharedByteBufferStream<RawStream>         (direct Read/Write)
```

### 4.3 Reusable Components

| Component | Source | Usage in WebSocket |
|-----------|--------|-------------------|
| `SimpleHeader::*` | `simple_http/impls.rs` | WebSocket handshake headers |
| `Status::SwitchingProtocols` | `simple_http/impls.rs` | Handshake validation |
| `HttpClientConnection` | `simple_http/client/connection.rs` | TCP/TLS connection |
| `SharedByteBufferStream` | `io/ioutils/mod.rs` | Frame I/O |
| `ParsedUrl` (Uri) | `simple_http/url/mod.rs` | ws:// and wss:// parsing |
| `DnsResolver` | `simple_http/client/dns.rs` | Hostname resolution |
| `HttpConnectionPool` | `simple_http/client/pool.rs` | Connection reuse (optional) |
| `SimpleIncomingRequestBuilder` | `simple_http/impls.rs` | Build upgrade request |
| `TaskIterator` | `valtron/task.rs` | Non-blocking operations |

### 4.4 URL Scheme Handling

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

    pub fn default_port(&self) -> u16 {
        match self {
            Self::Http | Self::Ws => 80,
            Self::Https | Self::Wss => 443,
            Self::Custom(_) => 0,
        }
    }
}
```

---

## 5. State Machine Design

### 5.1 WebSocket Connection States

```
┌─────────┐
│  Init   │  Initial state, ready to connect
└────┬────┘
     │
     ├─ DNS Resolution
     ├─ TCP Connect
     ├─ TLS Handshake (for wss://)
     ├─ HTTP Upgrade Request
     │
     v
┌──────────┐
│Connecting│  Establishing connection
└────┬─────┘
     │
     ├─ Read 101 Response
     ├─ Validate Sec-WebSocket-Accept
     │
     v
┌──────────┐
│   Open   │  Connection established, ready for frames
└────┬─────┘
     │
     ├─ Send/Receive Data frames
     ├─ Send/Receive Ping/Pong frames
     │
     ├─ Receive Close frame → Send Close response
     ├─ Send Close frame → Wait for Close response
     │
     v
┌──────────┐
│ Closing  │  Close handshake in progress
└────┬─────┘
     │
     v
┌──────────┐
│  Closed  │  Connection closed
└──────────┘
```

### 5.2 Phase 1: Blocking Implementation

```rust
pub struct WebSocketConnection {
    stream: SharedByteBufferStream<RawStream>,
    role: Role,
    state: ConnectionState,
    fragmented_message: Option<FragmentedMessage>,
}

pub enum Role {
    Client,  // Must mask outgoing frames
    Server,  // Must not mask outgoing frames
}

enum ConnectionState {
    Open,
    Closing { close_sent: bool, close_received: bool },
    Closed,
}
```

**Blocking API**:
```rust
impl WebSocketConnection {
    // Connect to WebSocket server (blocking)
    pub fn connect(url: &str) -> Result<Self, WebSocketError>;

    // Send message (blocking write)
    pub fn send(&mut self, msg: WebSocketMessage) -> Result<(), WebSocketError>;

    // Receive next message (blocking read)
    pub fn recv(&mut self) -> Result<WebSocketMessage, WebSocketError>;

    // Close connection gracefully (blocking)
    pub fn close(&mut self, code: u16, reason: &str) -> Result<(), WebSocketError>;
}
```

### 5.3 Phase 2: TaskIterator Implementation

```rust
pub struct WebSocketTask {
    state: Option<WebSocketTaskState>,
}

enum WebSocketTaskState {
    Init(WebSocketConnectInfo),
    DnsResolving(DrivenRecvIterator<DnsTask>),
    Connecting(DrivenRecvIterator<TcpConnectTask>),
    TlsHandshake(DrivenRecvIterator<TlsHandshakeTask>),
    UpgradeRequest(DrivenRecvIterator<HttpUpgradeTask>),
    Open(WebSocketConnection),
    Closed,
}

impl TaskIterator for WebSocketTask {
    type Ready = WebSocketMessage;
    type Pending = WebSocketPending;
    type Spawner = BoxedSendExecutionAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // Non-blocking state machine
    }
}

pub enum WebSocketPending {
    Connecting,
    WaitingForFrame,
    Closing,
}
```

---

## 6. Frame Handling Approach

### 6.1 Frame Type Definitions

```rust
pub struct Frame {
    pub fin: bool,
    pub rsv1: bool,
    pub rsv2: bool,
    pub rsv3: bool,
    pub opcode: Opcode,
    pub mask: Option<[u8; 4]>,
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
}
```

### 6.2 Frame Encoding

```rust
impl Frame {
    pub fn encode(&self) -> Result<Vec<u8>, FrameError> {
        let mut buf = Vec::with_capacity(self.payload.len() + 14);

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

        Ok(buf)
    }
}
```

### 6.3 Frame Decoding

```rust
impl Frame {
    pub fn decode<R: Read>(reader: &mut R) -> Result<Self, FrameError> {
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
        }

        // Validate payload length for control frames
        if opcode.is_control() && payload_len > 125 {
            return Err(FrameError::ControlFrameTooLarge);
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

        Ok(Frame {
            fin,
            rsv1,
            rsv2,
            rsv3,
            opcode,
            mask,
            payload,
        })
    }
}
```

### 6.4 Message Assembly

```rust
pub struct MessageAssembler {
    fragments: Vec<Fragment>,
    opcode: Option<Opcode>,
}

struct Fragment {
    data: Vec<u8>,
}

impl MessageAssembler {
    pub fn push_frame(&mut self, frame: Frame) -> Result<Option<WebSocketMessage>, FrameError> {
        // Control frames are never fragmented
        if frame.opcode.is_control() {
            return Ok(Some(WebSocketMessage::from_control_frame(frame)?));
        }

        // First frame
        if self.opcode.is_none() {
            if frame.opcode == Opcode::Continuation {
                return Err(FrameError::UnexpectedContinuation);
            }
            self.opcode = Some(frame.opcode);
        } else {
            // Continuation frame
            if frame.opcode != Opcode::Continuation {
                return Err(FrameError::ExpectedContinuation);
            }
        }

        self.fragments.push(Fragment { data: frame.payload });

        // Final frame?
        if frame.fin {
            let opcode = self.opcode.take().unwrap();
            let data: Vec<u8> = self.fragments.drain(..).flat_map(|f| f.data).collect();

            let message = match opcode {
                Opcode::Text => {
                    let text = String::from_utf8(data)
                        .map_err(|_| FrameError::InvalidUtf8)?;
                    WebSocketMessage::Text(text)
                }
                Opcode::Binary => WebSocketMessage::Binary(data),
                _ => return Err(FrameError::InvalidOpcode),
            };

            Ok(Some(message))
        } else {
            Ok(None)  // More fragments needed
        }
    }
}
```

---

## 7. Implementation Phases

### Phase 1: Core WebSocket (Blocking)

**Duration**: 1-2 weeks
**Goal**: Working WebSocket client with blocking I/O

**Tasks**:
1. **Frame encoding/decoding** (`frame.rs`)
   - Implement `Frame` struct
   - Implement `encode()` method
   - Implement `decode()` method
   - Add masking/unmasking helper
   - Unit tests for frame parsing

2. **WebSocket handshake** (`handshake.rs`)
   - Implement `compute_accept_key()` using SHA-1
   - Build HTTP upgrade request using `SimpleIncomingRequestBuilder`
   - Parse 101 response and validate headers
   - Return `HttpClientConnection` on success

3. **WebSocket connection** (`connection.rs`)
   - Implement `WebSocketConnection` struct
   - Implement `send()` method (encode + write frame)
   - Implement `recv()` method (read + decode frame)
   - Implement `close()` method
   - Handle Ping/Pong automatically
   - Message fragmentation support

4. **Message types** (`message.rs`)
   - Define `WebSocketMessage` enum
   - Conversion from/to frames

5. **Error handling** (`error.rs`)
   - Define `WebSocketError` enum
   - Conversions from I/O errors

6. **Public API** (`mod.rs`)
   - Simple builder API
   - Documentation and examples

**Success Criteria**:
- Can connect to `ws://` servers
- Can connect to `wss://` servers (with TLS)
- Can send Text/Binary messages
- Can receive Text/Binary messages
- Ping/Pong auto-response works
- Close handshake works
- All tests pass

### Phase 2: TaskIterator Integration (Non-Blocking)

**Duration**: 1-2 weeks
**Goal**: Non-blocking WebSocket operations

**Tasks**:
1. **WebSocket task** (`task.rs`)
   - Implement `WebSocketTask` state machine
   - Implement `TaskIterator` trait
   - Non-blocking connect
   - Non-blocking send/receive

2. **Frame reader task**
   - Implement `FrameReaderTask`
   - Non-blocking frame reading
   - Integration with message assembler

3. **Integration with HTTP client**
   - Reuse DNS resolution
   - Reuse connection pooling
   - Reuse TLS handshake

**Success Criteria**:
- Non-blocking connect works
- Can send/receive without blocking
- Integrates with valtron executor
- Performance is acceptable

### Phase 3: Advanced Features

**Duration**: 1-2 weeks
**Goal**: Production-ready WebSocket support

**Tasks**:
1. **Subprotocol negotiation**
   - Support `Sec-WebSocket-Protocol` header
   - Validate negotiated subprotocol

2. **Extensions support**
   - Support `Sec-WebSocket-Extensions` header
   - Implement per-message deflate (optional)

3. **Server-side WebSocket** (`server.rs`)
   - Accept upgrade requests
   - Server-side connection handling
   - Integration with HTTP server (future)

4. **Performance optimizations**
   - Zero-copy frame parsing where possible
   - Buffer pooling
   - Batch frame writing

**Success Criteria**:
- Subprotocol negotiation works
- Server-side support works
- Performance benchmarks pass

---

## 8. Technical Design Details

### 8.1 WebSocket Handshake Implementation

```rust
pub struct WebSocketHandshake;

impl WebSocketHandshake {
    pub fn connect(url: &str) -> Result<WebSocketConnection, WebSocketError> {
        let uri = Uri::parse(url)?;

        // Validate scheme
        if !uri.scheme().is_websocket() {
            return Err(WebSocketError::InvalidScheme);
        }

        // Generate random key
        let key = generate_websocket_key();

        // Build HTTP upgrade request using SimpleIncomingRequestBuilder
        let request = SimpleIncomingRequestBuilder::get(uri.path_and_query())
            .header(SimpleHeader::HOST, &uri.host_with_port())
            .header(SimpleHeader::UPGRADE, "websocket")
            .header(SimpleHeader::CONNECTION, "Upgrade")
            .header(SimpleHeader::SEC_WEBSOCKET_KEY, &key)
            .header(SimpleHeader::SEC_WEBSOCKET_VERSION, "13")
            .build()?;

        // Connect and send request
        let mut conn = HttpClientConnection::connect(&uri, &SystemDnsResolver, None)?;
        let request_bytes = Http11::request(&request).http_render()?;

        for chunk in request_bytes {
            conn.write_all(&chunk?)?;
        }
        conn.flush()?;

        // Read response
        let reader = HttpResponseReader::new(conn.clone_stream(), SimpleHttpBody);
        let mut intro = None;
        let mut headers = None;

        for part in reader {
            match part? {
                IncomingResponseParts::Intro(status, proto, text) => {
                    if status != Status::SwitchingProtocols {
                        return Err(WebSocketError::UpgradeFailed(status));
                    }
                    intro = Some((status, proto, text));
                }
                IncomingResponseParts::Headers(h) => {
                    headers = Some(h);
                    break;
                }
                _ => {}
            }
        }

        let headers = headers.ok_or(WebSocketError::MissingHeaders)?;

        // Validate headers
        validate_upgrade_response(&headers, &key)?;

        // Create WebSocket connection
        Ok(WebSocketConnection::from_connection(conn, Role::Client))
    }
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
        .ok_or(WebSocketError::MissingAcceptHeader)?;

    let expected_accept = compute_accept_key(key);
    if accept != &expected_accept {
        return Err(WebSocketError::InvalidAcceptKey);
    }

    Ok(())
}

fn compute_accept_key(client_key: &str) -> String {
    use sha1::{Digest, Sha1};
    use base64::Engine;

    const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    let mut hasher = Sha1::new();
    hasher.update(client_key.as_bytes());
    hasher.update(GUID.as_bytes());
    let hash = hasher.finalize();

    base64::engine::general_purpose::STANDARD.encode(hash)
}
```

### 8.2 WebSocket Connection API

```rust
pub struct WebSocketConnection {
    stream: SharedByteBufferStream<RawStream>,
    role: Role,
    state: ConnectionState,
    assembler: MessageAssembler,
}

impl WebSocketConnection {
    pub fn send(&mut self, message: WebSocketMessage) -> Result<(), WebSocketError> {
        let frame = match message {
            WebSocketMessage::Text(text) => Frame {
                fin: true,
                rsv1: false,
                rsv2: false,
                rsv3: false,
                opcode: Opcode::Text,
                mask: self.generate_mask(),
                payload: text.into_bytes(),
            },
            WebSocketMessage::Binary(data) => Frame {
                fin: true,
                rsv1: false,
                rsv2: false,
                rsv3: false,
                opcode: Opcode::Binary,
                mask: self.generate_mask(),
                payload: data,
            },
            WebSocketMessage::Ping(data) => Frame {
                fin: true,
                rsv1: false,
                rsv2: false,
                rsv3: false,
                opcode: Opcode::Ping,
                mask: self.generate_mask(),
                payload: data,
            },
            WebSocketMessage::Pong(data) => Frame {
                fin: true,
                rsv1: false,
                rsv2: false,
                rsv3: false,
                opcode: Opcode::Pong,
                mask: self.generate_mask(),
                payload: data,
            },
            WebSocketMessage::Close(code, reason) => {
                let mut payload = code.to_be_bytes().to_vec();
                payload.extend_from_slice(reason.as_bytes());
                Frame {
                    fin: true,
                    rsv1: false,
                    rsv2: false,
                    rsv3: false,
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
            let frame = Frame::decode(&mut self.stream)?;

            // Handle control frames immediately
            if frame.opcode.is_control() {
                return self.handle_control_frame(frame);
            }

            // Assemble message from data frames
            if let Some(message) = self.assembler.push_frame(frame)? {
                return Ok(message);
            }
        }
    }

    fn handle_control_frame(&mut self, frame: Frame) -> Result<WebSocketMessage, WebSocketError> {
        match frame.opcode {
            Opcode::Ping => {
                // Auto-respond with Pong
                let pong_frame = Frame {
                    fin: true,
                    rsv1: false,
                    rsv2: false,
                    rsv3: false,
                    opcode: Opcode::Pong,
                    mask: self.generate_mask(),
                    payload: frame.payload.clone(),
                };
                self.send_frame(pong_frame)?;
                Ok(WebSocketMessage::Ping(frame.payload))
            }
            Opcode::Pong => {
                Ok(WebSocketMessage::Pong(frame.payload))
            }
            Opcode::Close => {
                let (code, reason) = parse_close_payload(&frame.payload)?;

                // Update state
                if let ConnectionState::Open = self.state {
                    self.state = ConnectionState::Closing {
                        close_sent: false,
                        close_received: true,
                    };

                    // Send Close response
                    self.close(code, &reason)?;
                }

                Ok(WebSocketMessage::Close(code, reason))
            }
            _ => Err(WebSocketError::InvalidOpcode),
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

        let frame = Frame {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
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

    fn send_frame(&mut self, frame: Frame) -> Result<(), WebSocketError> {
        let encoded = frame.encode()?;
        self.stream.write_all(&encoded)?;
        self.stream.flush()?;
        Ok(())
    }

    fn generate_mask(&self) -> Option<[u8; 4]> {
        match self.role {
            Role::Client => Some(rand::random()),
            Role::Server => None,
        }
    }
}
```

---

## Appendix A: Dependencies

Required crates (add to `Cargo.toml`):

```toml
[dependencies]
sha1 = "0.10"          # For Sec-WebSocket-Accept computation
rand = "0.8"           # For masking key generation
base64 = "0.21"        # For base64 encoding/decoding
```

---

## Appendix B: Testing Strategy

### Unit Tests

1. **Frame encoding/decoding**
   - Test all opcodes
   - Test payload lengths (< 126, < 65536, >= 65536)
   - Test masking/unmasking
   - Test error cases

2. **Message assembly**
   - Test single-frame messages
   - Test fragmented messages
   - Test control frame interleaving
   - Test error cases

3. **Handshake**
   - Test key generation
   - Test accept key computation
   - Test header validation

### Integration Tests

1. **Echo server tests**
   - Connect to public WebSocket echo server
   - Send text message, verify echo
   - Send binary message, verify echo
   - Test ping/pong
   - Test close handshake

2. **TLS tests**
   - Connect to `wss://` server
   - Verify TLS handshake works

3. **Fragmentation tests**
   - Send large message, verify correct fragmentation
   - Receive fragmented message, verify correct assembly

---

## Appendix C: References

- [RFC 6455 - The WebSocket Protocol](https://tools.ietf.org/html/rfc6455)
- [RFC 7692 - Compression Extensions for WebSocket](https://tools.ietf.org/html/rfc7692)
- [IANA WebSocket Close Codes](https://www.iana.org/assignments/websocket/websocket.xml)

---

**Document Version**: 1.0
**Last Updated**: 2026-03-03
**Authors**: Claude AI (via comprehensive codebase analysis)
