---
feature: websocket
description: WebSocket client and server support with frame-based messaging and TLS
status: pending
priority: low
depends_on:
  - connection
  - public-api
estimated_effort: large
created: 2026-01-19
last_updated: 2026-01-24
author: Main Agent
tasks:
  completed: 0
  uncompleted: 17
  total: 17
  completion_percentage: 0
files_required:
  implementation_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/13-implementation-agent-guide.md
      - .agents/rules/11-skills-usage.md
      - .agents/stacks/rust.md
    files:
      - ../requirements.md
      - ./feature.md
  verification_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/08-verification-workflow-complete-guide.md
      - .agents/stacks/rust.md
    files:
      - ../requirements.md
      - ./feature.md
---

# WebSocket Feature

## 🔍 CRITICAL: Retrieval-Led Reasoning Required

**ALL agents implementing this feature MUST use retrieval-led reasoning.**

### Before Starting Implementation

**YOU MUST** (in this order):
1. ✅ **Search the codebase** for similar implementations using Grep/Glob
2. ✅ **Read existing code** in related modules to understand patterns
3. ✅ **Check stack files** (`.agents/stacks/[language].md`) for language-specific conventions
4. ✅ **Read parent specification** (`../requirements.md`) for high-level context
5. ✅ **Read module documentation** for modules this feature touches
6. ✅ **Check dependencies** by reading other feature files referenced in `depends_on`
7. ✅ **Follow discovered patterns** consistently with existing codebase

### FORBIDDEN Approaches

**YOU MUST NOT**:
- ❌ Assume patterns based on typical practices without checking this codebase
- ❌ Implement without searching for similar features first
- ❌ Apply generic solutions without verifying project conventions
- ❌ Guess at naming conventions, file structures, or patterns
- ❌ Use pretraining knowledge without validating against actual project code

### Retrieval Checklist

Before implementing, answer these questions by reading code:
- [ ] What similar features exist in this project? (use Grep to find)
- [ ] What patterns do they follow? (read their implementations)
- [ ] What naming conventions are used? (observed from existing code)
- [ ] How are errors handled in similar code? (check error patterns)
- [ ] What testing patterns exist? (read existing test files)
- [ ] Are there existing helper functions I can reuse? (search thoroughly)

### Enforcement

- Show your retrieval steps in your work report
- Reference specific files/patterns you discovered
- Explain how your implementation matches existing patterns
- "I assumed..." responses will be rejected - only "I found in [file]..." accepted

---

## Overview

Add WebSocket protocol support to the HTTP client, including both client (connect to WebSocket servers) and server (accept WebSocket upgrades) capabilities. This feature implements the WebSocket handshake, frame-based messaging, and integrates with the existing TLS infrastructure.

## Dependencies

This feature depends on:
- `connection` - Uses HttpClientConnection for TCP/TLS
- `public-api` - Uses client infrastructure for WebSocket client

This feature is required by:
- None (end-user feature)

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

## Implementation Details

### File Structure

```
client/
├── websocket/
│   ├── mod.rs       (re-exports)
│   ├── frame.rs     (WebSocketFrame, Opcode)
│   ├── message.rs   (WebSocketMessage)
│   ├── client.rs    (WebSocketClient, handshake)
│   ├── server.rs    (WebSocketUpgrade)
│   ├── connection.rs (WebSocketConnection)
│   └── error.rs     (WebSocketError)
└── ...
```

### Sec-WebSocket-Accept Computation

```rust
fn compute_accept_key(key: &str) -> String {
    use sha1::{Sha1, Digest};
    use base64::encode;

    const MAGIC: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    let combined = format!("{}{}", key, MAGIC);
    let hash = Sha1::digest(combined.as_bytes());
    encode(hash)
}
```

### Frame Masking

```rust
fn apply_mask(data: &mut [u8], mask: [u8; 4]) {
    for (i, byte) in data.iter_mut().enumerate() {
        *byte ^= mask[i % 4];
    }
}
```

## Success Criteria

- [ ] `websocket/` module exists and compiles
- [ ] `WebSocketMessage` enum covers all message types
- [ ] `WebSocketFrame` correctly encodes/decodes frames
- [ ] Client handshake works with ws:// URLs
- [ ] Client handshake works with wss:// URLs
- [ ] `compute_accept_key()` generates correct hashes
- [ ] Client masking is correct (must mask)
- [ ] Server accepts upgrade requests correctly
- [ ] Server does not mask outgoing frames
- [ ] Ping/Pong auto-response works
- [ ] Close handshake works correctly
- [ ] Message fragmentation handling works
- [ ] Subprotocol negotiation works
- [ ] `MessageIterator` works correctly
- [ ] TLS (wss://) works with existing TLS infrastructure
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core -- websocket
cargo build --package foundation_core
cargo build --package foundation_core --features ssl-rustls
```

## Notes for Agents

### Before Starting
- **MUST VERIFY** connection and public-api features are complete
- **MUST READ** RFC 6455 (The WebSocket Protocol)
- **MUST READ** existing connection handling for TCP/TLS patterns

### Implementation Guidelines
- Clients MUST mask, servers MUST NOT mask
- Auto-respond to Ping with Pong
- Handle message fragmentation (fin=false)
- Close handshake: send Close, wait for Close response
- Use existing TLS infrastructure for wss://

### Security Considerations
- Validate UTF-8 for text messages
- Limit frame/message sizes to prevent DoS
- Handle close codes correctly (1000=normal, 1001=going away, etc.)
- Don't expose internal errors in close reasons

### Integration with TaskIterator
Consider implementing TaskIterator for non-blocking WebSocket I/O:

```rust
impl TaskIterator for WebSocketConnection {
    type Ready = WebSocketMessage;
    type Pending = WebSocketState;
    type Spawner = NoAction;
}
```

---
*Created: 2026-01-19*
*Last Updated: 2026-01-19*
