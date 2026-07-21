---
feature_name: overlay-ipam
feature_number: 12
created: 2026-07-18
status: complete
---

# Start — Overlay IPAM (F12)

## Current state

IP-from-pubkey derivation is implemented in `native/node.rs::derive_tunnel_ip`.
Collision detection via gossip is deferred (extremely low probability).
Static IP override is supported via `NodeConfig.static_ip`.

## Remaining work

- IPv6 overlay support (`fd00:ewe::/32` default)
- Gossip collision detection (rehash-on-collision)
