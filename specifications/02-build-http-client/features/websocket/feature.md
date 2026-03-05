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

### Architectural Principle: TaskIterator with DrivenSendTaskIterator Wrappers

**CRITICAL:** All WebSocket client implementations MUST use TaskIterator as the core pattern with `DrivenSendTaskIterator` for blocking wrappers. The TaskIterator is a pure state machine with NO loops - execution driving happens in the wrapper.

**Design Hierarchy:**
1. **TaskIterator (Core)** - Pure state machine, ONE step per `next()` call, NO loops
2. **Plain Iterator (Internal)** - Used internally for frame parsing, wrapped by TaskIterator
3. **DrivenSendTaskIterator (Wrapper)** - Handles execution driving via `run_until_next_state()`
4. **Blocking Iterator API (Convenience)** - Wraps DrivenSendTaskIterator, extracts Ready values

### WebSocket Client (TaskIterator - Core API)

Connect to WebSocket servers using TaskIterator:

```rust
use foundation_core::wire::websocket::WebSocketTask;
use foundation_core::valtron::{TaskIterator, TaskStatus};

// Create WebSocket task - this is the core API
let mut task = WebSocketTask::connect("wss://example.com/ws")?
    .subprotocol("graphql-ws");

// Use with valtron executor
loop {
    match task.next() {
        Some(TaskStatus::Ready(result)) => {
            match result {
                Ok(WebSocketMessage::Text(text)) => println!("{}", text),
                Ok(WebSocketMessage::Binary(data)) => handle_binary(data),
                Ok(WebSocketMessage::Ping(data)) => {
                    // Auto-respond with pong
                    task.pong(data)?;
                }
                Ok(WebSocketMessage::Close(code, reason)) => {
                    println!("Closed: {} - {}", code, reason);
                    break;
                }
                Err(e) => eprintln!("WebSocket Error: {:?}", e),
            }
        }
        Some(TaskStatus::Pending(progress)) => {
            // Task is working - do other work or wait
        }
        Some(TaskStatus::Delayed(duration)) => {
            // Reconnection backoff - executor handles delay
        }
        Some(TaskStatus::Spawn(action)) => {
            // Executor spawns sub-task (e.g., connection task)
            executor.spawn(action)?;
        }
        None => {
            // Task complete (connection closed)
            break;
        }
    }
}
```

### WebSocket Client (Blocking Iterator - Convenience API)

For simple use cases, a blocking iterator wrapper is provided using `DrivenSendTaskIterator`:

```rust
use foundation_core::wire::websocket::{WebSocketConnection, WebSocketMessage};

// Connect to WebSocket (blocking convenience wrapper)
let mut ws = WebSocketConnection::connect("wss://example.com/ws")?
    .subprotocol("graphql-ws")
    .connect();

// Send messages
ws.send(WebSocketMessage::Text("hello".into()))?;
ws.send(WebSocketMessage::Binary(vec![1, 2, 3]))?;

// Receive messages (blocking iterator - uses DrivenSendTaskIterator internally)
// NO LOOPS in TaskIterator - execution driving is in DrivenSendTaskIterator
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

**Implementation Note:** The blocking `WebSocketConnection` wrapper uses `DrivenSendTaskIterator` which handles execution driving externally via `run_until_next_state()`. The TaskIterator itself has NO loops.

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

HTTP upgrade handshake using existing RenderHttp infrastructure:

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
        conn: &mut HttpClientConnection,
        url: &ParsedUrl,
        options: &WebSocketOptions,
    ) -> Result<Self, WebSocketError> {
        // Generate random key
        let key = generate_websocket_key();

        // Build upgrade request using SimpleIncomingRequestBuilder
        let mut builder = SimpleIncomingRequestBuilder::get(url.path_and_query())
            .header(SimpleHeader::HOST, url.host_with_port())
            .header(SimpleHeader::UPGRADE, "websocket")
            .header(SimpleHeader::CONNECTION, "Upgrade")
            .header(SimpleHeader::SEC_WEBSOCKET_KEY, &key)
            .header(SimpleHeader::SEC_WEBSOCKET_VERSION, "13");

        // Add subprotocol if provided
        if let Some(subprotocol) = &options.subprotocol {
            builder = builder.header(SimpleHeader::SEC_WEBSOCKET_PROTOCOL, subprotocol);
        }

        let request = builder.build()?;

        // Render and send request using RenderHttp
        let request_bytes = Http11::request(&request).http_render()?;

        for chunk in request_bytes {
            conn.write_all(&chunk?)?;
        }

        // Read response using HttpResponseReader
        let mut response_reader = HttpResponseReader::new(conn)?;
        let response = response_reader.read_response()?;

        if response.status != Status::SwitchingProtocols {
            return Err(WebSocketError::UpgradeFailed(response.status.code()));
        }

        // Verify accept key
        let expected_accept = compute_accept_key(&key);
        let actual_accept = response.headers.get(SimpleHeader::SEC_WEBSOCKET_ACCEPT)
            .ok_or(WebSocketError::MissingAcceptKey)?;

        if actual_accept != expected_accept {
            return Err(WebSocketError::InvalidAcceptKey);
        }

        Ok(Self::new(conn, Role::Client))
    }
}
```

### Server Upgrade

Accept upgrade request using existing RenderHttp infrastructure:

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
        // Extract key
        let key = request.headers.get(SimpleHeader::SEC_WEBSOCKET_KEY)
            .ok_or(WebSocketError::MissingKey)?;

        // Compute accept
        let accept = compute_accept_key(key);

        // Get negotiated subprotocol
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

    /// Missing Sec-WebSocket-Accept header
    MissingAcceptKey,

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

### Phase 1: Core WebSocket (Blocking I/O) with TaskIterator

**Duration**: 1-2 weeks
**Goal**: Working WebSocket client with TaskIterator and DrivenSendTaskIterator wrapper

**File structure**:
```
backends/foundation_core/src/wire/
└── websocket/                  # NEW module
    ├── mod.rs                  # Public API and re-exports
    ├── frame.rs                # Frame encoding/decoding
    ├── handshake.rs            # HTTP upgrade handshake
    ├── task.rs                 # WebSocketTask (TaskIterator - core API)
    ├── connection.rs           # WebSocketConnection (DrivenSendTaskIterator wrapper)
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
   - Build HTTP upgrade request using existing `SimpleIncomingRequestBuilder`
   - Render request using `Http11::request().http_render()` (RenderHttp trait)
   - Parse 101 response using existing `HttpResponseReader`
   - Validate `Sec-WebSocket-Accept` header
   - Return `WebSocketConnection` on success (takes ownership of stream)

3. **URL scheme extension**
   - Extend `simple_http/url/scheme.rs` to support `ws://` and `wss://` schemes
   - Add `is_websocket()` and `is_secure_websocket()` methods

4. **WebSocket Task** (`task.rs`) - **Core TaskIterator**
   - Follow pattern from `simple_http/client/tasks/send_request.rs`
   - State enum with `Option<Box<...>>` for data carrying
   - State machine: Init → Connecting → Handshake → Open → Closed
   - NO loops in `next()` - ONE step per call

   ```rust
   use crate::valtron::{TaskIterator, TaskStatus, BoxedSendExecutionAction, DrivenRecvIterator};

   // Task wraps state in Option for termination
   pub struct WebSocketTask(Option<WebSocketState>);

   // State carries ALL data - follows send_request.rs pattern
   enum WebSocketState {
       Init(Option<Box<WebSocketConnectInfo>>),
       Connecting(DrivenRecvIterator<HttpConnectTask>),
       Handshake(Box<HandshakeState>),
       Open(WebSocketStream),
       Closed,
   }

   // Handshake state machine - detailed sub-states for HTTP upgrade
   enum HandshakeState {
       BuildingRequest(Box<WebSocketConnectInfo>),
       SendingRequest {
           connection: HttpClientConnection,
           request_bytes: Vec<Vec<u8>>,
           current_chunk: usize,
       },
       ReadingResponse {
           connection: HttpClientConnection,
           reader: HttpResponseReader<SimpleHttpBody, RawStream>,
       },
       ValidatingResponse {
           connection: HttpClientConnection,
           status: Status,
           headers: SimpleHeaders,
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
               WebSocketState::Init(mut info_opt) => match info_opt.take() {
                   Some(info) => {
                       // Spawn TCP/TLS connection sub-task
                       let (action, recv) = spawn_connect_task(&info.url);
                       self.0 = Some(WebSocketState::Connecting(recv));
                       Some(TaskStatus::Spawn(action))
                   }
                   None => {
                       self.0 = Some(WebSocketState::Closed);
                       None
                   }
               }
               WebSocketState::Connecting(mut recv) => {
                   // ONE step: call next() on receiver
                   let result = recv.next();
                   self.0 = Some(WebSocketState::Connecting(recv));
                   match result {
                       Some(TaskStatus::Ready(Ok(conn))) => {
                           // Connection established, start handshake
                           // Transition to Handshake::BuildingRequest
                           let info = extract_connect_info(); // Extract from previous state
                           self.0 = Some(WebSocketState::Handshake(Box::new(
                               HandshakeState::BuildingRequest(Box::new(info))
                           )));
                           Some(TaskStatus::Pending(WebSocketPending::HandshakeSending))
                       }
                       Some(TaskStatus::Ready(Err(e))) => {
                           self.0 = Some(WebSocketState::Closed);
                           Some(TaskStatus::Ready(Err(WebSocketError::ConnectionFailed(e))))
                       }
                       Some(TaskStatus::Pending(_)) => {
                           Some(TaskStatus::Pending(WebSocketPending::Connecting))
                       }
                       Some(TaskStatus::Delayed(duration)) => {
                           Some(TaskStatus::Delayed(duration))
                       }
                       Some(TaskStatus::Spawn(action)) => {
                           Some(TaskStatus::Spawn(action))
                       }
                       None => {
                           self.0 = Some(WebSocketState::Closed);
                           Some(TaskStatus::Ready(Err(WebSocketError::ConnectionClosed)))
                       }
                   }
               }
               WebSocketState::Handshake(mut handshake_state) => {
                   // Handle detailed handshake sub-states
                   match *handshake_state {
                       HandshakeState::BuildingRequest(mut info) => {
                           // Generate random WebSocket key
                           let ws_key = generate_websocket_key();

                           // Build HTTP upgrade request
                           let mut builder = SimpleIncomingRequestBuilder::get(info.path.clone())
                               .header(SimpleHeader::HOST, info.host.clone())
                               .header(SimpleHeader::UPGRADE, "websocket")
                               .header(SimpleHeader::CONNECTION, "Upgrade")
                               .header(SimpleHeader::SEC_WEBSOCKET_KEY, &ws_key)
                               .header(SimpleHeader::SEC_WEBSOCKET_VERSION, "13");

                           // Add subprotocol if requested
                           if let Some(ref protocol) = info.subprotocol {
                               builder = builder.header(SimpleHeader::SEC_WEBSOCKET_PROTOCOL, protocol);
                           }

                           // Add extra headers
                           for (name, value) in info.extra_headers.iter() {
                               builder = builder.header(name.clone(), value.clone());
                           }

                           let request = builder.build()?;

                           // Render request to bytes using RenderHttp
                           let request_bytes = Http11::request(&request).http_render()?;

                           // Transition to SendingRequest
                           self.0 = Some(WebSocketState::Handshake(Box::new(
                               HandshakeState::SendingRequest {
                                   connection: info.connection.take().unwrap(),
                                   request_bytes,
                                   current_chunk: 0,
                               }
                           )));
                           Some(TaskStatus::Pending(WebSocketPending::HandshakeSending))
                       }
                       HandshakeState::SendingRequest {
                           mut connection,
                           request_bytes,
                           mut current_chunk,
                       } => {
                           // Send ONE chunk of request bytes
                           if current_chunk < request_bytes.len() {
                               let chunk = &request_bytes[current_chunk];
                               match connection.stream_mut().write(chunk) {
                                   Ok(0) => {
                                       // Connection closed during write
                                       self.0 = Some(WebSocketState::Closed);
                                       Some(TaskStatus::Ready(Err(WebSocketError::ConnectionClosed)))
                                   }
                                   Ok(_) => {
                                       current_chunk += 1;
                                       self.0 = Some(WebSocketState::Handshake(Box::new(
                                           HandshakeState::SendingRequest {
                                               connection,
                                               request_bytes,
                                               current_chunk,
                                           }
                                       )));
                                       Some(TaskStatus::Pending(WebSocketPending::HandshakeSending))
                                   }
                                   Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                       // Socket not ready, try again
                                       self.0 = Some(WebSocketState::Handshake(Box::new(
                                           HandshakeState::SendingRequest {
                                               connection,
                                               request_bytes,
                                               current_chunk,
                                           }
                                       )));
                                       Some(TaskStatus::Pending(WebSocketPending::HandshakeSending))
                                   }
                                   Err(e) => {
                                       self.0 = Some(WebSocketState::Closed);
                                       Some(TaskStatus::Ready(Err(WebSocketError::IoError(e))))
                                   }
                               }
                           } else {
                               // All chunks sent, flush and transition to reading
                               match connection.stream_mut().flush() {
                                   Ok(()) => {
                                       // Create response reader
                                       let reader = HttpResponseReader::new(
                                           connection.clone_stream(),
                                           SimpleHttpBody,
                                       );
                                       self.0 = Some(WebSocketState::Handshake(Box::new(
                                           HandshakeState::ReadingResponse {
                                               connection,
                                               reader,
                                           }
                                       )));
                                       Some(TaskStatus::Pending(WebSocketPending::HandshakeReading))
                                   }
                                   Err(e) => {
                                       self.0 = Some(WebSocketState::Closed);
                                       Some(TaskStatus::Ready(Err(WebSocketError::IoError(e))))
                                   }
                               }
                           }
                       }
                       HandshakeState::ReadingResponse {
                           mut connection,
                           mut reader,
                       } => {
                           // Read ONE part of response
                           match reader.next() {
                               Some(Ok(IncomingResponseParts::Intro(status, _proto, _text))) => {
                                   // Check if 101 Switching Protocols
                                   if status != Status::SwitchingProtocols {
                                       self.0 = Some(WebSocketState::Closed);
                                       return Some(TaskStatus::Ready(Err(
                                           WebSocketError::UpgradeFailed(status.into_usize())
                                       )));
                                   }
                                   // Store status, continue reading headers
                                   self.0 = Some(WebSocketState::Handshake(Box::new(
                                       HandshakeState::ReadingResponse {
                                           connection,
                                           reader,
                                       }
                                   )));
                                   Some(TaskStatus::Pending(WebSocketPending::HandshakeReading))
                               }
                               Some(Ok(IncomingResponseParts::Headers(headers))) => {
                                   // Got headers, transition to validation
                                   self.0 = Some(WebSocketState::Handshake(Box::new(
                                       HandshakeState::ValidatingResponse {
                                           connection,
                                           status: Status::SwitchingProtocols,
                                           headers,
                                       }
                                   )));
                                   Some(TaskStatus::Pending(WebSocketPending::HandshakeReading))
                               }
                               Some(Ok(IncomingResponseParts::Body(_))) => {
                                   // Body in handshake response - unexpected but continue
                                   self.0 = Some(WebSocketState::Handshake(Box::new(
                                       HandshakeState::ReadingResponse {
                                           connection,
                                           reader,
                                       }
                                   )));
                                   Some(TaskStatus::Pending(WebSocketPending::HandshakeReading))
                               }
                               Some(Err(e)) => {
                                   self.0 = Some(WebSocketState::Closed);
                                   Some(TaskStatus::Ready(Err(WebSocketError::HttpError(e))))
                               }
                               None => {
                                   // Reader exhausted without headers
                                   self.0 = Some(WebSocketState::Closed);
                                   Some(TaskStatus::Ready(Err(WebSocketError::HandshakeFailed(
                                       "Response ended before headers".into()
                                   ))))
                               }
                           }
                       }
                       HandshakeState::ValidatingResponse {
                           connection,
                           status: _,
                           headers,
                       } => {
                           // Validate Sec-WebSocket-Accept header
                           let expected_accept = compute_accept_key(&self.stored_ws_key);
                           let actual_accept = headers
                               .get(&SimpleHeader::SEC_WEBSOCKET_ACCEPT)
                               .and_then(|v| v.first())
                               .map(|s| s.as_str());

                           match actual_accept {
                               Some(accept) if accept == expected_accept => {
                                   // Handshake successful!
                                   // Extract subprotocol if negotiated
                                   let negotiated_protocol = headers
                                       .get(&SimpleHeader::SEC_WEBSOCKET_PROTOCOL)
                                       .and_then(|v| v.first())
                                       .cloned();

                                   // Create WebSocket stream
                                   let ws_stream = WebSocketStream::new(
                                       connection,
                                       Role::Client,
                                       negotiated_protocol,
                                   );

                                   self.0 = Some(WebSocketState::Open(ws_stream));
                                   Some(TaskStatus::Ready(Ok(WebSocketMessage::ConnectionEstablished)))
                               }
                               Some(accept) => {
                                   // Invalid accept key
                                   self.0 = Some(WebSocketState::Closed);
                                   Some(TaskStatus::Ready(Err(
                                       WebSocketError::InvalidAcceptKey {
                                           expected: expected_accept,
                                           actual: accept.to_string(),
                                       }
                                   )))
                               }
                               None => {
                                   // Missing accept key
                                   self.0 = Some(WebSocketState::Closed);
                                   Some(TaskStatus::Ready(Err(
                                       WebSocketError::MissingAcceptKey
                                   )))
                               }
                           }
                       }
                   }
               }
               WebSocketState::Open(mut stream) => {
                   // ONE step: read ONE frame from stream
                   match stream.next_frame() {
                       Ok(frame) => {
                           self.0 = Some(WebSocketState::Open(stream));
                           Some(TaskStatus::Ready(Ok(frame.to_message()?)))
                       }
                       Err(WebSocketError::ConnectionClosed) => {
                           self.0 = Some(WebSocketState::Closed);
                           None  // Task done
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

   **Key Handshake Implementation Details:**

   1. **BuildingRequest State:**
      - Stores `WebSocketConnectInfo` with URL, headers, subprotocol preferences
      - Generates random 16-byte WebSocket key, base64 encoded
      - Uses `SimpleIncomingRequestBuilder` to construct HTTP GET request
      - Adds mandatory WebSocket headers: Upgrade, Connection, Sec-WebSocket-Key, Sec-WebSocket-Version
      - Optionally adds Sec-WebSocket-Protocol for subprotocol negotiation

   2. **SendingRequest State:**
      - Stores rendered request bytes as `Vec<Vec<u8>>` (chunks from `http_render()`)
      - Tracks `current_chunk` index for incremental sending
      - Writes ONE chunk per `next()` call
      - Handles `WouldBlock` by retrying same chunk
      - Flushes after all chunks sent

   3. **ReadingResponse State:**
      - Uses `HttpResponseReader<SimpleHttpBody, RawStream>` for response parsing
      - Reads ONE `IncomingResponseParts` per `next()` call
      - Expects: Intro → Headers → (optional Body)
      - Validates Status::SwitchingProtocols (101) on Intro

   4. **ValidatingResponse State:**
      - Extracts Sec-WebSocket-Accept from headers
      - Computes expected accept: `base64(sha1(client_key + GUID))`
      - Compares expected vs actual accept
      - Extracts negotiated subprotocol from Sec-WebSocket-Protocol header
      - On success: creates WebSocketStream, transitions to Open state

   **Handshake Error Handling:**
   - Connection failure during SendingRequest → ConnectionClosed
   - Non-101 status → UpgradeFailed(status_code)
   - Missing Sec-WebSocket-Accept → MissingAcceptKey
   - Invalid Sec-WebSocket-Accept → InvalidAcceptKey { expected, actual }
   - Response ends before headers → HandshakeFailed(message)
   - I/O errors → IoError(inner_error)

5. **WebSocket Connection** (`connection.rs`) - **DrivenSendTaskIterator Wrapper**
   - Wraps `WebSocketTask` with `DrivenSendTaskIterator`
   - Provides simple blocking API for simple use cases
   - NO loops in TaskIterator - execution driving handled by wrapper

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

       pub fn send(&mut self, message: WebSocketMessage) -> Result<(), WebSocketError> {
           // Send via TaskIterator
           // Implementation detail
       }

       pub fn messages(&mut self) -> MessageIterator<'_> {
           MessageIterator { conn: self }
       }
   }

   pub struct MessageIterator<'a> {
       conn: &'a mut WebSocketConnection,
   }

   impl<'a> Iterator for MessageIterator<'a> {
       type Item = Result<WebSocketMessage, WebSocketError>;

       fn next(&mut self) -> Option<Self::Item> {
           // DrivenSendTaskIterator handles execution driving
           // Calls run_until_next_state() then task.next() ONCE
           // NO LOOPS in TaskIterator
           match self.conn.driven.next() {
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

6. **Message types** (`message.rs`)
   - Define `WebSocketMessage` enum (Text, Binary, Ping, Pong, Close)
   - Implement `MessageAssembler` for fragment handling
   - Conversion helpers

7. **Error handling** (`error.rs`)
   - Define `WebSocketError` enum
   - Conversions from `io::Error`, `FromUtf8Error`, etc.

**Success Criteria**:
- [ ] `WebSocketTask` implements `TaskIterator` correctly
- [ ] State machine follows valtron pattern (Init → Connecting → Handshake → Open → Closed)
- [ ] State carries ALL data between transitions (Option<Box<...>> pattern)
- [ ] NO LOOPS in `next()` - each call does ONE step
- [ ] `TaskStatus` variants used correctly (Ready, Pending, Delayed, Spawn)
- [ ] `Frame` struct correctly encodes/decodes all frame types
- [ ] Frame encoding handles payload lengths: <126, 126-65535, ≥65536
- [ ] Client masking is correct (clients MUST mask all outgoing frames)
- [ ] `compute_accept_key()` generates correct SHA-1 + base64 hashes
- [ ] Client handshake works with `ws://` URLs (plain TCP)
- [ ] Client handshake works with `wss://` URLs (TLS)
- [ ] `DrivenSendTaskIterator` wrapper works for blocking use cases
- [ ] `WebSocketConnection` (blocking wrapper) sends/receives messages correctly
- [ ] Can send Text and Binary messages
- [ ] Can receive Text and Binary messages
- [ ] Ping/Pong auto-response works (receiver must respond with Pong)
- [ ] Close handshake works correctly (bidirectional)
- [ ] Message fragmentation works (multi-frame messages with FIN flag)
- [ ] Control frames are never fragmented (per RFC 6455)
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`
- [ ] Integration test with public echo server passes

### Phase 2: Advanced TaskIterator Features

**Duration**: 1 week
**Goal**: Full valtron executor integration and advanced features

**File structure** (additions):
```
backends/foundation_core/src/wire/websocket/
└── Reconnecting_task.rs        # ReconnectingWebSocketTask (TaskIterator)
```

**Tasks**:

1. **ReconnectingWebSocketTask** (`reconnecting_task.rs`)
   - Wrap `WebSocketTask` with reconnection logic
   - Follow pattern from `send_request.rs` - state carries ALL data
   - Use existing `ExponentialBackoffDecider` for backoff
   - Auto-reconnect on connection loss
   - TaskIterator state machine

   ```rust
   use crate::valtron::{TaskIterator, TaskStatus, BoxedSendExecutionAction};

   // Task wraps state in Option for termination
   pub struct ReconnectingWebSocketTask(Option<ReconnectingWebSocketState>);

   enum ReconnectingWebSocketState {
       Init(Option<Box<WebSocketConnectInfo>>),
       Connecting(WebSocketTask),
       Open(WebSocketTask),
       Reconnecting(Box<(Duration, RetryState, WebSocketConnectInfo)>),
       Exhausted,
   }

   impl TaskIterator for ReconnectingWebSocketTask {
       type Ready = Result<WebSocketMessage, WebSocketError>;
       type Pending = WebSocketPending;
       type Spawner = BoxedSendExecutionAction;

       fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
           match self.0.take()? {
               // Handle reconnection state machine
               // On connection loss: transition to Reconnecting state
               // On reconnect: re-establish WebSocket connection
               // Respect backoff duration
           }
       }
   }
   ```

2. **Subprotocol negotiation**
   - Support `Sec-WebSocket-Protocol` header in handshake
   - Validate negotiated subprotocol in response

3. **Server-side WebSocket** (`server.rs`)
   - Implement `WebSocketUpgrade::is_upgrade_request()`
   - Implement `WebSocketUpgrade::accept()` (compute accept key, send 101 response)
   - Server-side connection handling (must NOT mask frames)

4. **Performance optimizations**
   - Zero-copy frame parsing where possible
   - Buffer pooling for frames
   - Batch frame writing

**Success Criteria**:
- [ ] `ReconnectingWebSocketTask` implements `TaskIterator` correctly
- [ ] State carries ALL data (Option<Box<...>> pattern)
- [ ] NO LOOPS in `next()` - each call does ONE step
- [ ] Auto-reconnects on connection loss via TaskIterator state machine
- [ ] Exponential backoff works via `ExponentialBackoffDecider`
- [ ] Max retries honored
- [ ] Subprotocol negotiation works correctly
- [ ] Server-side upgrade handling works
- [ ] Server does NOT mask outgoing frames (per RFC 6455)
- [ ] Performance optimizations implemented
- [ ] Full integration with valtron executors
- [ ] Performance benchmarks pass

## Success Criteria

**Phase 1 (Core with TaskIterator)**:
- [ ] `websocket/` module exists and compiles
- [ ] `WebSocketTask` implements `TaskIterator` trait correctly
- [ ] State machine follows valtron pattern (Init → Connecting → Handshake → Open → Closed)
- [ ] State carries ALL data between transitions (Option<Box<...>> pattern)
- [ ] NO LOOPS in `next()` - each call does ONE step
- [ ] `TaskStatus` variants used correctly (Ready, Pending, Delayed, Spawn)
- [ ] `Frame` struct correctly encodes/decodes all frame types
- [ ] Frame encoding handles payload lengths: <126, 126-65535, ≥65536
- [ ] Client masking is correct (clients MUST mask all outgoing frames)
- [ ] `compute_accept_key()` generates correct SHA-1 + base64 hashes
- [ ] Client handshake works with `ws://` URLs (plain TCP)
- [ ] Client handshake works with `wss://` URLs (TLS)
- [ ] `DrivenSendTaskIterator` wrapper works for blocking use cases
- [ ] `WebSocketConnection` (blocking wrapper) sends/receives messages correctly
- [ ] Can send Text and Binary messages
- [ ] Can receive Text and Binary messages
- [ ] Ping/Pong auto-response works (receiver must respond with Pong)
- [ ] Close handshake works correctly (bidirectional)
- [ ] Message fragmentation works (multi-frame messages with FIN flag)
- [ ] Control frames are never fragmented (per RFC 6455)
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`
- [ ] Integration test with public echo server passes

**Phase 2 (Advanced Features)**:
- [ ] `ReconnectingWebSocketTask` implements `TaskIterator` correctly
- [ ] State carries ALL data (Option<Box<...>> pattern)
- [ ] NO LOOPS in `next()` - each call does ONE step
- [ ] Auto-reconnects on connection loss via TaskIterator state machine
- [ ] Exponential backoff works via `ExponentialBackoffDecider`
- [ ] Max retries honored
- [ ] Subprotocol negotiation works correctly
- [ ] Server-side upgrade handling works
- [ ] Server does NOT mask outgoing frames (per RFC 6455)
- [ ] Performance optimizations implemented
- [ ] Full integration with valtron executors
- [ ] Performance benchmarks pass

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

### CRITICAL: TaskIterator with DrivenSendTaskIterator Wrappers

**MANDATORY:** All WebSocket client implementations MUST follow the TaskIterator with DrivenSendTaskIterator design pattern:

1. **TaskIterator: Pure state machine** - NO loops in `next()`, ONE step per call
2. **State carries ALL data** - Use `Option<State>` wrapper, state variants hold `Option<Box<...>>`
3. **Blocking wrappers use DrivenSendTaskIterator** - Execution driving happens in wrapper, NOT in TaskIterator
4. **Data moved between states** - Extract from one variant, move to next

**DO NOT:**
- Use loops inside `next()` in TaskIterator - state machine must be step-wise
- Store state without `Option` wrapper (task must be able to terminate)
- Put execution driving logic in TaskIterator - that's what DrivenSendTaskIterator is for
- Ignore valtron state machine patterns

**DO:**
- Follow `send_request.rs` pattern exactly for TaskIterator
- Wrap task state in `Option<...>`
- Carry data in state variants using `Option<Box<...>>`
- Return `None` from `next()` when task is done
- Use `TaskStatus` variants correctly (Ready, Pending, Delayed, Spawn)
- Use `DrivenSendTaskIterator` for blocking Iterator wrappers
- Use `TaskStatusMapper` to filter/transform TaskStatus values

### Pattern Summary

```
TaskIterator (WebSocketTask)
    ↓ (pure state machine, ONE step per next())
DrivenSendTaskIterator (wraps TaskIterator)
    ↓ (calls run_until_next_state() then task.next() ONCE)
Iterator::next() -> TaskStatus<Ready, Pending, Spawner>
    ↓ (optional: map/filter with TaskStatusMapper)
Ready values only (for blocking Iterator API)
```

### TaskIterator Pattern Reference

Study these existing implementations:
- `wire/simple_http/client/tasks/send_request.rs` - `SendRequestTask` state machine
- `wire/simple_http/client/tasks/request_stream.rs` - `GetHttpRequestStreamTask`
- `wire/simple_http/client/tasks/state.rs` - State machine patterns
- `valtron/executors/task_iters.rs` - `DrivenSendTaskIterator` implementation
- `valtron/executors/drivers.rs` - `ReadyConsumingIter` for Ready-only extraction

### Critical Pre-Checks

Before starting implementation, **MUST**:
1. **Verify dependencies**: `connection` and `public-api` features are complete
2. **Read RFC 6455** - The WebSocket Protocol specification
3. **Explore existing code**:
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
- Use existing `SimpleIncomingRequestBuilder` to build request
- Add required headers: `Upgrade`, `Connection`, `Sec-WebSocket-Key`, `Sec-WebSocket-Version`
- Render request using `Http11::request().http_render()` (RenderHttp trait)
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
- `SimpleIncomingRequestBuilder` - Build HTTP upgrade request
- `SimpleIncomingResponse` - Build HTTP upgrade response
- `HttpResponseReader` - Parse HTTP response
- `Http11::request().http_render()` - Render HTTP request
- `Http11::response().http_render()` - Render HTTP response
- `TaskIterator` - Non-blocking state machine pattern
- `DrivenSendTaskIterator` - Blocking wrapper pattern
- `DnsResolver` - Hostname resolution
- `HttpConnectionPool` - Connection reuse (optional)

**Do NOT re-implement**:
- HTTP request formatting (use `SimpleIncomingRequestBuilder` + `Http11::request().http_render()`)
- HTTP response formatting (use `SimpleIncomingResponse` + `Http11::response().http_render()`)
- HTTP response parsing (use `HttpResponseReader`)
- TLS handshake (use `HttpClientConnection::connect()`)
- DNS resolution (use `DnsResolver`)
- Execution driving (use `DrivenSendTaskIterator`)

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
9. **Using loops in TaskIterator::next()** - use state machine, not loops
10. **Not using Option<State> wrapper** - task must be able to terminate

---
*Created: 2026-02-28*
*Last Updated: 2026-03-03*
*See [ARCHITECTURE.md](./ARCHITECTURE.md) for comprehensive design documentation*
