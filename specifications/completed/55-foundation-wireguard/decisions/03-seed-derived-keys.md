# 03 — Seed-Derived Keys ("the 128/256 key")

**Date:** 2026-07-12
**Status:** Resolved (owner-confirmed)

## Decision

The bootstrap "128/256 key" is a **derivation seed**, not a raw WireGuard key. A 128-bit (16-byte)
or 256-bit (32-byte) secret seed is run through a **KDF** to *deterministically* derive the full
x25519 static keypair. **Sharing the small secret shares the identical keypair.**

## Why

- WireGuard keys are x25519, **256-bit** (32 bytes) — 64 hex chars or 44 base64 chars. So "128"
  cannot be a raw key; the only reading in which *"services can share their keys just by the secret …
  I also like this for secure pub+private key sharing"* is literally true is a **seed** that both
  sides expand into the **same** keypair.
- A 128-bit seed is short (~22 base64 chars) and human-copyable; a 256-bit seed is the higher-entropy
  long-lived variant. Both derive a full-strength 256-bit x25519 keypair — seed entropy, not key
  length, is the security parameter.
- Determinism gives us the ephemeral-bootstrap story cleanly: the seed derives a **throwaway
  bootstrap keypair** that every joiner shares; after join, each node generates **fresh random**
  per-peer identity keys ([decision 10](10-identity-handoff-and-mtls.md)).

## What we do

### Derivation

Use **HKDF-BLAKE2s** (BLAKE2 is already a `boringtun` dependency; keeps the crypto surface small)
with strict **domain separation** so one seed yields distinct, non-correlatable keys per purpose:

```
ikm  = seed_bytes                       # 16 or 32 bytes
salt = "foundation_wireguard/v1"        # version-bound
prk  = HKDF-Extract(salt, ikm)

bootstrap_sk   = clamp(HKDF-Expand(prk, "wg-bootstrap-x25519|" || network_id, 32))
bootstrap_psk  =        HKDF-Expand(prk, "wg-bootstrap-psk|"    || network_id, 32)
tls_psk        =        HKDF-Expand(prk, "tls-psk|"             || network_id, 32)
network_id_derived = HKDF-Expand(prk, "network-id", 16)   # when not supplied explicitly
```

- `bootstrap_sk` → `x25519::StaticSecret` (with x25519 clamping) → `bootstrap_pk`. Deterministic:
  all holders of the seed derive the **same** bootstrap keypair.
- `bootstrap_psk` is the WireGuard **preshared key** for bootstrap-phase tunnels (defense-in-depth
  on the Noise handshake).
- `tls_psk` is the external PSK for the TLS bootstrap channel ([decision 06](06-hybrid-transport-tls-psk.md)).
- `network_id` binds every derived value to a specific network, so the same seed used for two
  networks (or two protocol versions via the `salt`) never produces colliding keys.

### Seed representation & entropy

- Seed is encoded in the bootstrap token as **base64url** (no padding). 128-bit minimum (rejected
  below that); **256-bit recommended** for long-lived networks.
- `WgSeed::generate()` uses `rand_core::OsRng` (already pulled by `boringtun`) for 128 or 256 bits.
- **No silent defaults** — a malformed/short seed is a hard error (`feedback_no_silent_defaults`).

### Identity keys (post-join) are NOT seed-derived

Per-peer identity keypairs are generated **randomly** (`OsRng`), never from the seed — that is what
makes each node individually authenticated rather than all-share-one-secret
([decision 10](10-identity-handoff-and-mtls.md)).

## Consequences / security notes

- Anyone holding the seed can derive the bootstrap private key and impersonate a bootstrap peer —
  which is *exactly why the seed is ephemeral* ([decision 11](11-ephemeral-seed-lifecycle.md)) and
  why steady state moves to random per-peer identity keys.
- KDF domain-separation constants and the `salt` version string are **normative** — changing them is
  a wire-breaking change and must bump the token version prefix ([decision 04](04-bootstrap-token-envelope.md)).

## Related decisions

- [04 — Bootstrap token envelope](04-bootstrap-token-envelope.md)
- [10 — Identity handoff & mTLS](10-identity-handoff-and-mtls.md)
- [11 — Ephemeral seed lifecycle](11-ephemeral-seed-lifecycle.md)
