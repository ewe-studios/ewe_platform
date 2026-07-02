# Decision 13: WebSocket Transport (Bidi over HTTP/1.1)

> **Status: deferred — scheduled last.** This is the final transport to implement, after
> Phase 1 (HTTP/1.1), Phase 2 (HTTP/2), and the HTTP/3 / iroh work in Decision 01. It is
> additive: nothing else depends on it, and it does not change the default HTTP/1.1 behavior.

## Context

Decision 01 / S3 / OQ#4 reject **bidirectional streaming over plain HTTP/1.1** with `505`,
because a raw HTTP/1.1 request/response body is **half-duplex**: the client must finish
sending its body before the server may stream its response. Full-duplex bidi there is
genuinely impossible without changing the framing.

A **WebSocket `Upgrade`** (RFC 6455) sidesteps this. The connection *starts* as an HTTP/1.1
request with `Upgrade: websocket` + `Connection: Upgrade` + `Sec-WebSocket-Key`; the server
replies `101 Switching Protocols`, and from that point the same TCP connection is a
**full-duplex, message-framed channel**. Both peers may send frames at any time. That is
exactly the shape of bidirectional RPC.

Crucially, **a WebSocket connection is our transport seam** (Decision 11): an inbound message
stream and an outbound `ConcurrentQueue`. So this slots in as just another `Transport`
implementation — it validates the spec's central thesis that the Connect/gRPC protocols are
abstracted from the transport and plug-and-play on top of any full-duplex carrier.

We already own the WebSocket layer in `foundation_netio/src/websocket/`. A detailed read
(2026) shows it is **strong but not turnkey** for our needs — what exists, and what must be
enhanced, is recorded precisely below so the feature work is scoped honestly.

**Ready and RFC-correct (reuse as-is):**
- `shared::frame` — `WebSocketFrame`, `Opcode`, encode/decode, masking, extended lengths,
  `validate`, `to_message`.
- `shared::assembler` — `MessageAssembler`: full fragmentation with size limits and
  *incremental* UTF-8 validation across fragments; tolerates interleaved control frames.
- `shared::handshake` — `compute_accept_key`, `generate_websocket_key` (wasm-clean RNG via
  `foundation_compact`, not `getrandom`), `build_upgrade_request`, `validate_upgrade_response`.
- `native::task::WebSocketTask` (**client**) — a complete progress-driven `TaskIterator`:
  drains an outbound `delivery_queue`, auto-Pongs, runs the assembler, handles Close,
  zero-copy `decode_with_buffer`. `MessageDelivery::queue() -> &Arc<ConcurrentQueue<…>>` **is
  already an outbound seam queue**. `ReconnectingWebSocketTask` forwards the full `TaskStatus`
  set incl. `Depends`/`Wait`.
- `native::server::WebSocketUpgrade` — `is_upgrade_request`/`extract_key`/
  `extract_subprotocols`/`accept` (and `accept` **echoes the negotiated
  `Sec-WebSocket-Protocol`**).

**Gaps that block a robust RPC server transport (enhanced by this decision):**

1. **Read model is timeout-poll, not reactor-parking.** `WebSocketTask` sets a read timeout,
   does a blocking `decode_with_buffer`, and on `WouldBlock`/`TimedOut` returns
   `TaskStatus::Delayed(..)` — it never emits `Depends`/`QueueReadiness`. So a worker is tied
   up for up to `read_timeout` per poll and the stack does **not** use Decision 00. *(Works
   today; we enhance it to `Depends` where it pays off — see E2.)*
2. **The frame decoder cannot resume a partial frame.** `decode`/`decode_with_buffer` tolerate
   `WouldBlock`/`TimedOut` only on the **first header byte**; once consumed, any such error is
   converted to `ProtocolError("stream corrupted")`. It assumes the whole frame arrives within
   one read-timeout window (kernel-buffered). On a *true* non-blocking fd a large frame splits
   across reads → false "corruption". A **resumable/buffered decoder** is required before WS
   can ride the Decision 00 reactor path (E1).
3. **Server side is blocking convenience only.** `WebSocketServerConnection::recv()` is
   `recv_frame()? .to_message()` — **no assembler** (a `Continuation` frame errors by design in
   `to_message`, so multi-frame messages fail; the "handles frame assembly" doc comment is
   wrong), **no auto-Pong on read**, **no delivery-queue seam**, and only partial Close
   handling. There is **no progress-driven server task** mirroring `WebSocketTask`. (E3.)

So this decision is **wiring + three targeted enhancements** (E1–E3), not a from-scratch
implementation. We own the code; we enhance the existing types and add a robust server task.

## Decision

**Add `WebSocketTransport` as an additional, opt-in transport that carries Connect-style
streaming (including full-duplex bidi) over a WebSocket connection — implemented last.**

### Wire mapping

- **Handshake / routing.** The upgrade request's path selects the procedure (same routing as
  Decision 08). The server performs the `101` upgrade via the existing
  `foundation_http` upgrade path + `foundation_netio` handshake helpers. Subprotocol
  negotiation (`Sec-WebSocket-Protocol: ewe.connectrpc.batch.v1, ewe.connectrpc.v1`) picks
  the framing mode and versions the protocol; call metadata (codec/compression/timeout)
  rides the upgrade URL's Connect GET query params (`?connect=v1&encoding=…`) since
  browsers cannot set upgrade headers — full rules in OQ#13.3 (resolved).
- **Framing.** Each RPC message is a Connect **enveloped frame** (Decision 05) carried in a
  **binary** `WebSocketMessage`. We reuse the existing envelope reader/writer — the WebSocket
  layer only supplies message boundaries; we do **not** invent a new envelope.
  - **Batch framing is first-class (OQ#13.1, resolved).** We own this transport, so a WS
    binary message carries a **batch header + N complete envelopes**, mirroring the envelope
    header shape (Decision 05):

    ```
    ┌────────────┬──────────────┬──────────────┬───┬──────────────┐
    │ u8 bflags  │ u32 count=N  │ envelope 1   │ … │ envelope N   │
    └────────────┴──────────────┴──────────────┴───┴──────────────┘
    ```

    - `count ≥ 1`; empty batches are a protocol error. An envelope MUST NOT span WS
      messages (WS-level fragmentation/`MessageAssembler` already handles large messages
      transparently below this layer). After reading `count` envelopes, leftover bytes —
      or running short — is a protocol error → Close + RPC error.
    - `bflags` is reserved (0) for future batch-level semantics (e.g. whole-batch
      compression); receivers MUST reject unknown flags.
    - **Batching is opportunistic, never timed:** a batch is whatever is already queued in
      the Decision 11 response/request pipe at flush time (vectored-write style). No
      Nagle-style timer, no held-back messages — Decision 11's flush-per-frame latency
      contract is preserved; header amortization comes free on bursty producers. Batch
      size is naturally bounded by the pipe depth (4) × per-message caps; assembler size
      limits apply to the whole WS message.
    - **Interop mode:** where a peer requires plain framing, the subprotocol negotiation
      (OQ#13.3) selects **1:1 mode** — one bare envelope per WS binary message, no batch
      header. Batch mode is used only when both ends negotiate our subprotocol token.
- **Direction.** On the **client**, outbound goes through `MessageDelivery` / its
  `ConcurrentQueue` and inbound arrives from the `WebSocketTask` stream. On the **server**,
  the `WebSocketServerTask` (E3) exposes the `inbound`/`outbound` `ConcurrentQueue` pair
  directly. Either way this is the Decision 11 `MessageSource` / `MessageSink` pair with a
  WebSocket-shaped backing — no new seam concept. (The blocking `recv()`/`messages()` APIs are
  *not* the RPC path.)
- **Trailers / end-of-stream.** Connect streaming end-of-stream is an `EndStreamResponse`
  envelope (Decision 05), carried as a final binary message; the WebSocket `Close` frame is
  the transport-level teardown, mapped to/from cancellation (Decision 04, Option B).
- **Control frames.** `Ping`/`Pong` are handled by the WebSocket layer for keepalive and are
  **not** surfaced to handlers. `Close` (with code/reason) maps to stream completion or
  cancellation. Auto-Pong, the delivery-queue seam, and Close-handshake completion are
  **config-gated** on the server (see E3 / `WsServerConfig`).

### Foundation enhancements (E1–E3) — designed here, built as features

These upgrade the existing `foundation_netio` WebSocket layer; we own it and enhance in place
rather than vendoring or forking.

**E1 — Resumable frame decoder (`WebSocketFrameDecoder`).**
This is the **WebSocket implementation of the generalized `IncrementalDecoder` primitive**
defined in **Decision 12 §11** (the same partial-read seam used by the `http2/` frame codec,
HTTP/3 framing, and the Connect envelope reader). It replaces the one-shot
`decode`/`decode_with_buffer` assumption ("whole frame within one read-timeout window") with a
**stateful decoder that holds partial-frame progress across polls**:

```rust
pub enum FrameStep { Pending, Complete(WebSocketFrame) }

pub struct WebSocketFrameDecoder {
    // phase: reading header / extended-len / mask-key / payload(remaining)
    // carries a growable buffer of bytes received so far for the in-flight frame
}
impl WebSocketFrameDecoder {
    /// Feed whatever bytes are currently available (may be empty). Never errors on a
    /// short read — returns `Pending` and keeps state. Errors only on protocol violations.
    pub fn step(&mut self, src: &mut impl Read) -> Result<FrameStep, WebSocketError>;
}
```

- On a non-blocking fd, `WouldBlock` mid-frame is **normal**: the decoder saves what it has
  and returns `Pending` (no "stream corrupted"). When more bytes arrive it resumes.
- Backwards compatible: the existing blocking `decode` becomes a thin
  `loop { step }`-until-`Complete` wrapper for callers that want the simple API.
- Enforces the same size limits and control-frame rules as today.

**E2 — `Depends` read model via the existing native reactor (opt-in, enhances E1).**
With a resumable decoder, both client and server tasks can stop timeout-polling and **park**:
when `step` returns `Pending` because the socket has no bytes, the task parks on socket
readiness and is woken when readable. The reactor and the bridge **already exist** in
`foundation_nativeapis` — `native::poll` (epoll/kqueue) + `native::fd::RegisteredFd<T:
AsRawFd>` / `FdRegistration`, which implement `EventReadiness`. So the task simply holds an
`Arc<RegisteredFd<…>>` and returns `TaskStatus::Depends(registered_fd)`; no new
`ReadinessSource` abstraction is required (Decision 00 reconciliation).
- **Prerequisite:** expose `AsRawFd` on `netio`'s `RawStream`/`Connection` (**Decision 12
  §12** — delegates to the inner `TcpStream`; for TLS, the underlying socket fd) so the upper
  layer can register it. The unused optional `foundation_netio` dep was removed from
  `foundation_nativeapis`, so the reactor is reachable either via `netio → nativeapis` or by
  wiring at the ConnectRPC crate.
- **Backend:** epoll/kqueue today; **io_uring on Linux** for high connection counts via
  **[Decision 14](14-io-uring-reactor-backend.md)** — same `EventReadiness` seam, same task
  code.
- Gated by availability: if no reactor is reachable (e.g. wasm), the task falls back to the
  **existing timeout-poll + `Delayed`** behavior — no regression. This is the "enhance with
  `Depends` if it makes sense" path, tied to **Decision 00**.

**E3 — Robust server task (`WebSocketServerTask`) + `WsServerConfig`.**
Add a progress-driven server-side `TaskIterator` (mirroring `WebSocketTask`'s `Open` state,
but with **server** semantics: don't mask outgoing, reject unmasked client data frames — that
check already exists in `recv_frame`):

```rust
pub struct WsServerConfig {
    pub auto_pong: bool,            // default true — answer client Ping with Pong
    pub max_message_size: usize,    // assembler limit
    pub graceful_close: bool,       // default true — complete the Close handshake
    pub read_model: ReadModel,      // Depends (if reactor present) | TimeoutPoll
    pub inbound: Arc<ConcurrentQueue<WebSocketMessage>>,   // seam in
    pub outbound: Arc<ConcurrentQueue<WebSocketMessage>>,  // seam out
}
```

- Uses `MessageAssembler` so multi-frame messages assemble (fixes the server gap).
- Drains `outbound` to send, pushes assembled inbound messages to `inbound` — the dual
  `ConcurrentQueue` pair *is* the Decision 11 seam on the server side.
- Auto-Pong, delivery-queue seam, and full Close handshake are **enabled per
  `WsServerConfig`** (you asked for config-gating). The existing blocking
  `WebSocketServerConnection` stays as the simple convenience API; the new task is the robust
  RPC path. We may also retrofit `WebSocketServerConnection::recv` to use the assembler for
  correctness even in the blocking API.

### What it enables

- **Full-duplex bidi on HTTP/1.1 deployments** (browsers, HTTP/1.1-only proxies) without
  HTTP/2 — the one capability the `505` denies on plain HTTP/1.1.
- A second validation point for the transport seam: the same handlers, codecs, interceptors,
  and router run unchanged; only the `Transport` impl differs.

### Scope boundary — non-standard

This is **not** part of the official Connect or gRPC-Web wire specs (neither defines a
WebSocket mapping; the `improbable-eng` grpc-web-over-WS scheme is its own non-standard
protocol and is **not** adopted here). Consequences:

- WebSocket bidi **interoperates only when both peers are our stack** (our generated client +
  our server). It is **not** a way to talk to upstream connect-go / connect-web clients.
- It is **additive**: the default for standard HTTP/1.1 remains the `505` rejection of bidi
  (Decision 01 / S3). WebSocket is selected explicitly (distinct transport / endpoint), never
  as a silent fallback.
- Codegen (Decision 10) is unaffected: handler and client signatures are identical; the
  transport is chosen at wiring time.

## Consequences

- bidi becomes available on HTTP/1.1-only environments, at the cost of a non-standard wire —
  acceptable because it is opt-in and our-stack-to-our-stack.
- Reuses `foundation_netio` WebSocket framing and `foundation_http` upgrade; no new external
  dependency.
- Sits cleanly behind the Decision 11 seam, so it does not perturb the protocol/codec/handler
  layers.
- **E1 (resumable decoder) is generalized, not WS-private:** it is the WebSocket impl of the
  shared `IncrementalDecoder` primitive (Decision 12 §11), reused by `http2/`, HTTP/3, and the
  Connect envelope reader — one tested partial-read seam instead of per-protocol handlers.
- **E2 ties WS to the native reactor:** true parking on native uses `foundation_nativeapis`'s
  reactor via `RegisteredFd: EventReadiness` (Decisions 00/14) once `RawStream: AsRawFd`
  (Decision 12 §12) is wired; until then WS uses the existing timeout-poll fallback with no
  regression.
- **Sequencing:** implemented **last** among transports; nothing in Phases 1–3 depends on it.
  Within this decision, order is **E1 → E3 → (E2 when Decision 00 reactor lands) → 13-F1**.

## Open Questions

- **OQ#13.1 — resolved: batch framing is first-class.** A WS binary message is
  `u8 bflags + u32 count + N complete envelopes` (header mirrors the Decision 05 envelope
  shape; `count ≥ 1`; spanning forbidden; trailing/short bytes = protocol error; unknown
  `bflags` rejected). Batching is **opportunistic only** — drain what's already queued in
  the Decision 11 pipes at flush time, never a timer — so flush-per-frame latency is
  preserved. Plain **1:1 mode** (bare envelope per message, no batch header) exists for
  interop and is selected via subprotocol negotiation (OQ#13.3). See §Framing for the
  normative rules.
- **OQ#13.2 — resolved: all four RPC kinds, one call per connection.**
  - **All kinds** (unary, server-stream, client-stream, bidi) are valid over WS — capability
    matching reports `Duplex::Full` + trailer support, no artificial kind restrictions. The
    seam doesn't branch on kind (unary is a 1-message stream), so supporting all four costs
    zero transport code while restricting would *require* extra capability-matrix code. In
    browser/WASM, WS is the **only** client-stream/bidi path (Fetch can't stream request
    bodies, Decision 11), and a WS-only client can still call any procedure.
  - **One RPC per WS connection:** the upgrade path selects the procedure;
    `EndStreamResponse` + `Close` tears down. No call re-arm / sequential-reuse rules —
    rejected as a mini-multiplexing protocol we'd have to spec, test, and interop forever.
  - **Cost, stated honestly:** unary over WS pays an upgrade round-trip per call — worse
    than pooled h1 keep-alive (Decision 07). Docs mark HTTP as the preferred
    unary/server-stream path; WS unary exists for uniformity, not as the default.
- **OQ#13.3 — resolved: versioned, domain-scoped token family + Connect GET query params
  for call metadata.**
  - **Tokens:** `ewe.connectrpc.v1` (1:1 framing) and `ewe.connectrpc.batch.v1` (batch
    framing, OQ#13.1) — the framing mode is settled at the handshake, never byte-sniffed;
    `v1` gives a compatible evolution path (k8s `v4.channel.k8s.io` pattern); the `ewe.`
    scope avoids squatting the bare `connectrpc` name in the flat WS subprotocol namespace.
  - **Negotiation (normative):** client offers a preference-ordered list
    (`Sec-WebSocket-Protocol: ewe.connectrpc.batch.v1, ewe.connectrpc.v1`); the server
    picks the **first it supports** and echoes exactly one in the `101`. No recognized
    token offered → **refuse the upgrade (400)**; never accept with a missing/unknown
    subprotocol. Client side: missing echo, or an echo not in the offered list → protocol
    error before any message is sent. No silent fallback into ambiguous framing.
  - **Call metadata (codec / compression / timeout):** browsers cannot set headers on
    `new WebSocket()` — the URL and subprotocol list are the only client-controlled
    channels. So metadata rides the **upgrade URL query params, reusing Connect's official
    GET-protocol vocabulary**: `?connect=v1&encoding=proto&compression=gzip[&timeout_ms=…]`.
    Native clients MAY instead send the normal Connect headers on the upgrade request; if
    both are present and disagree → refuse the upgrade (no silent precedence). Failures
    (unknown codec, unsupported compression) are rejected at the HTTP layer **before** the
    `101`, where status codes and error bodies exist. The first-message metadata-envelope
    pattern (improbable-eng / graphql-ws) was rejected: it invents a WS-only metadata
    encoding, moves failures past the upgrade, and adds an awaiting-metadata state to every
    connection bring-up.
- **OQ#13.4 — resolved: retrofit `recv` with the assembler (correctness fix).** Verified in
  code: today `recv()` is `recv_frame()?.to_message()` (`websocket/native/server.rs:325`) —
  single-frame only, so any RFC-6455 peer that fragments breaks it (fragmented Text can even
  fail UTF-8 validation on a partial payload); the "handles frame assembly" doc comment is
  currently false. `MessageAssembler` (`websocket/shared/assembler.rs`) already has the
  needed API (`process_frame` → `Option<message>`, `max_message_size` cap, `is_assembling`).
  The retrofit is a small loop: read frames; control frames (Ping/Close) return immediately
  per RFC 6455; data/continuation frames feed the assembler (a struct field, so an
  interleaved control frame returns now and the next `recv()` resumes the in-progress
  assembly); a completed message returns; the size cap gives the blocking API the same
  memory bound as the task path. `messages()` wraps `recv` and gets the fix for free.
  Boundaries unchanged: `recv` stays blocking-convenience-only — no auto-Pong, no delivery
  queue; the RPC path remains `WebSocketServerTask` (E3).

## Features

| # | Feature | Depends on |
|---|---|---|
| 13-E1 | **Resumable frame decoder** `WebSocketFrameDecoder` — WS impl of the Decision 12 §11 `IncrementalDecoder`; partial-frame state across reads; `WouldBlock` mid-frame → `Pending`, not "corrupted"; blocking `decode` becomes a `step`-loop wrapper | Decision 12 §11 |
| 13-E3 | **`WebSocketServerTask` + `WsServerConfig`** — progress-driven server `TaskIterator`: assembler, dual `ConcurrentQueue` seam, config-gated auto-Pong / graceful Close; retrofit blocking `recv` to assemble (OQ#13.4) | 13-E1 |
| 13-E2 | **`Depends` read model** — client + server tasks park on socket readiness via the native reactor (`RegisteredFd: EventReadiness`, Decisions 00/14) when no bytes; fall back to timeout-poll + `Delayed` when no reactor is reachable | 13-E1, **Decisions 00/14**, Decision 12 §12 |
| 13-F1 | **`WebSocketTransport` (server + client)** — wire the enhanced WS conn/task into the Decision 11 seam, perform the upgrade, carry Connect envelopes as binary WS messages; full-duplex bidi end-to-end | 13-E1, 13-E3 |

*(Implementation order: 13-E1 → 13-E3 → 13-F1, with 13-E2 layered in once the reactor is wired
(`RawStream: AsRawFd`, Decision 12 §12). This whole decision is still scheduled last among
transports.)*
