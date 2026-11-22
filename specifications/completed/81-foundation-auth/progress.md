# Spec 81 Progress: Foundation Auth

_Last updated: 2026-11-23._

## Status at a glance

| Feature | State | Source |
|---|---|---|
| 00 — foundation auth | ✅ Complete: T5, T7 and T11 verified; issues #761 and #762 fixed | Merged on 2026-11-23; moved to `specifications/completed` |

## Follow-up change on 2026-11-23 (issues #761 and #762)

| Issue | What landed | Files |
|---|---|---|
| #761 | The `secret-rsa` feature: RSA-PSS with SHA-256 keys of 2048 to 8192 bits, native only. The salt comes from the `Ctx` entropy source. A build without the feature returns `SecretError::UnsupportedAlgorithm` from verify and `BadKey` from a key parse. | `src/secret/keys.rs`, `src/secret/issuer.rs`, `src/keys/parse.rs`, `src/keys/mod.rs`, `Cargo.toml` |
| #762 | A `client_id` on `TrustedClient`, a `TrustStore::client_by_id` method on all three backends, and `TrustedClients::check_client_not_revoked`. `SignedSecretAuth` calls it for an access secret with `sub`. | `src/trust/`, `src/client_auth/signed_secret.rs`, `tests/client_auth/signed_secret.rs`, `tests/mtls/trust.rs` |

The crate version is 0.0.12. Every consumer requires 0.0.12.

**Dependency deviation from auth.md.** The design named `ring` for `secret-rsa`. `ring` seals
its `SecureRandom` trait, so a caller cannot supply the entropy of `Ctx`; its only source is
the operating system. The implementation uses the pure-Rust `rsa` crate, which takes a
caller-supplied random source, and `rand_chacha` 0.3 to expand 32 bytes from the `Ctx`
`EntropySource`. The feature stays native only and has the same wire format.

## What landed on 2026-11-23 (first change)

| Task | What | Files |
|---|---|---|
| T5 | `SignedSecret` v1 and v2, `SigningKey`, `VerifyingKey`, `SecretIssuer`, `SecretVerifier`, `DecodeLimits`, `SecretError`, the golden version 1 fixture | `backends/foundation_auth/src/secret/`, `src/fingerprint.rs`, `tests/secret/`, `tests/fixtures/` |
| T7 | `KeySource`, `PrivateKey` (OpenSSH, PKCS#8, SEC1), `KeyLoadError`, `VerifyingKey::list_from_json` | `backends/foundation_auth/src/keys/`, `tests/keys/` |
| T11 | `SignedSecretAuth`, `JwtAuth`, `InsecureNoAuth`, `ForwardedPrincipalAuth`, `AnyOf`, the provider set, `ClientAuthInterceptor` | `backends/foundation_auth/src/client_auth/`, `backends/foundation_connectrpc/src/shared/auth.rs`, `tests/client_auth/`, `foundation_connectrpc/tests/client_auth_interceptor_tests.rs` |

Verified commands:

```text
cargo test -p foundation_auth
cargo test -p foundation_auth --features secret-rsa --test secret_tests
cargo test -p foundation_auth --features secret-rsa --test keys_tests rsa
cargo test -p foundation_auth --test client_auth_tests signed_secret
cargo test -p foundation_auth --test mtls_tests trust
cargo check -p foundation_auth --target wasm32-unknown-unknown --no-default-features --features wasm,secret
cargo fmt --all -- --check
cargo clippy -p foundation_auth --all-targets --no-deps -- -D warnings
cargo clippy -p foundation_auth --all-targets --no-deps --features secret-rsa -- -D warnings
cargo check -p foundation_agent_rpc
cargo check -p foundation_connectrpc --test client_auth_interceptor_tests
```

## Decisions taken during implementation

| Design point | Implemented as | Why |
|---|---|---|
| `SecretIssuer` and `SecretVerifier` take `Env` | They take `Ctx` | `Env` holds no clock (spec 84); the verifier needs the clock, and root rule 11 says code takes a `Ctx`. Entropy still comes from `ctx.require_env()` and is a typed error when absent. |
| `SignedSecret` has no `subject` field in the sketch | Added `subject: Option<String>` and `SecretRequest::with_subject` | Version 2 carries `sub`, and `SignedSecretAuth` reads it. The field is additive. |
| `signed_message(domain, ..)` needs the domain | `SignedSecret::verify_self_signed()` uses `AGENT_RPC`; `verify_self_signed_for(domain)` takes any domain | The domain is not on the wire. A client helper must name its domain. |
| The interceptor is in `foundation_connectrpc` | Implemented there, under feature `auth` | auth.md section 8.2 puts it there; `foundation_auth` cannot depend on `foundation_connectrpc` without a cycle. |
| RSA-PSS (`secret-rsa`) | Implemented with `rsa` + `rand_chacha`, native only, entropy from `Ctx` | The design named `ring`, but `ring` seals its random source to the operating system, which root rule 11 forbids. |
| Client revocation by `sub` | Implemented with a client-id index in the trust store | The store had no client-id lookup; #762 is fixed. |
| `TrustedClient` fields | Added `client_id: Option<String>` with `#[serde(default)]` | The `sub` check needs the ID. The default reads an old key-value record. The SQL store keeps the ID in a new table, so an existing database keeps its tables. |

## Findings filed

| Issue | Severity | Summary |
|---|---|---|
| #761 | medium | The designed `secret-rsa` feature (RSA-PSS keys) is not implemented. **Fixed and closed.** |
| #762 | high | `SignedSecretAuth` cannot check that the `sub` client is revoked. **Fixed and closed.** |
| #763 | low | `p256` and `ssh-key` are declared in more than one crate instead of the root workspace. |
| #764 | low | `KeySource::from_worker_secret` is absent; use `KeySource::from_value` in the Worker host. |

## What this spec owns

The library, not its consumers, and only the part that no other specification owns:

| Module | Tasks |
|---|---|
| Signed secrets and key files | T5, T7 |
| The signed-secret, JWT and plaintext providers | T11 (their tasks) |
| The connectrpc interceptor and the Connect code map | T11 |

## What left this specification (2026-11-15)

The owner reviewed six conflicts on 2026-11-15. Two of them moved work out of this
specification:

| Item | New home | Why |
|---|---|---|
| The `ClientAuth` trait, `AuthInfo`, `CallCredentials`, `Principal`, `AuthMethod`, `AuthError`, `Secret`, `constant_time_eq`, the bearer parser | [Spec 86](../completed/86-auth-primitives/requirements.md) | The sandbox server (spec 77) needs a secret and a bearer check without the agent RPC stack. One copy serves all. |
| `CaStore`, `CertificateAuthority`, `TrustStore`, `ClientCertVerifier`, `ServerTlsConfig`, `MtlsConfig`, `MtlsAuth`, `CfTlsClientAuth`, `CfMtlsAuth` | [Spec 87](../completed/87-mutual-tls/requirements.md) | Mutual TLS must be scheduled and delivered before its consumers. No task in this feature needed it. |

`auth.md` now points to both specifications from sections 5, 6, 7, 8.1 and 8.3.

## What stays out

Key protection at rest (keystore, KEK, WebCrypto, remote KMS, rotation) stays in spec 83
feature 08. This library accepts operator key files for the proof of concept.

## Completion (2026-11-23)

The main agent reviewed the change. Every test suite passes. The merge into master is complete.
The spec is in `specifications/completed/81-foundation-auth`.

## Next steps

1. The consumers (spec 83 host wiring, spec 82 proxy registration) build the provider set from
   this crate and spec 87.

## Open questions

See `features/00-foundation-auth/feature.md` and `auth.md`.

## Key source note (2026-11-23)

The 2026-11-15 note below said `KeySource::File(PathBuf)` and `KeyLoadError::NoFileSystem`.
The authority, auth.md section 4.4, says `File(String)` over the `foundation_nativeapis` VFS and
"there is no `NoFileSystem` error". The code follows auth.md: a host with no file system gets
the ordinary `VfsError::NotFound`, which maps to `KeyLoadError::NotFound`. The enum keeps one
shape on every target.

## Documentation correction (2026-11-15)

- **`KeySource` (auth.md section 4.4).** `File(PathBuf)` was a `cfg(not(wasm))` variant, so the
  public enum changed shape per target. The variant now exists on every target, and
  `PrivateKey::load` returns the new `KeyLoadError::NoFileSystem` on a wasm host. The
  constructors stay target-gated: `from_env_var` is native only, `from_worker_secret` is wasm
  only.

No code changed. The specification waits for the owner review. Issue #556 records it, and it
is closed.
