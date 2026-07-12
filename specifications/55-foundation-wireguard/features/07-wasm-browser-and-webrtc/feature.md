# Feature 07 — wasm/Browser & WebRTC

**Depends on:** 04, 05, 06
**Decisions:** [08](../../decisions/08-wasm-browser-and-webrtc.md), [07](../../decisions/07-relay-as-capability.md), [06](../../decisions/09-webtransport-on-quinn-proto.md)

## WHY

Let a **browser** peer join the mesh and reach services — the only platform with no UDP. Implemented
in this spec (owner-confirmed): browser runs `Tunn`+smoltcp, joins via `wss`, relays over WebSocket,
and upgrades to **direct WebRTC** when NAT permits.

## WHAT

`wasm/` (browser peer) + `native/` additions (relay server WS accept, WebRTC answerer):

1. Browser `Tunn` + smoltcp `NetStack` compiled to `wasm32`, JS-bridged transport.
2. `wss` connectrpc join (PSK-authenticated), reusing F51 WebSocket client.
3. WebSocket relay client (browser) ↔ relay server (native, feature 05).
4. WebRTC direct data channel (browser `RtcPeerConnection`/`RtcDataChannel`) + native answerer;
   signaling over the join/gossip RPC.

## HOW ([decision 08](../../decisions/08-wasm-browser-and-webrtc.md))

- **Data plane in wasm:** the smoltcp `Device`'s wire is a JS-bridged channel (WS/WebRTC), not a
  native UDP socket. `Tunn` crypto + smoltcp run unchanged in wasm.
- **Join:** connectrpc over `web_sys::WebSocket` (`wss`) to a member's bootstrap endpoint; TLS-PSK →
  PSK-authenticated `wss`; same `Join`/`PullMembership`/`Announce`.
- **Relay (always-available):** browser frames `{dst_peer_id, opaque_wg_bytes}` over WS to a
  relay-capable member; native relay forwards (feature 05). Ciphertext only.
- **WebRTC (direct upgrade):** `RtcPeerConnection` + `RtcDataChannel`; **SDP offer/answer + ICE
  candidates carried over the existing join/gossip RPC** (no separate signaling server). On success,
  WG datagrams flow P2P; relay drops to fallback. Native side terminates the data channel (ICE/DTLS/
  SCTP) — the heaviest sub-component; keep valtron-driven, **no tokio**. Evaluate a sans-I/O WebRTC
  core vs a minimal data-channel-only implementation (see risks).
- **WebTransport:** optional later relay transport via feature 06 + `web-transport-wasm`.

## Risks / sequencing

- **Native WebRTC (ICE/STUN/DTLS/SCTP) is significant.** Land the **WebSocket relay path first**
  (universal, validates browser connectivity end-to-end), then the WebRTC direct upgrade. If WebRTC
  overruns, the WS relay fully satisfies browser reachability and WebRTC can slip without reshaping
  the capability model ([decision 08](../../decisions/08-wasm-browser-and-webrtc.md)).
- Browsers are relay **clients** only, never relay servers.

## Task list

1. wasm build of `Tunn` + smoltcp `NetStack`; JS-bridged `Device`.
2. `wss` connectrpc join client (reuse F51); PSK-authenticated handshake.
3. WS relay client (browser) + WS accept in the native relay server.
4. WebRTC: browser offerer + native answerer; signaling over gossip; data-channel `Device` transport.
5. Tests: headless-browser (deno/wasm testbed per project memory) join + reach a native service via
   relay; WebRTC direct upgrade in a permissive-NAT sim.

## Test plan / success

- Browser peer joins via `wss`, reaches a native peer's service through a relay (success criterion 5).
- Where NAT permits, browser upgrades to direct WebRTC and stops relaying.
