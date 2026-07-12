# Feature 06 — WebTransport in `foundation_netio`

**Depends on:** none (parallel track; needs netio `quic`+`http3`)
**Unblocks:** 07 (optimization path)
**Decisions:** [09](../../decisions/09-webtransport-on-quinn-proto.md)

## WHY

Give the platform a **WebTransport** transport (no tokio) so the WG relay can later use QUIC
datagrams (no head-of-line blocking) instead of only WebSocket. It is a general netio capability, not
WG-specific.

## WHAT

`foundation_netio::webtransport`:

1. Port the **sans-I/O** `web-transport-proto` (Extended-CONNECT handshake, session, capsule +
   datagram framing) onto our `http3` + `quinn-proto` substrate.
2. Server + native client session API (bidi/uni streams + datagrams).
3. Reuse `web-transport-wasm` for the browser client (feature 07).

## HOW ([decision 09](../../decisions/09-webtransport-on-quinn-proto.md))

- Adapt `web-transport-proto` by replacing its `tokio::io` `AsyncRead`/`AsyncWrite` **trait usage**
  (its only tokio touchpoint) with netio's stream abstractions. The protocol logic (RFC-stable
  Extended CONNECT + capsules) is unchanged.
- **Server:** on the existing `http3` connection, accept `:protocol = webtransport` Extended-CONNECT;
  establish a WT session; surface streams/datagrams over the `quinn-proto` `QuicDriver` (F33).
- **Client (native):** initiate the CONNECT; same session API.
- **No new QUIC stack** — reuse `foundation_netio::quic` (`quinn-proto`, valtron) + `http3`.

## Task list

1. Vendor/port `web-transport-proto` logic into `netio::webtransport` with netio stream traits.
2. Server accept path over `http3` Extended CONNECT; session establishment.
3. Native client; bidi/uni stream + datagram surface.
4. Tests (`required-features = ["quic","h3"]`): native client ↔ native server WT session, stream +
   datagram round-trip, over `quinn-proto` on valtron; assert **no tokio** in the tree.

## Test plan / success

- WT session establishes via Extended CONNECT; streams + datagrams round-trip.
- Zero tokio (success criterion 9); reusable by feature 07's relay.
