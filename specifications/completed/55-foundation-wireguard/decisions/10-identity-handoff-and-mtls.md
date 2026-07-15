# 10 — Identity Handoff & Optional App-Layer mTLS

**Date:** 2026-07-12
**Status:** Resolved (owner-confirmed)

## Decision

After bootstrap, the mesh transitions from the **shared** bootstrap keypair to **per-peer random
identity keypairs**, and stays on WireGuard. Steady-state traffic is authenticated by WireGuard's own
Noise handshake over per-peer static keys; **optional** application-layer mTLS (via `boring`) may be
layered on top per service. WireGuard is **not** abandoned after bootstrap.

## Why

- The shared bootstrap key (derived from the seed — [decision 03](03-seed-derived-keys.md)) is
  deliberately weak: anyone with the seed can act as a bootstrap peer. It exists only to get a node
  *in*. Moving to per-peer identity keys makes each node **individually** authenticated and bounds
  the seed's compromise window ([decision 11](11-ephemeral-seed-lifecycle.md)).
- WireGuard's Noise_IK handshake over per-peer static keys already provides **mutual authentication +
  forward secrecy**; a mandatory mTLS layer on top would be redundant. So mTLS is **optional** —
  present for services that want application-level identity/authorization or defense-in-depth, off by
  default.
- The owner's phrasing ("negotiate a proper mTLS **or** private/public key sharing process … talk
  *entirely* on the WireGuard network") maps exactly to: the core handoff is key rotation *on* WG;
  mTLS is an optional add-on *over* WG — **not** a move off WG onto plaintext+mTLS (explicitly
  rejected).

## What we do

### Handoff sequence

```mermaid
sequenceDiagram
    participant N as Node
    participant M as Mesh
    Note over N: bootstrap phase — shared derived key + PSK, tunnel up, joined
    N->>N: generate RANDOM identity keypair (OsRng, NOT seed-derived)
    N->>M: Announce(identity_pubkey, tunnel_ip, endpoints, caps) over secured gossip
    M->>M: add N as WG peer keyed by identity_pubkey; N adds each peer likewise
    Note over N,M: tunnels re-key onto per-peer identity static keys
    N->>N: retire bootstrap keypair; stop honoring bootstrap-key sessions
    Note over N,M: steady state — per-peer WG; optional app-layer mTLS per service
```

- **Identity generation:** `x25519::StaticSecret::random_from_rng(OsRng)`; the private half never
  leaves the node, the public half is gossiped.
- **Re-pairing:** each node installs every other node's identity pubkey as a `Tunn` peer (its own
  `Tunn` per peer, keyed by that peer's identity key + `allowed_ips` = tunnel_ip). The shared
  bootstrap `Tunn` is dropped once the identity tunnel to a peer is established.
- **Session rekey is orthogonal:** WireGuard already rotates **session** keys internally (~2 min,
  `REKEY_AFTER_TIME`); the handoff here concerns **static identity** keys, done once per node
  lifetime (plus optional periodic identity rotation).

### Optional app-layer mTLS

- A per-service flag (`security: mtls`) turns on `boring`-based mTLS over the overlay socket. Peer
  identity for mTLS can be pinned to the WG identity pubkey (raw-public-key TLS) or a derived
  self-signed cert, so no external CA is required.
- Default is **off**: services get privacy + peer auth from WG alone.

## Consequences

- A brief window exists where a node holds both a bootstrap tunnel and forming identity tunnels;
  the mesh layer must sequence the cutover so no traffic is dropped (establish identity tunnel →
  migrate → retire bootstrap tunnel).
- Identity keys should be **persisted** ([feature 09/10]) so a restart rejoins as the same identity
  rather than re-announcing a new one and orphaning a tombstone.

## Related decisions

- [03 — Seed-derived keys](03-seed-derived-keys.md)
- [05 — SWIM full-membership gossip](05-swim-full-membership-gossip.md)
- [11 — Ephemeral seed lifecycle](11-ephemeral-seed-lifecycle.md)
