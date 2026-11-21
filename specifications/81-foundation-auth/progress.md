# Spec 81 Progress: Foundation Auth

_Last updated: 2026-11-15._

## Status at a glance

| Feature | State | Source |
|---|---|---|
| 00 — foundation auth | ⏳ 0/6, design complete | Moved from spec 83 feature 11 |

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
| The `ClientAuth` trait, `AuthInfo`, `CallCredentials`, `Principal`, `AuthMethod`, `AuthError`, `Secret`, `constant_time_eq`, the bearer parser | [Spec 86](../86-auth-primitives/requirements.md) | The sandbox server (spec 77) needs a secret and a bearer check without the agent RPC stack. One copy serves all. |
| `CaStore`, `CertificateAuthority`, `TrustStore`, `ClientCertVerifier`, `ServerTlsConfig`, `MtlsConfig`, `MtlsAuth`, `CfTlsClientAuth`, `CfMtlsAuth` | [Spec 87](../87-mutual-tls/requirements.md) | Mutual TLS must be scheduled and delivered before its consumers. No task in this feature needed it. |

`auth.md` now points to both specifications from sections 5, 6, 7, 8.1 and 8.3.

## What stays out

Key protection at rest (keystore, KEK, WebCrypto, remote KMS, rotation) stays in spec 83
feature 08. This library accepts operator key files for the proof of concept.

## Next steps

1. Run the baseline against spec 86 and spec 87. The trait and the mTLS provider must exist
   before this feature starts.
2. Implement T5 and T7 first; the consumers need signed secrets.
3. Wire the provider set: this feature's providers plus the spec 87 mTLS providers.

## Open questions

See `features/00-foundation-auth/feature.md` and `auth.md`.

## Documentation correction (2026-11-15)

- **`KeySource` (auth.md section 4.4).** `File(PathBuf)` was a `cfg(not(wasm))` variant, so the
  public enum changed shape per target. The variant now exists on every target, and
  `PrivateKey::load` returns the new `KeyLoadError::NoFileSystem` on a wasm host. The
  constructors stay target-gated: `from_env_var` is native only, `from_worker_secret` is wasm
  only.

No code changed. The specification waits for the owner review. Issue #556 records it, and it
is closed.
