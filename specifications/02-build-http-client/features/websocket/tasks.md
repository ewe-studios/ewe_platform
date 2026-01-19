---
feature: websocket
completed: 0
uncompleted: 20
last_updated: 2026-01-19
tools:
  - Rust
  - cargo
---

# WebSocket - Tasks

## Task List

### Module Setup
- [ ] Create `client/websocket/mod.rs` - Module entry with re-exports
- [ ] Create `client/websocket/frame.rs` - Frame encoding/decoding
- [ ] Create `client/websocket/message.rs` - Message types
- [ ] Create `client/websocket/client.rs` - Client handshake
- [ ] Create `client/websocket/server.rs` - Server upgrade
- [ ] Create `client/websocket/connection.rs` - Connection handling
- [ ] Create `client/websocket/error.rs` - Error types
- [ ] Add sha1 dependency for accept key computation

### Frame Protocol
- [ ] Define `Opcode` enum (Continuation, Text, Binary, Close, Ping, Pong)
- [ ] Define `WebSocketFrame` struct
- [ ] Implement `WebSocketFrame::encode()` for frame serialization
- [ ] Implement `WebSocketFrame::decode()` for frame parsing
- [ ] Implement masking/unmasking logic

### Message Types
- [ ] Define `WebSocketMessage` enum
- [ ] Implement conversion from Frame to Message
- [ ] Implement conversion from Message to Frame
- [ ] Handle message fragmentation (continuation frames)

### Client Handshake
- [ ] Implement `generate_websocket_key()` (random 16 bytes, base64)
- [ ] Implement `compute_accept_key()` (SHA1 + base64)
- [ ] Implement HTTP upgrade request generation
- [ ] Implement upgrade response parsing
- [ ] Implement accept key verification

### Server Upgrade
- [ ] Implement `WebSocketUpgrade::is_upgrade_request()`
- [ ] Implement `WebSocketUpgrade::accept()`
- [ ] Implement upgrade response generation
- [ ] Implement subprotocol negotiation

### WebSocket Connection
- [ ] Define `WebSocketConnection` struct
- [ ] Define `Role` enum (Client, Server)
- [ ] Implement `send()` for all message types
- [ ] Implement `receive()` with frame parsing
- [ ] Implement `messages()` iterator
- [ ] Implement `pong()` response
- [ ] Implement `close()` handshake
- [ ] Implement auto-pong for incoming Ping

### Client Integration
- [ ] Add `websocket()` method to SimpleHttpClient
- [ ] Implement `WebSocketBuilder` with options
- [ ] Add `subprotocol()` and `subprotocols()` methods
- [ ] Add TLS support for wss:// URLs

### Error Handling
- [ ] Define `WebSocketError` enum with all variants
- [ ] Implement std::error::Error for WebSocketError
- [ ] Implement From conversions for error types

## Implementation Order

1. **error.rs** - Error types first (dependency for others)
2. **frame.rs** - Low-level frame protocol
3. **message.rs** - High-level message types
4. **client.rs** - Client handshake logic
5. **server.rs** - Server upgrade logic
6. **connection.rs** - Unified connection handling
7. **mod.rs** - Module organization and re-exports
8. **Integration** - Add to SimpleHttpClient

## Notes

### Frame Format (RFC 6455)
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

### Accept Key Computation
```rust
fn compute_accept_key(key: &str) -> String {
    const MAGIC: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let hash = sha1::Sha1::digest(format!("{}{}", key, MAGIC));
    base64::encode(hash)
}
```

### Masking Pattern
```rust
fn apply_mask(data: &mut [u8], mask: [u8; 4]) {
    for (i, byte) in data.iter_mut().enumerate() {
        *byte ^= mask[i % 4];
    }
}
```

### Close Codes
- 1000: Normal closure
- 1001: Going away
- 1002: Protocol error
- 1003: Unsupported data type
- 1007: Invalid frame payload data
- 1008: Policy violation
- 1009: Message too big
- 1011: Unexpected condition

---
*Last Updated: 2026-01-19*
