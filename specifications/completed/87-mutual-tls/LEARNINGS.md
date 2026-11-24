# LEARNINGS — Mutual TLS

## Why mutual TLS gets its own specification (2026-11-15)

Three problems caused this split:

1. **The work had no schedule.** Spec 77 wrote "mutual TLS is spec 65's job". Spec 65 became
   spec 83, and spec 83 does not own a CA. The row moved to spec 81, but no task in any
   specification read it. A capability that no task needs is a capability that never lands.
2. **The unit was too large.** Spec 81 feature 00 has twelve task groups. A consumer that needs
   only the CA chain check would wait for the signed-secret verifier, the key files and the
   provider set of that feature.
3. **Two consumers wait on it.** The proxy (spec 82) and the agent RPC server (spec 83) both
   need mTLS, and the sandbox service (spec 77) names it as an option for its control plane.

The split gives mTLS a task list, a delivery order and an owner. The consumers build on the
library and do not re-plan it.

## Why the design moved and was not rewritten (2026-11-15)

`auth.md` sections 5, 6 and 7 already described the CA, the trust store and the verifier at the
right depth. A rewrite would lose detail and would create two versions of the algorithm.

So `mtls.md` holds the text unchanged. It gets a numbering that starts at 1, and a Revision 2
table that names every substitution. A reader can compare a sentence in `mtls.md` with the
same sentence in the old spec 81 text.

The one real change is the time source. The design said "the current time". It now says "the
`Ctx` clock" (spec 84). This is not cosmetic: a certificate validity window is a time bound,
and a deterministic simulation test must be able to cross it.

## Why the chain check and the trust check are separate (2026-11-15)

The handshake proves three facts: the certificate is signed by the CA, it is inside its
validity window, and the client holds the private key. It does not prove that the client may
call.

A trust store answers the second question. Without it, every client that has any certificate
from the CA has the same rights: the same roles, the same limits, the same tenant. That is
wrong for a fleet with many agents and many proxies.

So the design keeps two checks in two places:

| Check | Where | When | Cost |
|---|---|---|---|
| Chain, validity, key possession | `ClientCertVerifier`, in the handshake | Once per connection | one signature check |
| Enrollment, revocation, limits | `MtlsAuth`, per call | Every call | one store read, usually cached |

The second check per call is the reason a revocation stops a live keep-alive connection. A
handshake check alone would keep the connection open until it closes.

## Why `CfMtlsAuth` exists and `MtlsAuth` cannot serve a Worker (2026-11-15)

Cloudflare ends TLS at its edge. The Worker never sees the handshake and never sees the chain.
It sees the certificate facts in `request.cf.tlsClientAuth`:

| Fact | Meaning |
|---|---|
| `cert_presented` | The client showed a certificate. |
| `cert_verified` | `SUCCESS` or a failure code. |
| `cert_fingerprint_sha256` | SHA-256 of the certificate, lowercase hexadecimal. |
| `cert_issuer_dn` | The DN of the issuer. |
| `cert_subject_dn` | The DN of the subject. |
| `cert_not_after`, `cert_serial` | The end of the validity window and the serial number. |

So the Worker cannot check a signature. It trusts Cloudflare's verdict and then checks the
trust store by fingerprint, exactly as `MtlsAuth` does. `CfMtlsAuth` does that, and the two
providers share the trust-store lookup.

This is the reason a spec 77 sandbox on a Worker has two options for its control plane: a
bearer secret, or Cloudflare client certificates. A Worker cannot run its own mTLS listener.

## Why the verifier reads the clock and not `SystemTime::now()` (2026-11-15)

`rustls` asks the verifier for the current time to compare against `not_before` and
`not_after`. The default is the system clock. A test that wants "the certificate expired one
second ago" cannot set the system clock.

The verifier takes the time from `Ctx`. A DST run then controls the validity window with the
same simulated clock that it uses for timers. This is root rule 11 applied to a TLS
certificate.

The same clock serves `TrustedClients`: the last-seen write and the client expiry.

## Why two key algorithms (2026-11-15)

A test needs a certificate that is the same on two runs. ECDSA P-256 signatures use a random
nonce, so the certificate bytes differ. Ed25519 signatures are deterministic.

| Algorithm | Deterministic | Browser accepts as a client certificate | Browser accepts as a server certificate |
|---|---|---|---|
| ECDSA P-256 | No, unless the nonce is fixed | Yes | Yes |
| Ed25519 | Yes | Yes | **No** |

So a DST run uses Ed25519 for the client certificate and either a P-256 server certificate
with a fixed nonce or no TLS at all. The P-256 fixed-nonce mode is for a test only: a reused
nonce leaks the private key.

## Dead end: a separate `foundation_mtls` crate

The first idea was a new crate. It fails on one point: `MtlsAuth` implements `ClientAuth`, and
`ClientAuth` takes `AuthInfo`, `CallCredentials` and `Secret`. All three are in
`foundation_auth`. A new crate would depend on `foundation_auth` only for those types, and the
workspace would then have two crates that describe authentication.

So the code stays in `foundation_auth`, in four modules. The specification is the boundary, not
the crate.

## Dead end: keep the code in `foundation_agent_rpc`

`foundation_agent_rpc` already has the CA, the trust store and the verifier. Keeping them there
looks cheaper: no move, no version bump.

It fails for two reasons:

1. The proxy (spec 82) would depend on the agent RPC crate for a CA. That is the wrong
   direction: the proxy does not serve agent RPC.
2. The sandbox service (spec 77) would depend on the agent RPC crate for a listener option.
   The owner rule of 2026-11-15 is that the sandbox does not know about the agent RPC stack.

So the code moves to `foundation_auth`, and `foundation_agent_rpc` builds on it.

## What a consumer must not do

1. Write a CA, a trust store or a verifier. Use `foundation_auth::pki`, `::trust` and `::mtls`.
2. Accept a CA chain as proof of the right to call. Check the trust store.
3. Read `SystemTime::now()` for a validity window. Use the `Ctx` clock through the verifier.
4. Compare two fingerprints with `==`. Use `constant_time_eq` from spec 86.
5. Run mTLS on a Worker. Use `CfMtlsAuth` and Cloudflare client certificates.
6. Keep a certificate, a key or a token in an error message. `AuthError` has the reason only.

## Implementation learnings (2026-11-22)

### A remote `rcgen` key pair cannot be written to a file

The design signs P-256 with RFC 6979 through `rcgen::RemoteKeyPair`. The first
implementation followed it. `ca_round_trip` then failed: `rcgen` panics when a
remote key pair is serialized (`serialize_pem` calls `serialize_der`). The CA
must survive a restart, so a key that cannot be written is not usable.

The fix keeps determinism and serialization: an Ed25519 key comes from a
32-byte seed drawn from the `Ctx` random number generator, is wrapped in the
48-byte PKCS#8 version 1 structure, and is loaded with
`KeyPair::from_pkcs8_der_and_sign_algo`. The key is deterministic and the PEM
round-trips. Ed25519 signatures have no nonce (RFC 8032), so two runs with the
same seed make the same certificate bytes. P-256 stays the default for a real
host and keeps its random nonce.

### The serial number is already deterministic

`rcgen` 0.13 writes a serial number from the SHA-256 of the public key when
`serial_number` is `None`. No extra code is needed for a stable serial.

### `x509-parser` follows `rcgen`

`rcgen` 0.13.2 depends on `x509-parser` 0.16. Declaring 0.17 would put two
versions of the parser in `Cargo.lock`, against root rule 17. The workspace
declares 0.16 and records the reason.

### `builder_with_details` is the clock injection point

`rustls` takes the time source in `ServerConfig::builder_with_details` and
`ClientConfig::builder_with_details`. The WebPKI verifier and the custom
verifier then receive `now` from that source. The provider is passed
explicitly, so no library installs a process-global provider.

### A generic store over `dyn Trait` needs `?Sized + 'static`

`SqlTrustStore<Q: QueryStore + ?Sized>` lets a caller hold
`SqlTrustStore<dyn QueryStore>`. A boxed future that borrows the store also
needs `Q: 'static`, because `TrustStore` requires `'static`. The bound is on
the struct and every impl.

### The trait's `is_secret_revoked` owns its identifier

The first signature tied `&self` and `secret_id` to one lifetime. Clippy then
asked to elide the lifetime, and the elided form does not compile: the boxed
future may outlive the identifier. Copying the identifier into a `String`
before the box makes the future borrow only `self`, and the elided signature
is correct.

### The spec's clippy command lints path dependencies

`cargo clippy -p foundation_auth -p foundation_agent_rpc -- -D warnings`
compiles and lints every path dependency. It fails on `foundation_jsonschema`,
which is one of the 13 packages in open issue #441. Use `--no-deps`, as
`.agents/AGENTS.md` says; that run is clean.

### Moving last-seen changes the host

The old verifier started a task that wrote last-seen during the handshake. The
design moves that write to `MtlsAuth` on the call path, because a handshake
cannot see a revocation on a live connection. The consequence is real: until
spec 83 feature 04 installs the provider, the agent RPC listener records no
last-seen time. The `auth_tests` suite now states this, so the gap is visible
and not silent.
