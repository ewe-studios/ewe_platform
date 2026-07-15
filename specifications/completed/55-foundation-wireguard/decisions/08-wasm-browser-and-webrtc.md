# 08 — wasm/Browser Support + WebRTC (implemented in this spec)

**Date:** 2026-07-12
**Status:** Resolved (owner-confirmed)

## Decision

Implement the **browser** data path **in this spec** (not deferred):

- A browser peer runs **`boringtun::Tunn` + smoltcp in wasm** and does its **own** WG crypto +
  TCP/IP end-to-end (both are pure Rust / `no_std`, compile to `wasm32`).
- **Join** over **connectrpc-over-`wss`** (PSK-authenticated), reusing the F51 WebSocket client.
- **Data plane + steady-state gossip**: since browsers have **no UDP**, the browser tunnels its
  **encrypted WG datagrams** to a **relay-capable member** ([decision 07](07-relay-as-capability.md))
  over **WebSocket first**, and **additionally uses WebRTC** to establish a **direct** peer data
  channel when NAT permits (relay as TURN-style fallback). **WebTransport** is a later no-HoL
  optimization ([decision 09](09-webtransport-on-quinn-proto.md)).

## Why

- Cross-platform (native/wasm/Android/iOS) is a first-class goal; deferring wasm risks module-shape
  drift (the F52 lesson in project memory). The owner chose **design + implement now**.
- The browser is the *only* platform with a hard constraint: **no UDP API exists**, so WG's UDP
  transport and the UDP SWIM gossip cannot run directly — a relay (or WebRTC-over-UDP) is
  unavoidable. Everything *above* the socket (Tunn crypto, smoltcp) runs unchanged in wasm.
- The relay only moves **opaque ciphertext**, so the browser's privacy is identical to a native
  peer's — the relay is not a trust break.
- The owner chose **WebRTC now** to cut relay load/latency: WebRTC data channels give the browser
  UDP-like P2P (via ICE/DTLS/SCTP), falling back to TURN-relay only when NAT blocks the direct path.

## What we do

### Browser peer (`wasm/`)

- Compile `Tunn` + the smoltcp `NetStack` to wasm; the smoltcp `Device`'s "wire" is a JS-bridged
  transport instead of a native UDP socket.
- **Join:** connectrpc client over `wss` to a member's bootstrap endpoint; TLS-PSK maps to a
  PSK-authenticated `wss` handshake carrying the same `Join`/`PullMembership`/`Announce`.
- **Relay client:** WebSocket (`web_sys::WebSocket`) to a relay member; frames
  `{ dst_peer_id, opaque_wg_bytes }` both ways. This is the always-available path.
- **WebRTC:** `web_sys::RtcPeerConnection` + `RtcDataChannel`; **signaling (SDP offer/answer + ICE
  candidates) is carried over the existing join/gossip RPC** (no separate signaling server). On a
  successful direct channel, WG datagrams flow peer-to-peer; the relay is dropped to fallback.
- Reuses `web-transport-wasm` (`web_sys::WebTransport`, no tokio) when the WebTransport path lands.

### Native peer side (`native/`)

- **Relay server** (the capability from [decision 07](07-relay-as-capability.md)) accepts browser
  attachments over WebSocket (and later WebTransport), forwarding opaque ciphertext.
- **WebRTC answerer:** the native side terminates the browser's WebRTC data channel. This needs
  ICE + DTLS + SCTP on the native side — the heaviest sub-component; see the feature doc for the
  build/borrow plan (evaluate a sans-I/O WebRTC core vs a minimal purpose-built data-channel-only
  implementation; keep it valtron-driven, no tokio).

### Android / iOS

- Native UDP works, so they use the ordinary native data plane — **no relay needed**. WebRTC/relay
  are available but not required. Optional platform-VPN TUN adapters (`VpnService`/`NetworkExtension`)
  are later porting work, not part of this decision.

## Consequences / risks

- **WebRTC on the native side is significant scope** (ICE/STUN/DTLS/SCTP). It is isolated in
  feature 07 so the WebSocket relay path (universal, simpler) can land and be validated first, with
  WebRTC as the direct-path upgrade. If native WebRTC proves too heavy for this spec's timeline, the
  WebSocket relay still fully satisfies browser connectivity; WebRTC can slip without reshaping the
  capability model.
- Browser cannot be a **relay** for others (no UDP, limited server sockets) — browsers are always
  relay *clients*; only native members advertise the relay capability.

## Related decisions

- [07 — Relay as capability](07-relay-as-capability.md)
- [09 — WebTransport on quinn-proto](09-webtransport-on-quinn-proto.md)
- [06 — Hybrid transport & TLS-PSK](06-hybrid-transport-tls-psk.md)
