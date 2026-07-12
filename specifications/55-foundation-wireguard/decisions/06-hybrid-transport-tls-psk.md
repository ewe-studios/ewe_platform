# 06 — Hybrid Gossip Transport + TLS-PSK Bootstrap

**Date:** 2026-07-12
**Status:** Resolved (owner-confirmed)

## Decision

- **Join / first contact** runs over a **TCP + TLS-PSK** channel (BoringSSL via `boring`), where the
  **bootstrap seed is the TLS pre-shared key** (external PSK, RFC 4279 / TLS 1.3). The
  join/membership exchange is a **`foundation_connectrpc`** RPC (`Join` / `PullMembership` /
  `Announce`) over that TLS connection.
- **Steady-state gossip** ([decision 05](05-swim-full-membership-gossip.md)) runs **inside the
  WireGuard tunnel** over UDP on a reserved internal address/port.

So: **TCP+TLS-PSK to get in → WireGuard-internal UDP once in.**

## Why

- **Encrypted join, no metadata leak.** Upgrading first contact from plaintext-authenticated UDP to
  TLS means *nothing* — not even membership/endpoint metadata — is exposed before the tunnel forms.
- **This is why `boring` is a source alongside `boringtun`.** BoringSSL supports **external PSK
  ciphersuites** directly; the shared seed authenticates the channel with **zero certs / zero PKI**,
  dovetailing with seed-derived keys ([decision 03](03-seed-derived-keys.md)). rustls' external-PSK
  support is far less mature, so `boring` is the right tool *for the control channel*.
- **Reuse, not reinvent.** Running the join as a connectrpc RPC over TLS means we reuse codecs,
  framing, and the WebSocket client (F51) — no hand-rolled bootstrap wire format. The browser path
  ([decision 08](08-wasm-browser-and-webrtc.md)) then falls out for free as connectrpc-over-`wss`.
- **Right tool per phase.** TCP+TLS is ideal for the **one-shot** membership dump (reliable, ordered,
  possibly large). UDP is ideal for **steady-state** SWIM gossip; and running it *inside* the tunnel
  means endpoints are only ever shared with already-trusted peers.

## What we do

### Bootstrap channel

- `tls_psk = HKDF-Expand(prk, "tls-psk|" || network_id)` ([decision 03](03-seed-derived-keys.md)).
- `boring` `SslContextBuilder` with a PSK callback (client + server) returning `tls_psk` and an
  identity hint = `network_id`. TLS 1.3 external PSK; no certificate verification path.
- A tiny connectrpc service over the TLS stream:
  - `Join(identity_pubkey, tunnel_ip_request, endpoints, caps)` → accept/assign + current membership.
  - `PullMembership()` → full `Vec<PeerRecord>` digest.
  - `Announce(PeerRecord)` → gossip fan-out trigger.
- **Server side is any member** (masterless — [decision 05](05-swim-full-membership-gossip.md)); the
  bootstrap listener is bound by every node that advertises a reachable endpoint.

### In-tunnel gossip channel

- Reserve an overlay address/port (e.g. `<network>::1:7777` or a fixed link-local) inside the smoltcp
  netstack / TUN range for SWIM UDP traffic; only tunnel members can reach it by construction.
- The SWIM sans-I/O core is transport-agnostic; the mesh layer routes its messages to the bootstrap
  connectrpc channel during join and to the in-tunnel UDP socket thereafter.

### Target gating

`boring` is **native-only**; browsers use `web_sys` TLS via `wss`
([decision 08](08-wasm-browser-and-webrtc.md)). The TLS-PSK bootstrap module is
`cfg(not(wasm32))`; the browser uses a PSK-authenticated `wss` handshake to a member's bootstrap
endpoint carrying the same connectrpc `Join`.

## Consequences

- Two crypto stacks coexist (ring for WG, BoringSSL for the control channel) — accepted
  ([decision 01](01-source-crates-and-pinning.md)).
- The bootstrap listener is an attack surface, but it is PSK-gated: without the seed-derived
  `tls_psk` the handshake fails before any RPC is processed. Combined with the seed's **ephemerality**
  ([decision 11](11-ephemeral-seed-lifecycle.md)), the exposure window is bounded.

## Related decisions

- [03 — Seed-derived keys](03-seed-derived-keys.md)
- [05 — SWIM full-membership gossip](05-swim-full-membership-gossip.md)
- [08 — wasm/browser & WebRTC](08-wasm-browser-and-webrtc.md)
- [11 — Ephemeral seed lifecycle](11-ephemeral-seed-lifecycle.md)
