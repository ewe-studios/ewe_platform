# Feature 02 — Bootstrap & TLS-PSK Join

**Status:** ✅ Complete (implemented + tested 2026-07-13)
**Depends on:** 01
**Unblocks:** 04, 10
**Decisions:** [04](../../decisions/04-bootstrap-token-envelope.md), [06](../../decisions/06-hybrid-transport-tls-psk.md), [03](../../decisions/03-seed-derived-keys.md)

## Implementation notes (2026-07-13)

- `shared/bootstrap` — `WgBootstrap::parse` (one parser, two forms: `wg1_` token OR bare
  seed hex/base64url) + `BootstrapToken` with the TLV `wg1_` wire format (tag / varint-len
  / value fields + trailing **CRC32C**, implemented inline). `BootstrapFlags`, `TokenSecret`
  (seed or seedless bootstrap-pubkey), expiry.
- `shared/bootstrap/rpc` — `BootstrapRequest`/`BootstrapResponse`/`Admission` +
  version-prefixed bincode codec (transport-agnostic, reused by the wasm `wss` client in F07).
- `native/bootstrap` — `BootstrapServer`/`BootstrapClient`/`BootstrapConnection` over a real
  TCP + **TLS-PSK** channel via **boring** (BoringSSL); `Join`/`PullMembership`/`Announce`
  over a length-prefixed frame protocol on the TLS stream. A `BootstrapHandler` supplies
  admission + membership.

Tests (`--profile uat`): token round-trip/corruption/truncation/endpoint-required/expiry,
token-vs-bare-seed dispatch, and **two nodes over real TCP+TLS-PSK**: admitted join +
membership transfer + observed Announce, wrong-seed handshake failure, revoked→reject.
All green, zero warnings.

Deviations / notes:
- **boring pinned to 4.x** (not the latest 5.x): the workspace already links BoringSSL via
  `jwt-simple`'s `boring ^4.1.0`, and `boring-sys` `links = "boringssl"` allows only one
  version workspace-wide.
- BoringSSL ships no GCM PSK suites, so the TLS-PSK channel uses **TLS 1.2 `PSK-AES256-CBC-SHA`/
  `PSK-AES128-CBC-SHA`** (PSK identity hint = network id). This is a private control channel;
  confidentiality/auth come from the seed PSK.
- The Join RPC uses our own compact length-prefixed bincode framing over the TLS stream rather
  than the full `foundation_connectrpc` server stack. **Deferred:** re-expressing it as a
  `foundation_connectrpc` service/transport (decision 06 alignment) when F04 assembles the
  runtime — the message types + admission semantics are already in place.

## WHY

Get a node *into* the network from a single secret, over an **encrypted** channel, and hand it the
current membership. This is the front door: parse the bootstrap input, stand up a TCP+TLS-PSK
connection (seed = PSK) via `boring`, and run the `Join`/`PullMembership`/`Announce` RPC over
`foundation_connectrpc`.

## WHAT

`shared::bootstrap` (parser, token format) + `native::bootstrap` (TLS-PSK channel, RPC service/client):

1. `WgBootstrap::parse` — one parser, two forms (self-contained `wg1_` token OR bare seed + endpoint).
2. Token codec — mint/parse the `wg1_` wire format ([decision 04](../../decisions/04-bootstrap-token-envelope.md)).
3. TLS-PSK channel — `boring` client + server with the seed-derived `tls_psk`.
4. Join RPC — connectrpc service `Join`/`PullMembership`/`Announce` over the TLS stream.

## HOW

### Parser & token ([decision 04](../../decisions/04-bootstrap-token-envelope.md))

- `parse(&str)`: `wg1_`-prefixed → decode base64url body, verify CRC32C, extract
  `{network_id, seed|bootstrap_pubkey, endpoints, flags, expires_at}`. Else → treat as bare seed
  (16/32 bytes hex/base64url); network_id + endpoint must come from builder/env or it is a hard error.
- `to_token(network, endpoints, seed_or_pubkey)` → mint a `wg1_` string.

### TLS-PSK channel ([decision 06](../../decisions/06-hybrid-transport-tls-psk.md))

- `boring::ssl::SslContextBuilder` with `set_psk_client_callback` / `set_psk_server_callback`
  returning `tls_psk = derive(...).tls_psk`; identity hint = `network_id`. TLS 1.3, no cert path.
- Client dials a seed endpoint (TCP), completes the PSK handshake; server side = any member with a
  bound bootstrap listener (masterless — [decision 05](../../decisions/05-swim-full-membership-gossip.md)).
- `cfg(not(wasm32))`; the browser variant (feature 07) uses PSK-authenticated `wss` instead.

### Join RPC (connectrpc over the TLS stream)

```
service WgBootstrap {
  Join(JoinRequest{identity_pubkey, tunnel_ip_request, endpoints, caps}) -> JoinResponse{decision, assigned_ip, members}
  PullMembership(Empty) -> Membership{records}
  Announce(PeerRecord) -> Empty        // triggers gossip fan-out (feature 03)
}
```

- Reuse connectrpc codecs/framing; the TLS stream is the transport. `decision` ∈
  `admit | pending | reject` ([decision 11](../../decisions/11-ephemeral-seed-lifecycle.md) admission).
- On `admit`, the joiner receives the full membership and proceeds to feature 04 wiring + the
  identity handoff (feature 08).

### Seed expiry / admission

- Server refuses the PSK handshake or returns `reject` when `expires_at` has passed or the seed's
  `tls_psk` id has been revoked ([decision 11](../../decisions/11-ephemeral-seed-lifecycle.md)).

## Task list

1. `WgBootstrap::parse` + token codec (mint/parse/CRC) + property tests (round-trip, truncation
   rejection, bare-seed detection).
2. `boring` TLS-PSK context (client+server) keyed by `tls_psk`; native TCP listener/dialer.
3. connectrpc `WgBootstrap` service + client over the TLS stream.
4. Admission modes (open/approved) + seed TTL/revocation checks.
5. Tests: two nodes, node B joins node A over real TCP+TLS-PSK, pulls membership, `Announce` observed;
   wrong-seed handshake fails; expired token rejected.

## Test plan / success

- Join over TLS-PSK succeeds only with the correct seed; membership transferred.
- Token round-trips; bare seed + env endpoint path works.
- Expired/revoked seed → `reject` (success criterion 4 partial).
