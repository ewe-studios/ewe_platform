---
feature: "HTTP/3 ConnectRPC transport + conformance (D01 phase 3)"
description: "Transport impl + ConnectionHandler third branch; gRPC/Connect over h3"
status: "implemented (server half + streaming + conformance test; netcap Quic listener remain)"
priority: "medium"
phase: 3
depends_on: ["34-http3-module", "22-router-dispatch"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 35-http3-transport-integration: HTTP/3 ConnectRPC transport + conformance (D01 phase 3)

## Description

Wiring HTTP/3 into the seam: the third ConnectionHandler branch and client transport, closing the any-protocol-on-any-transport matrix for QUIC.

## Normative sources (single source of truth — read before writing code)

- decisions/01-transport-and-runtime.md — phasing; decisions/11-transport-seam.md — capabilities

## Scope

- Transport impl (full_duplex, multiplexed, HTTP30); server ConnectionHandler third branch; capability matrix entries
- **netcap `Quic` variants** on `Connection` / `Listener` / `ConfigListenAddr`, and routing
  `HttpServer` through netcap's `Listener` (D12 §9 items 1–2). *Moved here from feature 33 on
  2026-07-10:* D12 §9 pairs these with the `ConnectionHandler` branch, and its item 3 makes
  `Connection` the byte stream for "individual QUIC streams" — which only means something once
  HTTP/3 maps streams onto the Router. F34 does not need them.
- **Alt-Svc advertisement (T2) and `ConnectionContext` population.** *Moved here from feature 34
  on 2026-07-10:* `Alt-Svc: h3=...` is a header an HTTP/1.1 or HTTP/2 **response** carries to
  advertise that the origin also speaks HTTP/3. It is emitted by the TCP server, not by the
  HTTP/3 module, and there is no TCP server to emit it from until this feature routes
  `HttpServer` through netcap's `Listener`. `ConnectionContext` is populated by the front end at
  accept/handshake time (D12 §13); `H3Connection` already threads an `Arc<ConnectionContext>`
  through `request_from_fields`, so only the filling-in remains.

## Acceptance criteria

- Connect + gRPC suites pass over HTTP/3; bidi full-duplex verified

## Progress 2026-07-10

### Done — the client transport and the capability matrix

- **`foundation_netio::quic` re-exports `ClientConfig`/`ServerConfig`.** `H3Transport::new`
  takes a client config; leaking `quinn_proto` into connectrpc's public API would force
  connectrpc to depend on it.
- **`foundation_connectrpc::transport::h3::H3Transport`**, behind a new `h3` feature
  (`h3 = ["foundation_netio/quic"]`). Mirrors `H2Transport`: `open()` performs the QUIC
  handshake on the caller's thread, then spawns `H3Pump` — a valtron `TaskIterator` that owns
  the `QuicDriver`, the `H3Connection` and the request stream, and returns the four
  `TransportStream` halves synchronously.
  - One task drives both QUIC and HTTP/3. `quinn-proto` is sans-IO, so the pump steps the
    driver once per poll before doing HTTP/3 work. Two tasks would need the driver and the
    stream handles to coordinate wake-ups.
  - **Full duplex is real**: every poll advances the request direction *and* the response
    direction, and QUIC gives each its own flow-control window.
  - Response chunks the body pipe refuses are queued in order. Dropping a `Full` chunk
    silently truncates the response body — the same trap `H2Pump` documents.
- **`H3Request::trailers()`** (netio). `poll_body` used to decode a trailing HEADERS frame as
  "end of body" and throw it away. A second HEADERS frame after the body **is** the trailers
  section (RFC 9114 §4.1), and gRPC's status lives there — so every gRPC call over HTTP/3
  would have failed to report its outcome.
- **Capability matrix entries.** `TransportCapabilities` for HTTP/3: `request_streaming`,
  `full_duplex`, `h2_trailers` (HTTP/3's trailers are a second HEADERS frame — the same
  capability gRPC needs), `http_versions: [HTTP30]`, `multiplexed: false` (one QUIC connection
  per call today; HTTP/3 multiplexes natively, so this flips once the client pools connections).

  No change was needed to `check_compatible`: `Proto` orders `HTTP10 < HTTP11 < HTTP20 < HTTP30`,
  so a transport offering only HTTP/3 already satisfies gRPC's `min_http_version = HTTP20`.
  `tests/h3_capability_tests.rs` (6 tests) asserts that ordering directly rather than trusting
  it, checks every (protocol × stream mode) pair, and — importantly — checks that the matrix
  still *rejects* a transport without trailers or without full duplex.

### Remaining — the server half

> **Sequencing note (2026-07-12, revised after investigation):** the four items
> below are one interlocking block. Two findings sharpen the scope:
>
> 1. **`dispatch_h3` is an adapter, not a mirror of `dispatch_h2`.** h2 serving is
>    frame-pipe based (`H2Serve` takes `PipeReceiver<H2IncomingFrame>` +
>    `PipeSender<H2Frame>`, and `dispatch_h2` drives those pipes). HTTP/3 is
>    *stream*-oriented: `H3Request` exposes `poll_headers()` / `poll_body()` /
>    `poll_send_headers()/data()/finish()` directly on the QUIC bidi stream. So
>    `dispatch_h3` must **bridge** that poll-based stream API to the ConnectRPC
>    dispatcher (which consumes `SimpleIncomingRequest`/byte pipes and emits framed
>    responses) — converting inbound QPACK fields via `request_from_fields`, feeding
>    `poll_body` chunks into the protocol's byte pipe, and pumping the response pipe
>    back out through `poll_send_*`. That adapter is real async code with its own
>    correctness surface, roughly the size of `dispatch_h2` itself.
> 2. **It is testable without the full netcap refactor.** netio's
>    `tests/http3/connection_tests.rs` already stands up a live QUIC pair
>    (`quic_pair()`, `drive_until`, `H3Connection::poll_accept`) and drives real
>    request/response/trailers by stepping the `QuicDriver`s by hand. A
>    connectrpc-side conformance test can reuse that shape to drive `dispatch_h3`
>    end-to-end over real QUIC/H3 — so the driven test does **not** block on item 2
>    (the netcap `Quic` `Listener` for the *production* accept path still does).
>
> Net: the server half is a single **feature-sized effort** — the `dispatch_h3`
> adapter + `H3Serve` trait (`foundation_http`) + `ConnectRpcServeH3` + a
> QUIC-driven conformance test, then (for production serving) the netcap `Quic`
> `Listener`/`Connection` variants + `ServerApp::Http3` + Alt-Svc. The client
> transport (above) is complete and independent; this is the next feature to build.

1. **`H3Serve` + the third `ConnectionHandler` branch.** `ServerApp` is
   `Http1 | Http2 | Both`; it needs `Http3`. `H2Serve` is the model: one task per stream, the
   handler never sees the connection, response frames go into a pipe the connection's poll loop
   drains. HTTP/3 is the same shape — `H3Connection::poll_accept` yields one `H3Request` per
   bidirectional stream.
2. **netcap `Quic` variants** on `Connection` / `Listener` / `ConfigListenAddr`, and routing
   `HttpServer` through netcap's `Listener` (D12 §9 items 1–2; moved here from F33). Note D12
   §9 item 3: `Connection` stays the byte stream for "individual QUIC streams".
3. **Alt-Svc advertisement (T2)** — a header an HTTP/1.1 or HTTP/2 *response* carries to
   advertise that the origin also speaks HTTP/3. Needs (2) first: it is emitted by the TCP
   server. **`ConnectionContext` population** at accept/handshake time (D12 §13);
   `H3Connection` already threads an `Arc<ConnectionContext>` through `request_from_fields`, so
   only the filling-in remains.
4. **Conformance.** The acceptance criterion — "Connect + gRPC suites pass over HTTP/3; bidi
   full-duplex verified" — needs (1) and (2), because the suites drive a real `HttpServer`
   (`tests/h2_streaming_tests.rs` is the model). Standing up an HTTP/3 server inside the test
   instead would be bespoke test machinery; the serving glue belongs in `foundation_http`.

The wire behaviour the transport rests on is already proved end-to-end in netio's
`tests/http3/connection_tests.rs`: a real request and response, plus trailers, over live QUIC.
