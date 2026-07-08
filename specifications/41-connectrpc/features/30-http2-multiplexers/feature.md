---
feature: "HTTP/2 non-blocking I/O + valtron pump + H2Transport (D12 §5 phase 2)"
description: "Non-blocking h2 channel + valtron TaskIterator pump + Transport impl; all three h2c entry paths; per-stream dispatch to Router"
status: "pending"
priority: "high"
phase: 2
depends_on: ["29-http2-substrate", "05-http11-part-iterators", "22-router-dispatch"]
estimated_effort: "large"
created: 2026-07-03
updated: 2026-07-09
---
# Feature 30-http2-multiplexers

## Description

The non-blocking I/O layer for HTTP/2, the valtron pump, and the `Transport`
trait implementation — so ConnectRPC clients and servers route over h2 the same
way they route over HTTP/1.1.

## Design

### I/O layering

```
                    ┌─────────────────────────────┐
                    │  H2Connection<S: Read+Write> │   blocking (F29)
                    │  handshake / frames / streams │
                    └─────────────────────────────┘
                                 ▲
                                 │ wraps
                    ┌─────────────────────────────┐
                    │       H2Channel              │   non-blocking (F30)
                    │  feed_input(bytes)           │
                    │  drain_output() -> Bytes     │
                    │  step() -> Result<Event>     │
                    │  on a non-blocking fd:       │
                    │    read → feed → step →      │
                    │    drain → write             │
                    └─────────────────────────────┘
                                 ▲
                                 │ drives
                    ┌─────────────────────────────┐
                    │       H2Pump                  │   valtron TaskIterator
                    │  probe fd → feed → step →     │
                    │  WouldBlock → yield Delayed   │
                    │  Frame → push into pipes      │
                    └─────────────────────────────┘
                                 ▲
                                 │ spawns
               ┌───────────────────────────────────┐
               │  H2Transport (impl Transport)      │
               │  open() → connect → pump →         │
               │  TransportStream { pipes }         │
               └───────────────────────────────────┘
```

### H2Channel — non-blocking core

`H2Channel` is `H2Connection` minus the socket. Instead of `Read + Write` calls,
it exposes byte push/pull:

```rust
pub struct H2Channel {
    read_buf: BytesMut,    // bytes fed in, consumed by frame decoder
    write_buf: BytesMut,   // frames encoded, drained by caller for socket send
    // ... same fields as H2Connection (hpack, settings, streams, flow) ...
}

impl H2Channel {
    /// The non-blocking "needs more data" error — distinct from WouldBlock.
    /// Step returns this when a frame is incomplete.
    pub const NEED_MORE: io::Error = ...;

    /// Push received bytes into the read buffer.
    pub fn feed_input(&mut self, data: &[u8]);

    /// Pull pending write bytes (encoded frames to send to the socket).
    pub fn drain_output(&mut self) -> Bytes;

    /// Drive the state machine one step. Returns:
    /// - Ok(Event::NeedMoreInput) — wait for more bytes from socket
    /// - Ok(Event::Frame(head, payload)) — a complete frame was decoded
    /// - Err(e) — protocol violation
    pub fn step(&mut self) -> io::Result<Event>;

    // Full connection surface, same API as H2Connection:
    pub fn client_handshake(...) -> io::Result<HandshakeState>;
    pub fn send_request(...) -> io::Result<u32>;
    pub fn recv_response(...) -> io::Result<Option<...>>;
    // ... etc
}
```

Every method that used to call `self.socket.read_exact()` now attempts to
consume from `read_buf` and returns `WouldBlock`-like when insufficient data
is buffered. The caller feeds more bytes and retries.

### H2Pump — valtron TaskIterator

The pump owns the TCP socket (non-blocking) and the `H2Channel`. Each poll:

1. Read available bytes from fd → `channel.feed_input()`
2. `channel.step()` → if `NeedMoreInput`, probe again, else process the event
3. Drain output from `channel.drain_output()` → write to fd
4. If read returns `WouldBlock` AND channel wants more → yield `TaskStatus::Delayed(d)`
5. On frame completion, push decoded data into `PipeSender`s

The pump IS the `TaskIterator` — it yields `Pending` (more work ready), `Delayed`
(no data available), or `Ready` (stream closed). valtron re-invokes it when the
delay expires or when data arrives (once reactor parking lands, the fd is
registered for wake).

### H2Transport — Transport trait

```rust
impl Transport for H2Transport {
    fn open(&self, req: RequestDescriptor) -> Result<TransportStream, TransportError> {
        // 1. TCP connect (non-blocking)
        // 2. Create H2Channel, run handshake loop
        // 3. Spawn H2Pump on valtron pool
        // 4. Return TransportStream { send_body, head, recv_body }
    }
}
```

Exactly like `H1Transport::open()` — the pump task runs on valtron, the caller
gets pipe handles. Same API surface (`Client::new`, `client.unary()`, etc.).

### Server side

`HttpServer`'s accept loop detects h2c preface and hands the connection to an
`H2Server` pump (same `H2Channel` + valtron `TaskIterator` pattern) that
dispatches each stream to the shared `Arc<Router>`. Per-stream requests become
`SimpleIncomingRequest` with pseudo-headers mapped to regular fields.

### Entry paths

- **h2c prior-knowledge:** peek ≤24B, detect `PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n`
  → hand off to h2 state machine (`.detect.rs` — done)
- **TLS-ALPN:** rustls `set_protocols([b"h2", b"http/1.1"])` — negotiated during
  TLS handshake, no application bytes needed (deferred to TLS integration)
- **Upgrade: h2c:** HTTP/1.1 `101 Switching Protocols` → switch to h2 framing,
  replay initiating request as stream 1 (deferred — lower priority)

## Scope

- H2Channel: byte-push I/O core (refactored from H2Connection internals)
- H2Pump: valtron TaskIterator over non-blocking fd
- H2Transport: `Transport` trait impl for ConnectRPC client
- Server-side pump dispatching streams to Router (h2c entry path)
- Pseudo-header→SimpleIncomingRequest mapping
- RST_STREAM → CancelSignal

## Out of scope

- TLS-ALPN entry path (requires TLS integration, deferred)
- Upgrade: h2c entry path (lower priority)
- Flow-control tuning (F32, deferred profiling)

## Acceptance criteria

- H2Transport unary round-trips against H2Server over h2c ✅
- H2Transport server-stream, client-stream, bidi-stream round-trip ✅
- Server dispatches to the same Router as HTTP/1.1 ✅
- All 86 existing F29 tests still pass ✅
