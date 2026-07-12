# 11 — Ephemeral Seed Lifecycle

**Date:** 2026-07-12
**Status:** Resolved (owner-confirmed)

## Decision

The bootstrap seed is **ephemeral**: it is a short-lived, single-network bootstrap credential. Once
the mesh has formed on per-peer identity keys ([decision 10](10-identity-handoff-and-mtls.md)), the
seed can be **rotated/revoked**, so a later leak of the old seed cannot join. Admission of new
identities is gossiped and can be gated by existing members.

Rejected: a long-lived indefinitely-valid join secret (simpler ops, but a leaked seed is a permanent
join capability). A configurable long-lived mode may be added later if operationally needed, but the
**default is ephemeral**.

## Why

- Matches the owner's model: *"the secret key is ephemeral; once the services have used the secret key
  to connect they negotiate a proper key process to move to more secure communication."*
- The seed's whole purpose is to weakly get a node *in*; after that, per-peer identity keys carry
  security. Keeping the seed valid forever would leave a permanent broad-authority credential — the
  opposite of the bounded-window property the handoff is designed to give.

## What we do

### TTL & rotation

- A minted token may carry `expires_at` ([decision 04](04-bootstrap-token-envelope.md)); after it,
  the TLS-PSK derived from that seed is refused by members. Absent an explicit TTL, a network-level
  default TTL applies.
- **Seed rotation:** an operator (or the deployment platform) can mint a **new** seed for the
  network; members accept the new `tls_psk` and, after a grace period, drop the old one. Rotation is
  gossiped as a network-config change signed by an existing member's identity key.
- **Revocation:** a seed can be revoked immediately (before TTL) by gossiping a revocation of that
  seed's `tls_psk` id; subsequent bootstraps with it fail.

### Admission control

- Because membership is masterless, "admission" is a **policy the existing members apply** at `Join`
  time, not a central gate. Modes (config-selectable):
  - **open** (default for ephemeral seed): any node presenting a valid, unexpired seed is admitted —
    the seed *is* the authorization.
  - **approved:** a new identity is added in a `pending` state and requires an existing member's
    signed approval to become `alive` (for higher-trust networks).
- The `Join` RPC ([decision 06](06-hybrid-transport-tls-psk.md)) returns admit/pending/reject.

### Persistence interaction

- A node persists its **identity** keypair (not the seed) so restarts rejoin as the same identity
  even after the seed is gone. A node that has completed handoff **does not need the seed again**.

## Consequences

- If the seed expires/rotates while some node has not yet completed handoff, that node may be locked
  out and must be re-seeded. The mesh layer completes handoff **promptly** after join to minimize
  this window, and the default TTL is generous relative to handoff time.
- Ephemeral-by-default is safer but means operators must mint a fresh seed to add nodes after
  expiry — acceptable and, for the Docker path, automated
  ([feature 10](../features/10-deployment-platform-integration/feature.md)).

## Related decisions

- [03 — Seed-derived keys](03-seed-derived-keys.md)
- [04 — Bootstrap token envelope](04-bootstrap-token-envelope.md)
- [10 — Identity handoff & mTLS](10-identity-handoff-and-mtls.md)
