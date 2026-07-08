---
feature: "HTTP/2 substrate: frame codec, HPACK, stream FSM, blocking-I/O connection (D12 §5)"
description: "Direction-neutral h2 building blocks, tokio-free, replicated from h2 as reference"
status: "complete"
priority: "high"
phase: 2
depends_on: ["04-incremental-decoder"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 29-http2-substrate: HTTP/2 substrate

## Description

Phase-2 groundwork: the owned, tokio-free HTTP/2 binary layer with h2 as reference.

## I/O model — two equal paths

The substrate is I/O-agnostic. `H2Connection<S: Read + Write>` works over any byte
stream. Two wrappers provide concrete I/O:

| Path       | Adapter   | I/O model                     | Use case                        |
|------------|-----------|-------------------------------|----------------------------------|
| **blocking** | direct    | `S: Read + Write` (e.g. `TcpStream`) | OS-thread pumps, tests, CLI tools |
| **non-blocking** | `H2Channel` | byte push/pull via `BytesMut` + `IncrementalDecoder` | valtron `TaskIterator` pumps, `H2Transport` |

**Blocking path:** `H2Connection::new(socket, is_server)` — calls `read_exact`/`write_all`
directly. The caller runs on its own OS thread (e.g. `HttpServer` accept loop).

**Non-blocking path (Feature 30):** `H2Channel` provides the core connection state machine
with push-I/O: `feed_input(bytes)` pushes received bytes into the read buffer; `drain_output()`
pulls pending write bytes. On a non-blocking fd, the valtron pump probes the fd, feeds any
available bytes to the channel, drains output back to the fd, and yields `Delayed` on
`WouldBlock`. This is the `H2Transport` path.

The channel exposes the full connection surface — `client_handshake()`, `server_handshake()`,
`send_request()`, `recv_response()`, `send_headers_response()`, `send_data_frame()`,
`recv_data_frame()` — returning `io::Result` and `NeedMore`/`WouldBlock` when I/O is needed.

## Scope

- `frame/` — 9-byte header codec, all 10 frame types
- `hpack/` — static/dynamic table, Huffman, RFC 7541 vectors
- `flow_control.rs` — per-stream + connection window arithmetic (property-tested)
- `stream.rs` — FSM: idle → reserved → open → half-closed → closed (exhaustive matrix tested)
- `settings.rs` — SETTINGS store with RFC 7540 defaults, validation, change tracking
- `connection.rs` — `H2Connection<S: Read+Write>`: handshake, frame dispatch, send/recv
- `server.rs` — `H2Server` wrapping `H2Connection` + `H2Serve` trait
- `client.rs` — `H2Client` wrapping `H2Connection` with request/response API
- `detect.rs` — h2c preface detection (peek ≤24B), `DetectedProtocol` enum

## Module layout

```
foundation_netio/src/http2/
├── mod.rs
├── flow_control.rs
├── frame/
│   └── mod.rs
├── hpack/
│   ├── mod.rs
│   ├── huffman.rs
│   └── huffman/
│       └── huffman_table.rs  (generated from RFC 7541 Appendix B)
├── settings.rs
├── stream.rs
├── connection.rs        # H2Connection<S: Read+Write> — blocking I/O
├── channel.rs           # H2Channel — non-blocking push/pull I/O (Feature 30)
├── server.rs
├── client.rs
└── detect.rs
```

## Acceptance criteria

- HPACK RFC 7541 vectors pass ✅ (23 tests)
- Frame codec round-trips all 10 frame types ✅ (12 tests)
- Flow-control arithmetic property-tested ✅ (26 tests)
- Stream state machine: all 7 states, exhaustive matrix ✅ (12 tests)
- Settings validation + round-trip ✅ (6 tests)
- Detection: h2c preface vs HTTP/1.1 vs NeedMore ✅ (4 tests)
- Connection: preface, handshake, request/response, StreamId ✅ (3 tests)
- **Total: 86 tests, zero warnings**
