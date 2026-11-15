# LEARNINGS — Foundation Auth

## Why auth left spec 83 (2026-11-14)

The proxy registration authority (spec 82) and the agent RPC server (spec 83) use the same auth
primitives. Both need signed secrets, key loading, a CA, a trust store and mTLS verifiers. If the
library stayed inside the agent RPC spec, the proxy spec would depend on the agent RPC spec.

`foundation_auth` already holds the application auth modules (OAuth, OIDC, JWT, sessions, IdP).
This spec adds the server credential modules; it does not replace the existing ones.

## Why the primitives and mTLS left this specification (2026-11-15)

This specification was two things in one plan: a set of security primitives, and the server
credentials that use them. The owner review of 2026-11-15 separated them, because the primitives
have consumers that are not agent RPC servers:

1. **The primitives** (`Secret`, `ClientAuth`, `AuthInfo`, `CallCredentials`, the bearer parser,
   the constant-time compare) moved to spec 86. The sandbox server of spec 77 needs a secret and
   a bearer check, and it must not depend on the agent RPC stack. It also removed the risk of
   two `Secret` types in one workspace.
2. **Mutual TLS** (the CA, the trust store, the verifiers, the mTLS providers) moved to spec 87.
   Spec 77 had deferred mTLS to spec 65, which became spec 83 and does not own a CA. No task
   here needed the CA, so nothing would schedule it.

This specification now owns what is left: signed secrets, operator key files, three providers
and the interceptor. It builds on both specifications and does not edit their modules.

The lesson: a specification about a library also needs a boundary *inside* the library. When one
table row has its own consumers and its own schedule, it is a specification, not a row.

## The two links

1. Client ↔ agent-rpc: signed secret, or mTLS on a direct connection. The server checks it.
2. Sidecar → proxy: enrollment token once, then mTLS. The proxy checks it.

Neither link sees the other's credential. The library serves both; it does not decide policy.

## Reuse the existing primitives

- Entropy and IDs come from `Env` (spec 84). Do not call `thread_rng`, `getrandom` or a global
  clock.
- The redacted string is `Secret` (spec 86). Do not add a second one.
- Compare a secret with `Secret::ct_eq` or `constant_time_eq`, never with `==`.
- PEM parsing and key files: accept `ssh-keygen` and `openssl` output.
- `ring`'s sealed random source cannot be made deterministic. Use the pure-Rust key crates and
  RFC 6979 nonces where DST requires replay.

## A public type keeps one shape on every target (2026-11-15)

`KeySource::File(PathBuf)` was a `#[cfg(not(target_family = "wasm"))]` variant, so the public
enum had two shapes. A wasm caller could not write it and could not match it, and the same
source named two types. The owner reviewed the same pattern in spec 77 (`RuntimeFetch`) and
asked for the whole set to be checked.

The variant now exists on every target. A `PathBuf` is portable; only the file read is not.
`PrivateKey::load` returns the new `KeyLoadError::NoFileSystem` when a wasm host is given a
`File` source. The constructors stay gated, because they are capabilities: `from_env_var` is
native only, and `from_worker_secret` is wasm only. A gated method is not a shape change.

The rule: a `cfg` goes on a private inner type, a method, a module or a dependency. It never
changes the variants of a public enum.
