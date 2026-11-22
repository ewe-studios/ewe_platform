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
source named two types. The owner reviewed the same pattern in spec 77 (the part wrappers)
and asked for the whole set to be checked.

The variant now exists on every target. A `PathBuf` is portable; only the file read is not.
`PrivateKey::load` returns the new `KeyLoadError::NoFileSystem` when a wasm host is given a
`File` source. The constructors stay gated, because they are capabilities: `from_env_var` is
native only, and `from_worker_secret` is wasm only. A gated method is not a shape change.

The rule: a `cfg` goes on a private inner type, a method, a module or a dependency. It never
changes the variants of a public enum.

## What the T5, T7 and T11 work taught (2026-11-23)

### The version 1 golden fixture is made with plain OpenSSL

The design asks for a version 1 secret made by the branch code. The branch code
(`foundation_agent_rpc/src/secret.rs`) is on this tree, so its signed byte string is the
authority. The fixture proves the format from outside our Rust code: `openssl genpkey` makes the
Ed25519 key, `python3` builds the exact byte string, and `openssl pkeyutl -sign -rawin` signs it.
`openssl pkeyutl -verify` also checked the signature before the fixture went in. A change that
breaks the byte order or a length prefix therefore fails against an external signer, not only
against our own signer.

### The domain is not on the wire, so a client helper must name it

`SecretIssuer` and `SecretVerifier` know their `SecretDomain`; the envelope does not carry it.
The design's `SignedSecret::verify_self_signed()` has no domain argument, but an agent RPC client
and a proxy client expect different domains. The code keeps `verify_self_signed()` as the agent
RPC default and adds `verify_self_signed_for(domain)`.

### `Env` has no clock

The design sketches `SecretIssuer { env: Env }` and `self.env.unix_secs()`. `Env` (spec 84)
deliberately holds no clock: the clock is an engine-minimum element of `Ctx`. The issuer and the
verifier take `Ctx` and read `ctx.clock()`; entropy still comes from `ctx.require_env()`. A
missing `Env` is `SecretError::Entropy`, never a fallback to `foundation_compact::entropy::fill`
(the effect lint forbids that call).

### PKCS#8 version 2 is 83 bytes, and its stored public key is checked

`ring` writes an Ed25519 PKCS#8 key as `30 51 ... | seed | 81 21 00 | public key`. The parser
checks all three parts and refuses a file whose stored public key does not match the seed. The
version 1 form (48 bytes) is what `ed25519-compact` and OpenSSL write.

### SEC1 parsing wants the original PEM block

`p256::SecretKey::from_sec1_pem` uses the `pem` crate, which rejected a rebuilt single-line PEM
even though the base64 was correct. The fix is to pass the original
`-----BEGIN EC PRIVATE KEY-----` block with its line breaks. An `openssl ecparam` file also holds
an `EC PARAMETERS` block first, so the parser searches for the private-key block, not the file
start.

### The in-memory VFS does not create parent directories

`MemoryFs::create_async` returns `not found` when the parent directory is absent. A key-loading
test must call `mkdir_all_async` first. This also showed that `PrivateKey::load` correctly maps
the ordinary `VfsError::NotFound` to `KeyLoadError::NotFound`, with no special "no file system"
error (auth.md section 4.4).

### `expect` and `expect_err` need `Debug` on the other type

`Result::expect` needs `Ok: Debug`; `Result::expect_err` needs `Err: Debug`. `UnaryReply`
implements neither. Use `.is_ok()` with a message, or `.err().expect(..)`, in a test of an RPC
interceptor.

### The interceptor reads the call context, not `UnaryCall`

`ClientAuthInner::check` builds `CallCredentials` from `ctx.request.headers`, because that is the
per-call state a real dispatcher fills. A test must put the header on the `RpcCtx`, not only on
the `UnaryCall` frame wrapper.

### RSA-PSS needs injected randomness

RSA-PSS signing is randomized. `ring` takes a `SecureRandom`. The design does not say where that
randomness comes from, and root rule 11 forbids the system global. A correct `secret-rsa`
implementation must capture an `Arc<dyn EntropySource>` from the `Ctx` at construction. This is
why the feature was left out of this change rather than half-built (issue #761).

## RSA-PSS with the `rsa` crate (2026-11-23, issue #761)

The design named `ring` behind `secret-rsa`. `ring` seals its `SecureRandom` trait
(`ring::rand::sealed::SecureRandom`), so a caller cannot implement it. The only random source
that `ring` accepts is `SystemRandom`, which reads the operating system. That breaks root rule
11: the salt must come from the injected `Ctx`.

The resolution:

- Use the pure-Rust `rsa` crate (0.9). It takes a caller-supplied `CryptoRngCore`, so the salt
  comes from us.
- Draw 32 bytes from `ctx.require_env()?.entropy()`, seed a `rand_chacha` 0.3 `ChaCha12Rng`
  with them, and give the stream to `rsa`. The one fallible step (the seed fill) returns
  `SecretError::Entropy`. The stream then serves every byte that signing asks for.
- `rand_chacha` 0.3 is the version that uses `rand_core` 0.6, the version that `rsa` 0.9 uses.
  The workspace has a second `rand_core` through `foundation_compact` (0.10); the two do not
  mix.
- The feature stays native only. A wasm build never compiles the RSA code, and
  `verify_rsa_pss` returns `SecretError::UnsupportedAlgorithm`.

The test `rsa_signing_draws_from_the_injected_entropy` proves the point: two issuers with the
same seeded `Ctx` make byte-identical secrets, including the randomized signature. A system
random source would differ on every run.

A lesson: a design that names a crate must also check that the crate accepts an injected effect.
A sealed trait is a hard stop, and it is cheaper to find it before the plan than mid-change.

## A client-id index is not an optimisation (2026-11-23, issue #762)

The trust store keyed clients by certificate fingerprint only. An access secret from enrollment
or refresh carries `sub` = the `client_id`, and the design requires a revocation check on that
ID. Without a lookup keyed by `client_id`, a revoked client-key client keeps calling until its
access secret expires.

The fix adds `TrustedClient::client_id`, `TrustStore::client_by_id` on all three backends, and
`TrustedClients::check_client_not_revoked`. The SQL store keeps the ID in a new
`trusted_client_ids` table. An existing `trusted-clients.db` keeps its old tables and needs
no `ALTER TABLE`. The check reads the store on each call, like the secret revocation check, so a
revocation stops the next call. The in-memory index is keyed by fingerprint and cannot answer
this question.

An unknown `client_id` is refused with `AuthError::Invalid`, not accepted. "The client must
still be enrolled and not revoked" is a positive condition.

