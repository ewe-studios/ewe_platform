# Spec 87 Progress: Mutual TLS

_Last updated: 2026-11-23._

**Completed.** All tasks are done. Merged into master as `0b257b086` (#683).

## Status at a glance

| Task | State | Commit |
|---|---|---|
| T1 — `pki`: `CaStore`, `CertificateAuthority`, `CaConfig`, CSR validation, issuance, renewal, deterministic signing | ✅ Done | `c81ab8ad3` |
| T2 — `trust`: `TrustStore`, `SqlTrustStore`, `KvTrustStore`, `MemoryTrustStore`, `TrustedClients`, revocation | ✅ Done | `4f694cc8a` |
| T3 — `mtls`: `ClientCertVerifier`, `ServerTlsConfig`, `MtlsConfig`, `peer_identity`, the clock read | ✅ Done | `a6a1fb0c8` |
| T4 — `MtlsAuth` over `ClientAuth` and the trust store | ✅ Done | `30cd7b09c` |
| T5 — `CfTlsClientAuth`, `CfMtlsAuth` | ✅ Done | `9ff942b50` |
| T6 — Move the CA, trust store and verifier out of `foundation_agent_rpc` | ✅ Done | `63aa072bc` |

## What landed

### T1 — `foundation_auth::pki` (native)

- `CaStore` with a file backend (`ca-cert.pem`, `ca-key.pem`, owner-only key
  permissions) and a memory backend.
- `CertificateAuthority`: `from_key`, `generate`, `issue_server_identity`,
  `sign_client_csr`, `issue_client`, `renew` and `root_store`.
- `CaConfig` (common name, key algorithm, CA lifetime, backdate) and
  `CertificateRequest` (CSR, client name, lifetime), each with a builder and a
  documented default.
- `KeyAlgorithm::{EcdsaP256, Ed25519}`, `CaKey`, `ServerIdentity`,
  `IssuedCertificate`, `sha256_hex`, `validate_client_name` and
  `generate_csr`.
- Ed25519 keys come from the `Ctx` random number generator, so two runs with
  the same seed produce the same certificate bytes. `rcgen` takes the serial
  number from the public key hash, so it is stable too.

### T2 — `foundation_auth::trust`

- `TrustStore` trait, `TrustedClient`, `SecretUse`, `UseOutcome`,
  `TrustStoreInfo`, `TrustError`, `TrustFuture`, `TrustedClientStream`.
- `MemoryTrustStore`, `SqlTrustStore<Q>` (default table prefix `agent_rpc_`)
  and `KvTrustStore<K>`.
- `TrustedClients` and `TrustIndex`: the store plus an in-memory index, with
  `Preload`, `Refresh` and `StoreOnly` modes.
- `TrustConfig`: `touch_interval` (75 s), `cache_ttl` (5 min),
  `revocation_ttl` (1 h), `prune_interval` (1 h), `expired_grace` (5 min) and
  `shared_store`. Each has a builder and a documented default.
- The SQL use counter stays one atomic statement. `prune` is a separate call.

### T3 — `foundation_auth::mtls` (native)

- `ClientCertVerifier` (`TrustedClientVerifier` is an alias): a WebPKI chain
  check to the CA, then the trusted-client index. Pure, synchronous, no task.
- `CtxTimeProvider`: `rustls` reads the time from the `Ctx` clock.
- `ServerTlsConfig`, `MtlsConfig`, `peer_identity`, `tls_info_of`.
- `server_config_mutual`, `server_config_server_auth`,
  `client_config_pinned`, `client_config_pinned_default`,
  `client_config_mutual` and `crypto_provider`.
- A real `rustls` handshake over loopback Transmission Control Protocol (TCP)
  proves both directions, and a simulated clock crosses the validity window.

### T4 — `MtlsAuth`

- `foundation_auth::client_auth::MtlsAuth` over `ClientAuth` and
  `TrustedClients`.
- It checks the trust store on every call, refuses a revoked or expired
  client, and writes last-seen with `.await`. `SeenErrorPolicy::Fail` is the
  default, so a store error reaches the caller.

### T5 — Cloudflare provider

- `CfTlsClientAuth` and `CfMtlsAuth`. The provider returns `Ok(None)` without
  facts or a certificate, refuses a certificate that Cloudflare did not
  verify, and by default requires the fingerprint in the trust store.

### T6 — `foundation_agent_rpc` builds on the library

- `auth.rs` holds only `MtlsConfig`, `EnrollmentService` and the TLS listener
  builder. The CA, trust store and verifier come from `foundation_auth`.
- `trust.rs` is a re-export plus `TRUSTED_CLIENTS_DB_FILE`.
- `client.rs` uses `foundation_auth::pki::generate_csr` and
  `foundation_auth::mtls::client_config_pinned_default`.
- `secret.rs` uses `foundation_auth::pki::sha256_hex`.
- The crate no longer declares `rcgen`, `rustls`, `sha2` or `time`.
- `foundation_agent_rpc` is 0.0.3.

## Decisions and deviations from `mtls.md`

1. **`PrivateKey` is not implemented.** Spec 81 task T7 owns `PrivateKey`, and
   it has not landed. This specification defines `CaKey` in `pki` instead: a
   key with `generate`, `from_pem`, `to_pem`, `csr_pem` and a redacted
   `Debug`/`Display`. A later change can accept `PrivateKey` without moving
   the CA.
2. **Deterministic signing uses Ed25519, not a `RemoteKeyPair`.** The design
   signs P-256 with RFC 6979 through `rcgen::RemoteKeyPair`. A remote key pair
   cannot be serialized, so `ca_round_trip` (persist and reload) would fail.
   The implementation builds an Ed25519 key from a seeded 32-byte seed and
   loads it through PKCS#8, so the key is both deterministic and writable. P-256
   stays the default for a real host; its signature nonce is random, as the
   design's table says.
3. **`ClientCertVerifier` is the struct name** from `requirements.md` section
   4. `TrustedClientVerifier` remains as an alias for the design's name.
4. **This specification does not fill `ConnectionContext::tls`.** Section 3 of
   `requirements.md` gives that work to spec 80. The provider tests build the
   connection context directly. A host fills it after the handshake.
5. **The agent RPC handshake no longer writes last-seen.** The design moves the
   write to `MtlsAuth` on the call path. The agent RPC server installs the
   provider in spec 83 feature 04; until then its listener does not record
   last-seen. The `a_secret_enrolls_once_then_mtls_is_required` test records
   this.
6. **The exact clippy verify command without `--no-deps` fails on
   `foundation_jsonschema`.** That crate is one of the 13 packages in open
   issue #441. `cargo clippy -p foundation_auth -p foundation_agent_rpc
   --no-deps -- -D warnings` is clean.

## Next steps

1. Spec 83 feature 04 installs `MtlsAuth` on the native call path and fills
   `ConnectionContext::tls` at the listener.
2. Spec 81 reads `foundation_auth::pki::CaKey` for the operator key files.
3. Spec 82 and spec 77 build their listeners on `foundation_auth::mtls`.

## Known issues

| Finding | Severity | State |
|---|---|---|
| Spec 77 pointed mTLS at spec 65, which no longer owns it | medium | Closed by this specification |
| No task scheduled the CA, trust store or verifier | high | Closed by this specification |
| `foundation_jsonschema` pedantic clippy warnings | low | Open issue #441 |
| The agent RPC listener does not write last-seen until spec 83 installs the provider | low | Open issue #682; the write belongs to the provider by design |

## Open questions

None. Section 12 of `requirements.md` records the five decisions.
