# LEARNINGS — Foundation Auth

## Why auth left spec 83 (2026-11-14)

The proxy registration authority (spec 82) and the agent RPC server (spec 83) use the same auth
primitives. Both need signed secrets, key loading, a CA, a trust store and mTLS verifiers. If the
library stayed inside the agent RPC spec, the proxy spec would depend on the agent RPC spec.

`foundation_auth` already holds the application auth modules (OAuth, OIDC, JWT, sessions, IdP).
This spec adds the server credential modules; it does not replace the existing ones.

## The two links

1. Client ↔ agent-rpc: signed secret, or mTLS on a direct connection. The server checks it.
2. Sidecar → proxy: enrollment token once, then mTLS. The proxy checks it.

Neither link sees the other's credential. The library serves both; it does not decide policy.

## Reuse the existing primitives

- Entropy and IDs come from `Env` (spec 80). Do not call `thread_rng`, `getrandom` or a global
  clock.
- PEM parsing and key files: accept `ssh-keygen` and `openssl` output.
- `ring`'s sealed random source cannot be made deterministic. Use the pure-Rust key crates and
  RFC 6979 nonces where DST requires replay.
