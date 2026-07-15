# 12 — IPAM: Overlay Address Assignment (PROPOSED)

**Date:** 2026-07-12
**Status:** **Proposed** (default recommendation — owner review pending)

## Decision (proposed)

Each node's overlay/tunnel IP (its WireGuard `allowed_ips` /32 or /128) is **derived from its
identity public key** by hashing into the network's address range, with **gossip-based
collision-detection** and an **operator override**.

## Why

- Masterless discovery ([decision 05](05-swim-full-membership-gossip.md)) means there is no
  coordinator to hand out DHCP-style leases. Deriving the address from the identity key needs **no
  central allocator** and is deterministic/stable across restarts (pairs with identity persistence,
  [decision 10](10-identity-handoff-and-mtls.md)).
- Collisions are astronomically unlikely in a large range and are cheaply resolved by gossip
  (probe-and-rehash), so we keep the no-coordinator property without risking silent address clashes.

## What we do (proposed)

- Network config declares an overlay CIDR (default **`fd00:ewe::/32`** for IPv6-first, or
  **`10.44.0.0/16`** for IPv4 networks — configurable).
- `tunnel_ip = range.base + (BLAKE2s(identity_pubkey || network_id) mod range.size)`, skipping
  reserved addresses (`::0`, the SWIM in-tunnel service address — [decision 06](06-hybrid-transport-tls-psk.md)).
- On `Announce`, if the derived address is already claimed by a *different* identity in membership,
  the joiner rehashes with a counter and re-announces; existing members detect and report the
  collision via gossip.
- **Operator override:** config/macro may pin a specific `tunnel_ip` per node (static assignment) for
  cases needing predictable addresses (e.g. a fixed service endpoint).

## Open questions for review

- IPv6-first (huge range, collisions effectively impossible) vs IPv4 default (broader app
  compatibility, smaller range → collision handling matters more). Recommendation: **IPv6 overlay by
  default**, IPv4 opt-in.
- Whether to also support a lightweight **lease** mode (a member acts as allocator) for operators who
  want dense, human-readable IPv4 addressing. Deferred unless requested.

## Related decisions

- [05 — SWIM full-membership gossip](05-swim-full-membership-gossip.md)
- [10 — Identity handoff & mTLS](10-identity-handoff-and-mtls.md)
- [13 — Tri-config & macro](13-tri-config-and-macro.md)
