# Feature 08 — Identity Handoff & Optional mTLS

**Depends on:** 04
**Decisions:** [10](../../decisions/10-identity-handoff-and-mtls.md), [11](../../decisions/11-ephemeral-seed-lifecycle.md), [03](../../decisions/03-seed-derived-keys.md)

## WHY

Move the mesh from the **shared** bootstrap key to **per-peer random identity keys**, retire the
ephemeral seed, and offer **optional** app-layer mTLS — so each node is individually authenticated
and the seed's compromise window is bounded.

## WHAT

`shared::mesh::handoff` + `shared::identity` + optional `native::mtls`:

1. Identity generation + persistence.
2. Handoff sequencing (establish identity tunnels → migrate → retire bootstrap tunnel).
3. Ephemeral-seed retirement/rotation/revocation hooks.
4. Optional `boring`-based app-layer mTLS over the overlay socket.

## HOW ([decision 10](../../decisions/10-identity-handoff-and-mtls.md))

**TODO**: Persistence must be via our VFS from the `foundation_nativeapis, this lets us mount both local and remote (cloudflare r2, sqlite, ..etc) as the store.

- **Generate:** `IdentityKeypair::generate()` (random `OsRng`, **not** seed-derived); persist the
  private half (feature 09 persistence) so restarts keep identity.
- **Announce** the identity pubkey over secured gossip; peers install each other as `WgTunnel` peers
  keyed by identity pubkey + `allowed_ips = tunnel_ip`.
- **Cutover:** establish the per-peer identity tunnel, migrate traffic, then drop the shared bootstrap
  `Tunn` — sequenced so no packets are lost.
- **Seed retirement** ([decision 11](../../decisions/11-ephemeral-seed-lifecycle.md)): after handoff a
  node no longer needs the seed; support seed TTL, operator rotation (gossiped, member-signed), and
  immediate revocation of a seed's `tls_psk` id.
- **Optional mTLS:** `security.mtls = on` wraps overlay sockets in `boring` mTLS, peer identity pinned
  to the WG identity pubkey (raw-public-key TLS or derived self-signed) — no external CA. **Off by
  default** (WG Noise already gives mutual auth + forward secrecy).

## Task list

1. Identity generation + persistence format.
2. Handoff state machine (announce → establish identity tunnels → migrate → retire bootstrap).
3. Seed TTL / rotation / revocation plumbing (ties to feature 02 admission + feature 10).
4. Optional app-layer mTLS wrapper over `OverlayStream`.
5. Tests: after join, peers use identity keys (inspect tunnel key material); revoke seed → new joiner
   with old seed refused; optional mTLS handshake succeeds pinned to identity.

## Test plan / success

- Post-handoff tunnels use per-peer identity keys, not the bootstrap key (success criterion 4).
- Revoked/expired seed blocks a fresh join.
- mTLS (when enabled) authenticates peers by WG identity.
