# 04 — Bootstrap Token Envelope (one parser, two forms)

**Date:** 2026-07-12
**Status:** Resolved (owner-confirmed)

## Decision

A single parser — `WgBootstrap::parse(&str)` — accepts **either** form:

1. **Self-contained token** — one opaque, versioned, checksummed blob packing
   `{ network_id, seed (or bootstrap pubkey), endpoint(s), flags }`. Paste it and a service can dial
   immediately with **zero** other config.
2. **Bare seed + separate endpoint** — the pasted value is *only* the seed; the endpoint(s) and
   network id arrive out-of-band (env var, DNS, config file).

## Why

Both flows appear in the owner's vision and must work from day one:

- **Human copy-paste / "here is your one join string"** → self-contained token.
- **Platform env-injection** (the Docker path: `foundation_deployment_platform` sets `WG_SECRET` and
  `WG_ENDPOINT` as **separate** env vars — [decision/feature 10](../features/10-deployment-platform-integration/feature.md))
  → bare seed + separate endpoint.
- **Ephemeral/rotating secrets** are cleaner as a tiny bare seed than as a re-minted fat token.

Supporting both behind one parser costs a little format design and buys ergonomics for every caller.

## What we do

### Self-contained token wire format

```
wg1_<base64url( body )>            # "wg1_" = version-tagged, human-greppable prefix
body = varint-tagged fields:
  0x01 network_id      : 16 bytes
  0x02 seed            : 16 | 32 bytes         (mutually exclusive with 0x03)
  0x03 bootstrap_pubkey: 32 bytes              (seedless token: carries pubkey, PSK supplied sep.)
  0x04 endpoint        : repeated {family, ip, port}   (>=1; multiple = seed redundancy)
  0x05 flags           : u16 bitfield (e.g. relay-capable-hint, ephemeral, ipv6-only)
  0x06 expires_at      : optional u64 unix secs (token TTL; see decision 11)
trailer = crc32c(body) : 4 bytes               # typo/truncation guard, NOT security
```

- The `wg1_` prefix versions the format and the KDF constants
  ([decision 03](03-seed-derived-keys.md)); a future `wg2_` can change layout/derivation without
  ambiguity.
- **Multiple endpoints** are encouraged — they are the initial-join seeds and harden the *first*
  contact against a single seed being down ([decision 05](05-swim-full-membership-gossip.md)).
- The CRC is an integrity/typo guard only; **confidentiality/authenticity come from TLS-PSK**
  ([decision 06](06-hybrid-transport-tls-psk.md)), not the token encoding.

### Bare-seed form

`WgBootstrap::parse` detects a bare seed when the input is *not* `wg1_`-prefixed and decodes as a
16/32-byte base64url/hex value; the caller must then supply `network_id` + `endpoint(s)` via the
builder/config/env. Parsing a bare seed with no endpoint available is a **hard error at connect
time**, never a silent localhost default.

### API shape

```rust
pub enum WgBootstrap {
    Token(BootstrapToken),                 // fully self-describing
    Seed { seed: WgSeed, /* endpoint+network filled from env/config */ },
}
impl WgBootstrap {
    pub fn parse(s: &str) -> Result<Self, BootstrapError>;
    pub fn to_token(&self) -> String;      // mint a wg1_ token from a network + endpoints
}
```

## Consequences

- Token length: a seed+one-endpoint token is ~60–90 chars — long but paste-once. The bare-seed form
  is ~22–44 chars for the env path.
- A **seedless** token (`0x03 bootstrap_pubkey`, no `0x02 seed`) exists for the case where the
  minting side does *not* want to hand out the seed's private half — the joiner then needs the PSK
  by another channel. This is a niche/advanced path; the common case ships the seed.

## Related decisions

- [03 — Seed-derived keys](03-seed-derived-keys.md)
- [06 — Hybrid transport & TLS-PSK](06-hybrid-transport-tls-psk.md)
- [11 — Ephemeral seed lifecycle](11-ephemeral-seed-lifecycle.md)
- [13 — Tri-config & macro](13-tri-config-and-macro.md)
