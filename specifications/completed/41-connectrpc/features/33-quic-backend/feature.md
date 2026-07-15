---
feature: "QUIC backend: quinn-proto + valtron driver + netcap variants (D01 T14, D12 §9)"
description: "Sans-IO quinn-proto as a Cargo dep, our UDP plumbing + event-loop task, netcap Quic variants"
status: "complete"
priority: "medium"
phase: 3
depends_on: ["10-reactor-parking"]
quinn-source: /home/darkvoid/Boxxed/@formulas/src.rust/src.Quinn/
estimated_effort: "large"
created: 2026-07-03
---
# Feature 33-quic-backend: QUIC backend: quinn-proto + valtron driver + netcap variants (D01 T14, D12 §9)

## Description

Phase-3 groundwork: QUIC via the sans-IO quinn-proto state machine driven by valtron tasks, exposed through our progress-returning trait set and netcap's centralized listener.

## Normative sources (single source of truth — read before writing code)

- decisions/01-transport-and-runtime.md — T14 (quinn-proto decision), §QUIC trait abstraction (normative traits)
- decisions/12-foundation-enablement.md — §9 (netcap Connection/Listener/ConfigListenAddr Quic variants)

## Scope

- quinn-proto (feature-gated Cargo dep, no vendoring) + valtron-driven event loop + UDP socket plumbing (optional quinn-udp GSO/GRO)
- Our QuicConnection/QuicSendStream/QuicRecvStream/QuicBidiStream traits (progress-returning, TaskStatus-driven, no Poll/Waker) implemented over quinn-proto

## Moved to feature 35 (2026-07-10)

- ~~netcap Listener/Connection Quic variants; HttpServer accepts via netcap Listener (D12 §9 items 1–2)~~

D12 §9 pairs those two items with the server's `ConnectionHandler` branch, and item 3 says
`Connection` is the byte stream for "individual QUIC streams" — which only has a meaning once
HTTP/3 is mapping streams onto the Router. Nothing in F34 (framing + QPACK over the trait set)
needs them. They now live in feature 35, whose job is exactly the server-side seam. This is a
re-slice, not a drop.

## Out of scope

- iroh (deferred — D01: do not build (b) speculatively)

## Acceptance criteria

- QUIC connect/accept + bidi stream echo over the trait set, parking on the reactor (no busy-poll)

## Reopened and re-closed 2026-07-10

Marked complete with one of three scope items delivered and no tests. F34's whole job is
"HTTP/3 framing **over the `QuicConnection` traits**", so it cannot start on this.

**What exists.** `quic/quinn_driver.rs` — 222 lines. A client-only `QuicDriver: TaskIterator`
that binds a UDP socket, drives a `quinn_proto::Endpoint`/`Connection`, and emits a
`QuicEvent` enum (`Connected` / `StreamOpened` / `StreamData` / `ConnectionClosed`). It parks
via `Depends(fd_readiness)` when the caller injects one. It compiles.

**What does not exist.**

1. **The normative trait set.** Scope item 2 and D01 §"Our QUIC trait abstraction" specify
   `QuicConnection` / `QuicSendStream` / `QuicRecvStream` / `QuicBidiStream`, with
   `QuicConnError` / `QuicStreamError`. `grep -rn 'QuicConnection' --include=*.rs backends/`
   matches **nothing**. The driver's `QuicEvent` enum is not that abstraction: it is a
   one-way event feed, not the progress-returning `accept_recv` / `accept_bidi` /
   `open_bidi` / `open_send` / `send` / `read` / `finish` / `reset` / `stop_sending` surface
   the rest of the design is written against.
2. **No server side.** `QuicDriver::connect` is the only constructor; the endpoint is built
   with `server_config: None`. There is no accept path, so "connect/accept" is unreachable.
3. **No netcap Quic variants** (scope item 3, D12 §9 items 1–2). `grep -rn 'Quic' netcap/`
   matches nothing.
4. **No tests.** Zero QUIC tests anywhere. The acceptance criterion — "QUIC connect/accept +
   bidi stream echo over the trait set, parking on the reactor (no busy-poll)" — could not
   have passed, because none of its three nouns exist.

### The design constraint the traits have to solve

`quinn_proto::Connection` hands out streams as **short-lived borrows**:
`streams(&mut self) -> Streams<'_>`, `recv_stream(&mut self, id) -> RecvStream<'_>`,
`send_stream(&mut self, id) -> SendStream<'_>`. So a `QuicRecvStream` implementation cannot
hold a `&mut Connection`. Streams must hold a shared handle to the connection state plus a
`StreamId`, and re-borrow on each call. The driver task and the stream handles therefore share
one `Arc<Mutex<ConnState>>`: the driver pumps datagrams, timers and `poll_transmit`; the trait
methods lock, act, and return one `Stream<..>` value per call.

### What shipped

- **`quic/traits.rs`** — the normative trait set, verbatim from D01: `QuicConnection`,
  `QuicSendStream`, `QuicRecvStream`, `QuicBidiStream`, `QuicConnError`, `QuicStreamError`.
  Progress-returning, one `Stream<..>` value per call, no `Poll`/`Context`/`Waker`. Not
  `dyn`-compatible, by design (`send(&mut impl Buf)`, `split() -> (impl .., impl ..)`).
- **`quic/mod.rs`** — a backend-neutral `StreamId` with the RFC 9000 §2.1 accessors HTTP/3
  needs (initiator, directionality), so the traits never leak `quinn_proto`. Conversion into
  `quinn_proto::StreamId` is fallible rather than `expect`-ing past the 62-bit varint bound.
- **`quic/state.rs`** — the design constraint, solved. `quinn_proto::Connection` hands out
  streams as short-lived borrows, so a stream handle cannot own a `&mut Connection`. Driver
  and stream handles share one `Arc<Mutex<ConnState>>`; each call locks, re-borrows, acts,
  and returns. quinn-proto is sans-IO, so nothing under that lock can block.
- **`quic/quinn_impl.rs`** — the trait impls. The error mapping is the load-bearing part:
  `WriteError::Blocked` / `ReadError::Blocked` become `Stream::Pending(())`, not errors. That
  is precisely why h3's `poll_ready` is unnecessary here — back-pressure *is* the return value.
- **`quic/quinn_driver.rs`** — rewritten. The endpoint moved out of per-connection state,
  because one endpoint routes datagrams for many connections and answers stateless retries.
  Adds the server accept path (`QuicDriver::server` + `take_accepted`), `handle_timeout`
  (without which loss detection never fires and a lossy link silently stalls), and
  `send_to(transmit.destination)` rather than assuming a connected socket.

### Bugs found in the code that was marked complete

- `QuicEvent::StreamData { id }` reported `StreamId::index()` — the *index*, not the stream
  id. That discards the two low bits that encode initiator and directionality, which is
  exactly what HTTP/3 dispatches on.
- No `handle_timeout` call anywhere: retransmissions, idle timeout and keep-alive never ran.
- FIN was never reported; `fin` was hardcoded `false`.
- The `quic` feature declared only `dep:quinn-proto`, yet the module `use`d `rustls`. It
  built solely because the *default* feature set happened to enable rustls;
  `--no-default-features --features quic` did not compile. The feature now declares what it
  uses.

### An API defect the tests found

`QuicBidiStream: QuicSendStream + QuicRecvStream` inherits `id()` from **both** supertraits,
so `stream.id()` on a concrete bidi stream is ambiguous (E0034). The normative traits are
unchanged; `QuinnBidiStream` gains an inherent `id()`, which wins resolution. Generic code
still disambiguates with `QuicSendStream::id(&s)`.

### Acceptance

`tests/quic/mod.rs` — 7 tests, all driving the **trait set**, never `quinn_proto` directly:

- client connects, server accepts, and the connection is usable (a stream opens)
- bidi echo: open → write → accept → read → echo → finish → end-of-stream latches
- `split()` yields both halves, each keeping the stream id
- unidirectional open/accept — the path HTTP/3's control and QPACK streams take
- a peer `reset(42)` surfaces as `Terminated { code: 42 }`, not as a clean FIN
- an idle driver with an injected readiness signal parks via `Depends` (**no busy-poll**)
- an idle driver without one falls back to `Delayed` rather than spinning

Certificates are `tests/fixtures/quic_{ca,cert,key}.pem`: a CA plus a leaf signed by it. A
self-signed CA presented as an end-entity certificate is rejected by rustls
(`CaUsedAsEndEntity`), which is what the first version of the fixture hit.
