---
workspace_name: "ewe_platform"
spec_directory: "specifications/02-build-http-client"
feature_directory: "specifications/02-build-http-client/features/websocket"
this_file: "specifications/02-build-http-client/features/websocket/feature.md"

status: pending
priority: low
created: 2026-02-28
updated: 2026-03-03

depends_on:
  - connection
  - public-api

tasks:
  completed: 0
  uncompleted: 3
  total: 3
  completion_percentage: 0
---


# WebSocket Feature

## Overview

Add WebSocket protocol support (RFC 6455) to the HTTP client, including both client (connect to WebSocket servers) and server (accept WebSocket upgrades) capabilities. This feature implements the WebSocket handshake, frame-based messaging, and integrates with the existing TLS infrastructure.

**IMPORTANT**: See [ARCHITECTURE.md](./ARCHITECTURE.md) for comprehensive analysis of existing infrastructure, design patterns, and detailed implementation guidance.

## Dependencies

This feature depends on:
- `connection` - Uses `HttpClientConnection` for TCP/TLS connections
- `public-api` - Uses client infrastructure (`ClientRequestBuilder`, response handling)
- **Existing infrastructure** (already available):
  - `SimpleHeader::SEC_WEBSOCKET_*` headers (already defined in `simple_http/impls.rs`)
  - `Status::SwitchingProtocols` (101 status code already defined)
  - `SharedByteBufferStream<RawStream>` for frame I/O
  - `Uri` URL parsing (needs extension for `ws://` and `wss://` schemes)
  - `TaskIterator` trait from `valtron` (for Phase 2 non-blocking operations)

This feature is required by:
- None (end-user feature)

### New Dependencies Required

Add to `Cargo.toml`:
```toml
[dependencies]
sha1 = "0.10"          # For Sec-WebSocket-Accept computation
rand = "0.8"           # For masking key generation (client frames)
base64 = "0.21"        # For base64 encoding/decoding of keys
```

## Requirements

### WebSocket Client

Connect to WebSocket servers:

```rust
// Connect to WebSocket
let ws = SimpleHttpClient::new()
    .websocket("wss://example.com/ws")
    .subprotocol("graphql-ws")
    .connect()?;

// Send messages
ws.send(WebSocketMessage::Text("hello".into()))?;
ws.send(WebSocketMessage::Binary(vec![1, 2, 3]))?;

// Receive messages (iterator-based)
for message in ws.messages() {
    match message? {
        WebSocketMessage::Text(text) => println!("{}", text),
        WebSocketMessage::Binary(data) => handle_binary(data),
        WebSocketMessage::Ping(data) => ws.pong(data)?,
        WebSocketMessage::Pong(_) => {},
        WebSocketMessage::Close(code, reason) => {
            println!("Closed: {} - {}", code, reason);
            break;
        }
    }
}

// Graceful close
ws.close(1000, "goodbye")?;
```

### WebSocket Server

Accept WebSocket upgrade requests:

```rust
// Within HTTP server handler
fn handle_request(request: IncomingRequest, conn: Connection) -> Result<(), Error> {
    if WebSocketUpgrade::is_upgrade_request(&request) {
        let ws = WebSocketUpgrade::accept(request, conn)?;
        handle_websocket(ws)
    } else {
        handle_http(request, conn)
    }
}

fn handle_websocket(ws: WebSocketConnection) -> Result<(), Error> {
    for message in ws.messages() {
        // Echo server
        ws.send(message?)?;
    }
    Ok(())
}
```

### Message Types

```rust
pub enum WebSocketMessage {
    /// UTF-8 text message
    Text(String),

    /// Binary data message
    Binary(Vec<u8>),

    /// Ping frame (must respond with Pong)
    Ping(Vec<u8>),

    /// Pong frame (response to Ping)
    Pong(Vec<u8>),

    /// Close frame with status code and reason
    Close(u16, String),
}
```

### Frame Protocol

WebSocket frame structure:

```rust
pub struct WebSocketFrame {
    pub fin: bool,
    pub opcode: Opcode,
    pub mask: Option<[u8; 4]>,
    pub payload: Vec<u8>,
}

pub enum Opcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

impl WebSocketFrame {
    pub fn encode(&self) -> Vec<u8>;
    pub fn decode(reader: &mut impl Read) -> Result<Self, WebSocketError>;
}
```

### Client Handshake

HTTP upgrade handshake:

```rust
// Client sends:
// GET /ws HTTP/1.1
// Host: example.com
// Upgrade: websocket
// Connection: Upgrade
// Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
// Sec-WebSocket-Version: 13
// Sec-WebSocket-Protocol: graphql-ws (optional)

// Server responds:
// HTTP/1.1 101 Switching Protocols
// Upgrade: websocket
// Connection: Upgrade
// Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=

impl WebSocketClient {
    pub fn handshake(
        conn: &mut Connection,
        url: &ParsedUrl,
        options: &WebSocketOptions,
    ) -> Result<Self, WebSocketError> {
        // Generate random key
        let key = generate_websocket_key();

        // Send upgrade request
        let request = format!(
            "GET {} HTTP/1.1\r\n\
             Host: {}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: {}\r\n\
             Sec-WebSocket-Version: 13\r\n\
             {}\r\n",
            url.path_and_query(),
            url.host_with_port(),
            key,
            options.subprotocol.as_ref().map(|p| format!("Sec-WebSocket-Protocol: {}\r\n", p)).unwrap_or_default()
        );
        conn.write_all(request.as_bytes())?;

        // Read response
        let response = read_http_response(conn)?;
        if response.status != 101 {
            return Err(WebSocketError::UpgradeFailed(response.status));
        }

        // Verify accept key
        let expected_accept = compute_accept_key(&key);
        if response.headers.get("Sec-WebSocket-Accept") != Some(&expected_accept) {
            return Err(WebSocketError::InvalidAcceptKey);
        }

        Ok(Self::new(conn, Role::Client))
    }
}
```

### Server Upgrade

Accept upgrade request:

```rust
pub struct WebSocketUpgrade;

impl WebSocketUpgrade {
    pub fn is_upgrade_request(request: &IncomingRequest) -> bool {
        request.headers.get("Upgrade").map(|v| v.eq_ignore_ascii_case("websocket")).unwrap_or(false)
            && request.headers.get("Connection").map(|v| v.to_lowercase().contains("upgrade")).unwrap_or(false)
    }

    pub fn accept(
        request: IncomingRequest,
        mut conn: Connection,
    ) -> Result<WebSocketConnection, WebSocketError> {
        // Extract key
        let key = request.headers.get("Sec-WebSocket-Key")
            .ok_or(WebSocketError::MissingKey)?;

        // Compute accept
        let accept = compute_accept_key(key);

        // Get negotiated subprotocol
        let subprotocol = request.headers.get("Sec-WebSocket-Protocol");

        // Send response
        let response = format!(
            "HTTP/1.1 101 Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Accept: {}\r\n\
             {}\r\n",
            accept,
            subprotocol.map(|p| format!("Sec-WebSocket-Protocol: {}\r\n", p)).unwrap_or_default()
        );
        conn.write_all(response.as_bytes())?;

        Ok(WebSocketConnection::new(conn, Role::Server))
    }
}
```

### WebSocket Connection

Unified client/server WebSocket connection:

```rust
pub struct WebSocketConnection {
    conn: Connection,
    role: Role,
    closed: bool,
    fragmented_message: Option<FragmentedMessage>,
}

pub enum Role {
    Client,
    Server,
}

impl WebSocketConnection {
    /// Send a message
    pub fn send(&mut self, message: WebSocketMessage) -> Result<(), WebSocketError> {
        let frame = match message {
            WebSocketMessage::Text(text) => WebSocketFrame {
                fin: true,
                opcode: Opcode::Text,
                mask: self.generate_mask(),
                payload: text.into_bytes(),
            },
            // ... other message types
        };
        self.send_frame(frame)
    }

    /// Receive next message
    pub fn receive(&mut self) -> Result<WebSocketMessage, WebSocketError> {
        loop {
            let frame = WebSocketFrame::decode(&mut self.conn)?;

            match frame.opcode {
                Opcode::Text if frame.fin => {
                    let text = String::from_utf8(frame.payload)?;
                    return Ok(WebSocketMessage::Text(text));
                }
                Opcode::Binary if frame.fin => {
                    return Ok(WebSocketMessage::Binary(frame.payload));
                }
                Opcode::Ping => {
                    // Auto-respond with pong
                    self.pong(frame.payload.clone())?;
                    return Ok(WebSocketMessage::Ping(frame.payload));
                }
                Opcode::Close => {
                    let (code, reason) = parse_close_payload(&frame.payload);
                    self.closed = true;
                    return Ok(WebSocketMessage::Close(code, reason));
                }
                // Handle fragmentation...
                _ => {}
            }
        }
    }

    /// Iterator over incoming messages
    pub fn messages(&mut self) -> MessageIterator<'_> {
        MessageIterator { ws: self }
    }

    /// Send pong response
    pub fn pong(&mut self, data: Vec<u8>) -> Result<(), WebSocketError>;

    /// Close connection
    pub fn close(&mut self, code: u16, reason: &str) -> Result<(), WebSocketError>;

    fn generate_mask(&self) -> Option<[u8; 4]> {
        // Clients MUST mask, servers MUST NOT mask
        match self.role {
            Role::Client => Some(rand::random()),
            Role::Server => None,
        }
    }
}

pub struct MessageIterator<'a> {
    ws: &'a mut WebSocketConnection,
}

impl<'a> Iterator for MessageIterator<'a> {
    type Item = Result<WebSocketMessage, WebSocketError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ws.closed {
            return None;
        }
        Some(self.ws.receive())
    }
}
```

### TLS Support (wss://)

WebSocket over TLS:

```rust
impl SimpleHttpClient {
    pub fn websocket(&self, url: &str) -> WebSocketBuilder {
        WebSocketBuilder::new(url)
    }
}

impl WebSocketBuilder {
    pub fn connect(self) -> Result<WebSocketConnection, WebSocketError> {
        let url = ParsedUrl::parse(&self.url)?;

        // Connect TCP
        let mut conn = TcpStream::connect((url.host.as_str(), url.port))?;

        // TLS for wss://
        if url.scheme == Scheme::Wss {
            let tls_config = create_tls_config();
            conn = upgrade_to_tls(conn, &url.host, tls_config)?;
        }

        // Perform WebSocket handshake
        WebSocketClient::handshake(&mut conn, &url, &self.options)
    }
}
```

### Subprotocol Negotiation

```rust
pub struct WebSocketOptions {
    pub subprotocols: Vec<String>,
    pub extensions: Vec<String>,
    pub extra_headers: Vec<(String, String)>,
}

impl WebSocketBuilder {
    pub fn subprotocol(mut self, protocol: &str) -> Self {
        self.options.subprotocols.push(protocol.to_string());
        self
    }

    pub fn subprotocols(mut self, protocols: &[&str]) -> Self {
        self.options.subprotocols.extend(protocols.iter().map(|s| s.to_string()));
        self
    }
}
```

### Error Handling

```rust
#[derive(Debug)]
pub enum WebSocketError {
    /// Upgrade request failed
    UpgradeFailed(u16),

    /// Invalid Sec-WebSocket-Accept header
    InvalidAcceptKey,

    /// Missing Sec-WebSocket-Key header
    MissingKey,

    /// Invalid frame format
    InvalidFrame(String),

    /// Invalid UTF-8 in text message
    InvalidUtf8(std::string::FromUtf8Error),

    /// Connection closed unexpectedly
    ConnectionClosed,

    /// Protocol error
    ProtocolError(String),

    /// IO error
    IoError(std::io::Error),
}
```

## Implementation Phases

### Phase 1: Core WebSocket (Blocking I/O)

**Duration**: 1-2 weeks
**Goal**: Working WebSocket client with blocking I/O

**File structure**:
```
backends/foundation_core/src/wire/
└── websocket/                  # NEW module
    ├── mod.rs                  # Public API and re-exports
    ├── frame.rs                # Frame encoding/decoding
    ├── handshake.rs            # HTTP upgrade handshake
    ├── connection.rs           # WebSocketConnection
    ├── message.rs              # WebSocketMessage enum
    └── error.rs                # WebSocketError
```

**Tasks**:
1. **Frame encoding/decoding** (`frame.rs`)
   - Implement `Frame` struct with FIN, RSV, opcode, mask, payload fields
   - Implement `encode()` method (handle payload lengths: <126, 126-65535, ≥65536)
   - Implement `decode()` method (read from `impl Read`)
   - Add `apply_mask()` helper function
   - Unit tests for all opcodes and payload lengths

2. **WebSocket handshake** (`handshake.rs`)
   - Implement `compute_accept_key(client_key)` using SHA-1 + base64
   - Implement `generate_websocket_key()` (16 random bytes, base64 encoded)
   - Build HTTP upgrade request using existing `ClientRequestBuilder`:
     ```rust
     ClientRequestBuilder::get(url)?
         .header(SimpleHeader::UPGRADE, "websocket")
         .header(SimpleHeader::CONNECTION, "Upgrade")
         .header(SimpleHeader::SEC_WEBSOCKET_KEY, &key)
         .header(SimpleHeader::SEC_WEBSOCKET_VERSION, "13")
     ```
   - Parse 101 response using existing `HttpResponseReader`
   - Validate `Sec-WebSocket-Accept` header
   - Return `HttpClientConnection` on success

3. **URL scheme extension**
   - Extend `simple_http/url/scheme.rs` to support `ws://` and `wss://` schemes
   - Add `is_websocket()` and `is_secure_websocket()` methods

4. **WebSocket connection** (`connection.rs`)
   - Implement `WebSocketConnection` struct:
     ```rust
     pub struct WebSocketConnection {
         stream: SharedByteBufferStream<RawStream>,
         role: Role,  // Client or Server
         state: ConnectionState,  // Open, Closing, Closed
         assembler: MessageAssembler,  // Handle fragmentation
     }
     ```
   - Implement `send()` method (encode frame + write to stream)
   - Implement `recv()` method (read + decode frame + assemble message)
   - Implement `close()` method (send Close frame, handle close handshake)
   - Auto-respond to Ping with Pong
   - Handle message fragmentation (multiple frames with FIN flag)

5. **Message types** (`message.rs`)
   - Define `WebSocketMessage` enum (Text, Binary, Ping, Pong, Close)
   - Implement `MessageAssembler` for fragment handling
   - Conversion helpers

6. **Error handling** (`error.rs`)
   - Define `WebSocketError` enum
   - Conversions from `io::Error`, `FromUtf8Error`, etc.

**Success Criteria**:
- Can connect to `ws://` servers (plain TCP)
- Can connect to `wss://` servers (TLS via existing infrastructure)
- Can send/receive Text and Binary messages
- Ping/Pong auto-response works
- Close handshake works correctly
- Message fragmentation works
- All unit tests pass

### Phase 2: TaskIterator Integration (Non-Blocking)

**Duration**: 1-2 weeks
**Goal**: Non-blocking WebSocket operations

**File structure** (additions):
```
backends/foundation_core/src/wire/websocket/
└── task.rs                     # TaskIterator implementation
```

**Tasks**:
1. **WebSocket task** (`task.rs`)
   - Implement `WebSocketTask` state machine:
     ```rust
     enum WebSocketTaskState {
         Init(WebSocketConnectInfo),
         Connecting(DrivenRecvIterator<ConnectTask>),
         UpgradeRequest(DrivenRecvIterator<HttpUpgradeTask>),
         Open(WebSocketConnection),
         Closed,
     }
     ```
   - Implement `TaskIterator` trait:
     ```rust
     impl TaskIterator for WebSocketTask {
         type Ready = WebSocketMessage;
         type Pending = WebSocketPending;
         type Spawner = BoxedSendExecutionAction;
     }
     ```
   - Non-blocking connect via task spawning
   - Non-blocking send/receive

2. **Frame reader task**
   - Implement `FrameReaderTask` for non-blocking frame reading
   - Integration with `MessageAssembler`

3. **Integration with existing client infrastructure**
   - Reuse DNS resolution from `DnsResolver`
   - Reuse connection pooling from `HttpConnectionPool` (optional)
   - Reuse TLS handshake patterns

**Success Criteria**:
- Non-blocking connect works
- Can send/receive without blocking thread
- Integrates with valtron executor
- Performance is acceptable
- Can handle multiple concurrent WebSocket connections

### Phase 3: Advanced Features

**Duration**: 1-2 weeks (optional)
**Goal**: Production-ready WebSocket support

**Tasks**:
1. **Subprotocol negotiation**
   - Support `Sec-WebSocket-Protocol` header in handshake
   - Validate negotiated subprotocol in response

2. **Server-side WebSocket** (`server.rs`)
   - Implement `WebSocketUpgrade::is_upgrade_request()`
   - Implement `WebSocketUpgrade::accept()` (compute accept key, send 101 response)
   - Server-side connection handling (must NOT mask frames)

3. **Performance optimizations**
   - Zero-copy frame parsing where possible
   - Buffer pooling for frames
   - Batch frame writing

**Success Criteria**:
- Subprotocol negotiation works correctly
- Server-side support works (accept upgrades)
- Performance benchmarks meet requirements

## Success Criteria

**Phase 1 (Core - Blocking)**:
- [ ] `websocket/` module exists and compiles
- [ ] `Frame` struct correctly encodes/decodes all frame types
- [ ] Frame encoding handles payload lengths: <126, 126-65535, ≥65536
- [ ] Client masking is correct (clients MUST mask all outgoing frames)
- [ ] `compute_accept_key()` generates correct SHA-1 + base64 hashes
- [ ] Client handshake works with `ws://` URLs (plain TCP)
- [ ] Client handshake works with `wss://` URLs (TLS)
- [ ] Can send Text and Binary messages
- [ ] Can receive Text and Binary messages
- [ ] Ping/Pong auto-response works (receiver must respond with Pong)
- [ ] Close handshake works correctly (bidirectional)
- [ ] Message fragmentation works (multi-frame messages with FIN flag)
- [ ] Control frames are never fragmented (per RFC 6455)
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`
- [ ] Integration test with public echo server passes

**Phase 2 (Non-Blocking)**:
- [ ] `WebSocketTask` implements `TaskIterator` trait
- [ ] Non-blocking connect works
- [ ] Non-blocking send/receive works
- [ ] Integrates with valtron executor
- [ ] Can handle multiple concurrent connections
- [ ] Performance benchmarks pass

**Phase 3 (Advanced - Optional)**:
- [ ] Subprotocol negotiation works
- [ ] Server-side upgrade handling works
- [ ] Server does NOT mask outgoing frames (per RFC 6455)
- [ ] Performance optimizations implemented

## Verification Commands

```bash
# Format check
cargo fmt -- --check

# Clippy linting
cargo clippy --package foundation_core --features ssl-rustls -- -D warnings

# Unit tests
cargo test --package foundation_core -- websocket

# Build without TLS
cargo build --package foundation_core

# Build with TLS (rustls)
cargo build --package foundation_core --features ssl-rustls

# Build with TLS (OpenSSL)
cargo build --package foundation_core --features ssl-openssl

# Build with TLS (native-tls)
cargo build --package foundation_core --features ssl-native-tls

# Integration test with echo server (requires network)
cargo test --package foundation_core --features ssl-rustls -- websocket::integration --ignored
```

## Notes for Implementation Agents

### Critical Pre-Checks

Before starting implementation, **MUST**:
1. **Read [ARCHITECTURE.md](./ARCHITECTURE.md)** - Comprehensive infrastructure analysis and design
2. **Verify dependencies**: `connection` and `public-api` features are complete
3. **Read RFC 6455** - The WebSocket Protocol specification
4. **Explore existing code**:
   - `simple_http/impls.rs` - WebSocket headers already defined
   - `simple_http/client/connection.rs` - TCP/TLS connection patterns
   - `simple_http/client/tasks/send_request.rs` - TaskIterator example
   - `io/ioutils/mod.rs` - Stream abstractions

### Key Implementation Rules

**Masking (RFC 6455 Section 5.3)**:
- Clients **MUST** mask all outgoing frames (use random 4-byte mask)
- Servers **MUST NOT** mask outgoing frames
- Violation of this rule will cause connection failures

**Frame Encoding**:
- Payload length: 7 bits for <126, 7+16 bits for 126-65535, 7+64 bits for ≥65536
- Extended length is big-endian
- Masking key is sent before payload (if MASK bit set)
- Apply mask: `payload[i] ^= mask[i % 4]`

**Control Frames (RFC 6455 Section 5.5)**:
- **MUST NOT** be fragmented (FIN=1 always)
- **MUST** have payload ≤125 bytes
- Can be injected between fragmented message frames
- Ping **MUST** be answered with Pong (same payload)

**Message Fragmentation**:
- First frame: FIN=0, opcode=Text/Binary
- Middle frames: FIN=0, opcode=Continuation
- Last frame: FIN=1, opcode=Continuation
- Control frames can appear between fragments

**Close Handshake**:
- Initiator sends Close frame
- Receiver responds with Close frame
- Both sides close TCP connection
- Close payload: 2-byte code (big-endian) + optional UTF-8 reason

**HTTP Upgrade Handshake**:
- Use existing `ClientRequestBuilder` to build request
- Add required headers: `Upgrade`, `Connection`, `Sec-WebSocket-Key`, `Sec-WebSocket-Version`
- Read response using existing `HttpResponseReader`
- Validate `Status::SwitchingProtocols` (101)
- Compute and verify `Sec-WebSocket-Accept` header
- After handshake, take ownership of stream for frame I/O

**TLS Support**:
- For `wss://` URLs, TLS handshake happens **before** WebSocket handshake
- Reuse existing TLS infrastructure (no new TLS code needed)
- `HttpClientConnection::connect()` already handles TLS for `https://`
- Extend URL scheme support to recognize `wss://` as secure

### Security Considerations

- **Validate UTF-8**: Text frames must contain valid UTF-8 (use `String::from_utf8()`)
- **Limit sizes**: Enforce max frame size (e.g., 16MB) to prevent DoS
- **Close codes**: Use standard codes (1000-1011), don't expose internal errors
- **Random masking**: Use cryptographically secure random for mask (client-side)

### Reusable Components

**From existing codebase**:
- `SimpleHeader::SEC_WEBSOCKET_*` - Headers already defined
- `Status::SwitchingProtocols` - 101 status code
- `HttpClientConnection` - TCP/TLS connection wrapper
- `SharedByteBufferStream<RawStream>` - Buffered Read/Write stream
- `ClientRequestBuilder` - Build HTTP upgrade request
- `HttpResponseReader` - Parse HTTP response
- `TaskIterator` - Non-blocking state machine pattern (Phase 2)
- `DnsResolver` - Hostname resolution
- `HttpConnectionPool` - Connection reuse (optional)

**Do NOT re-implement**:
- HTTP request building (use `ClientRequestBuilder`)
- HTTP response parsing (use `HttpResponseReader`)
- TLS handshake (use `HttpClientConnection::connect()`)
- DNS resolution (use `DnsResolver`)

### Testing Strategy

**Unit Tests**:
1. Frame encoding/decoding (all opcodes, payload lengths)
2. Masking/unmasking
3. Message assembly (single-frame, fragmented)
4. Accept key computation (test vectors from RFC 6455)
5. Error handling

**Integration Tests**:
1. Connect to public echo server (e.g., `wss://echo.websocket.org`)
2. Send text message, verify echo
3. Send binary message, verify echo
4. Test ping/pong
5. Test close handshake
6. Test large messages (fragmentation)

**Test Vectors (RFC 6455 Section 1.3)**:
```
Client key: dGhlIHNhbXBsZSBub25jZQ==
Expected accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=
```

### Common Pitfalls to Avoid

1. **Forgetting to mask** (client frames) or **masking when you shouldn't** (server frames)
2. **Not handling fragmentation** (assuming all messages fit in one frame)
3. **Not responding to Ping** (must auto-respond with Pong)
4. **Not validating UTF-8** in Text frames
5. **Fragmenting control frames** (they must be single-frame)
6. **Wrong byte order** for extended payload length (must be big-endian)
7. **Wrong close handshake** (both sides must send Close frame)
8. **Re-implementing HTTP parsing** (use existing infrastructure)

---
*Created: 2026-02-28*
*Last Updated: 2026-03-03*
*See [ARCHITECTURE.md](./ARCHITECTURE.md) for comprehensive design documentation*
