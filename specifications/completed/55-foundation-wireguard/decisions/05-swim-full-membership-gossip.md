# 05 — Masterless SWIM/Serf Full-Membership Gossip

**Date:** 2026-07-12
**Status:** Resolved (owner-confirmed)

## Decision

Peer discovery uses **masterless, full-membership gossip** in the SWIM/Serf style: every node knows
every other node, membership spreads epidemically, failure is detected by SWIM, and **no node is
special**. The first instance is only a *seed*; the mesh survives its death.

Rejected: a persistent coordinator (single point of failure) and static all-peers-list config (no
self-healing/scaling). Rejected for now: partial-view epidemic (HyParView/Plumtree) — it scales to
tens of thousands but no node has the full picture and it is materially more complex; full-membership
is the natural fit because **WireGuard needs the full peer list anyway** and our target is a service
mesh of hundreds–low-thousands.

## Why

- Matches the owner's vision precisely: *"once an instance knows, it gossips the services it knows to
  others, making it spread naturally, removing the need for any master."*
- Full membership means each node already holds exactly what WG configuration needs: for every peer,
  `{ identity_pubkey, tunnel_ip (allowed_ips), current_endpoint }`.
- SWIM gives robust failure detection with low false-positive rate (indirect probing) and bounded,
  well-understood message load.

## What we do

### Membership record (the gossiped unit)

```rust
struct PeerRecord {
    identity_pubkey: [u8; 32],   // node identity (post-handoff); the map key
    tunnel_ip: IpAddr,           // overlay address (allowed_ips) — see decision 12
    endpoints: Vec<SocketAddr>,  // observed reachable UDP endpoints (LWW)
    caps: Capabilities,          // relay-capable, gateway, ipv6, ...  (decision 07)
    incarnation: u64,            // owner-incremented refutation counter
    state: Alive | Suspect | Dead,
    heartbeat: u64,              // monotonic, for LWW on endpoints/caps
}
```

### SWIM mechanics

- **Join:** contact any seed endpoint over the TLS-PSK bootstrap channel
  ([decision 06](06-hybrid-transport-tls-psk.md)), `PullMembership` to get the full set, then
  `Announce` self; existing members re-gossip the new record (epidemic spread).
- **Failure detection:** periodic direct `ping`; on timeout, `ping-req` via *k* random members
  (indirect probe) before marking `Suspect`; a `Suspect` not refuted within a timeout becomes `Dead`
  and is **tombstoned**.
- **Refutation:** a node that hears itself `Suspect`/`Dead` broadcasts a higher `incarnation` to
  refute (standard SWIM), preventing flapping from transient blips.
- **LWW reconciliation:** endpoint/caps updates are last-writer-wins by `(incarnation, heartbeat)`;
  membership merges are commutative/idempotent (CRDT-like), so anti-entropy is safe to run anytime.
- **Anti-entropy:** each node periodically picks a random known peer and exchanges a membership
  **digest** (hashes + incarnations), pulling only what it is missing — catches anything gossip
  dropped.
- **Tombstones** carry a TTL so a re-joining node with the same identity isn't permanently shunned;
  after TTL they are garbage-collected.

### Seeds & resilience

- A token may carry **multiple** seed endpoints ([decision 04](04-bootstrap-token-envelope.md)); the
  joiner tries them in order/parallel.
- Once a node knows ≥1 live peer, it can re-bootstrap from **any** of them — the seed's role ends at
  t=0. **Killing the seed does not partition survivors** (success criterion 3).

### Where it runs

- **Join/first-contact:** over the TCP+TLS-PSK bootstrap channel (before the tunnel exists).
- **Steady-state gossip:** **inside the WireGuard tunnel** on a reserved internal address/port
  ([decision 06](06-hybrid-transport-tls-psk.md)) — so membership traffic is itself private and only
  shared with already-trusted peers.

### Implementation shape

The SWIM state machine is **sans-I/O** (like `Tunn`/smoltcp): it consumes timer ticks + inbound
messages and emits outbound messages + membership-change events. It is driven by a valtron task and
transported by connectrpc (bootstrap) / the in-tunnel UDP channel (steady state). This keeps it unit-
testable with a simulated clock and network.

## Consequences

- Message load is O(1) per node per period (SWIM), independent of cluster size; membership storage is
  O(N) per node (full membership) — fine to low-thousands.
- If we later need tens-of-thousands scale, the sans-I/O core can be swapped for a partial-view
  algorithm without touching the mesh/data-plane layers.

## Related decisions

- [04 — Bootstrap token envelope](04-bootstrap-token-envelope.md)
- [06 — Hybrid transport & TLS-PSK](06-hybrid-transport-tls-psk.md)
- [07 — Relay as capability](07-relay-as-capability.md)
- [12 — IPAM addressing](12-ipam-addressing.md)
