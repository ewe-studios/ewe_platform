# 09 — WebTransport on Our `quinn-proto` (no tokio), in `foundation_netio`

**Date:** 2026-07-12
**Status:** Resolved (owner-confirmed)

## Decision

Provide **WebTransport** in **`foundation_netio`** (a `webtransport` module, peer to `websocket` and
`http3`) by **porting the sans-I/O `web-transport-proto`** onto our existing **`quinn-proto` +
`http3`** substrate, driven on **valtron**. Do **not** depend on `web-transport-quinn` (it is
tokio-bound). Reuse **`web-transport-wasm`** (browser `web_sys::WebTransport`, no tokio) for the
browser side.

## Why — the crate split already isolates tokio for us

Reviewed `@formulas/.../src.WebTransport/src.MoqDev/web-transport`:

| Crate | Role | tokio? | Our plan |
|-------|------|--------|----------|
| `web-transport-proto` 0.6 | Core WT protocol: Extended-CONNECT handshake, session setup, capsule + datagram framing over HTTP/3 | **Sans-I/O** — the only tokio dep is for the `AsyncRead`/`AsyncWrite` **trait definitions** (their own comment) | **port/adapt**, swap the traits for ours |
| `web-transport-quinn` 0.11 | Binds proto → **quinn's tokio runtime** (`runtime-tokio`, `futures`, `tokio` time) | **Heavy** | **replace** with a valtron binding |
| `web-transport-wasm` 0.5 | Browser `web_sys::WebTransport` bindings | **None** | **reuse directly** |
| `web-transport-trait` 0.3 | Async trait abstraction | — | optional |

And the substrate WebTransport needs, **we already have**: `foundation_netio` uses **`quinn-proto`**
(the **sans-I/O** QUIC core, *not* tokio-`quinn`) driven by our own `QuicDriver` valtron task (F33),
plus our own HTTP/3 stack (`http3/`: connection, frame, qpack, stream, varint). WebTransport is
literally "HTTP/3 Extended CONNECT + capsules + QUIC datagrams," so the layer it sits on already
exists in netio, on valtron. We do exactly what `web-transport-quinn` does — bind the sans-I/O proto
to a QUIC endpoint — except our endpoint is `quinn-proto` on valtron instead of quinn on tokio.
**No tokio enters the tree** (success criterion 9).

## What we do

- New `foundation_netio::webtransport` module:
  - Adapt `web-transport-proto`'s handshake/session/capsule logic to our stream traits (replace its
    `tokio::io` `AsyncRead`/`AsyncWrite` trait usage with netio's stream abstractions).
  - Server: accept an Extended-CONNECT `:protocol = webtransport` request on our `http3` connection,
    establish a WT session, expose bidi/uni streams + datagrams over `quinn-proto`.
  - Client (native): initiate the CONNECT and expose the same session API.
- **Placement rationale:** WebTransport is a general transport capability (like `websocket`,
  `http3`), so it belongs in `foundation_netio` and is reusable platform-wide; the WG relay
  ([decision 07](07-relay-as-capability.md)/[08](08-wasm-browser-and-webrtc.md)) becomes a thin
  consumer, exactly as it already consumes `netio::websocket`.

## Sequencing

WebTransport (feature 06) has **no dependency** on the WG core and can be built on a parallel track.
It is the **later** relay optimization: WebSocket relay ships first
([decision 08](08-wasm-browser-and-webrtc.md)); WebTransport (QUIC datagrams, no head-of-line
blocking) is layered in once the netio module exists.

## Consequences

- We take on maintaining a port of `web-transport-proto`'s protocol logic. It is small and stable
  (HTTP/3 Extended CONNECT + capsule protocol are RFC-stable). We track upstream for spec fixes.
- Browser WebTransport is gated on `web_sys_unstable_apis` and browser support; that is why it is an
  *optimization* behind the universal WebSocket relay, not the primary path.

## Related decisions

- [07 — Relay as capability](07-relay-as-capability.md)
- [08 — wasm/browser & WebRTC](08-wasm-browser-and-webrtc.md)
