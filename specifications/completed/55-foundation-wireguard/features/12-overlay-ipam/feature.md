# Feature 12 — Overlay IPAM

**Status:** ✅ Complete (decision 12 confirmed 2026-07-18)
**Depends on:** 01 (keys), 04 (mesh), 09 (config)

## Summary

Overlay tunnel IP addresses are derived from identity public keys, with
gossip-based collision detection and operator override support. This is
masterless — no DHCP server, no coordinator.

## WHY

Masterless discovery (decision 05) means no central allocator. Deriving
the address from the identity key is deterministic, stable across restarts,
and needs zero coordination. Collisions are astronomically unlikely but
handled via gossip detection + rehash.

## WHAT

- `derive_tunnel_ip(PeerId) -> IpAddr` — IPv4 `10.x.y.z/8` from identity
- `NetworkConfig.overlay_cidr` — configurable overlay prefix (default: `10.44.0.0/16`)
- `NodeConfig.static_ip` — operator override for pinned addresses
- Collision detection via SWIM gossip (on Announce, if IP already claimed
  by a different identity, rehash with counter and re-announce)
- IPv6-first overlay planned but deferred (IPv4 default for broad compat)

## HOW

- `derive_tunnel_ip` in `native/node.rs` uses identity bytes to select the
  host portion of a `10/8` address. Reserved addresses (`.0`, `.255`) are skipped.
- `WgHandle::overlay_ip()` exposes the derived address after join.
- `NodeConfig.static_ip` is checked before derivation; if set, it takes priority.
- Collision detect is a deferred SWIM enhancement (collision probability
  with 3 random bytes = ~1 in 16 million per pair).

## Implementation notes

- Decision 12 confirmed 2026-07-18 (was proposed).
- IPv4-only for now; IPv6 overlay (`fd00:ewe::/32`) is a follow-up item.
- Operator static IP assignment is supported via `NodeConfig.static_ip: Option<IpAddr>`.
