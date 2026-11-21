# Spec 87 Progress: Mutual TLS

_Last updated: 2026-11-15._

## Status at a glance

| Task | State |
|---|---|
| T1 — `pki`: `CaStore`, `CertificateAuthority`, `CaConfig`, CSR validation, issuance, renewal, deterministic signing | ⬜ Pending |
| T2 — `trust`: `TrustStore`, `SqlTrustStore`, `KvTrustStore`, `MemoryTrustStore`, `TrustedClients`, revocation | ⬜ Pending |
| T3 — `mtls`: `ClientCertVerifier`, `ServerTlsConfig`, `MtlsConfig`, `peer_identity`, the clock read | ⬜ Pending |
| T4 — `MtlsAuth` over `ClientAuth` and the trust store | ⬜ Pending |
| T5 — `CfTlsClientAuth`, `CfMtlsAuth` | ⬜ Pending |
| T6 — Move the CA, trust store and verifier out of `foundation_agent_rpc` | ⬜ Pending |

## No product code exists

This specification is a plan in a pull request for the owner's review. No file under
`backends/` changes until the owner approves it.

## Where this specification came from

The owner reviewed six conflicts on 2026-11-15. This specification is one of them.

Spec 77 deferred mutual TLS to spec 65. Spec 65 was renumbered to spec 83 and no longer owns
mTLS. Spec 81 then owned the CA, the trust store and the verifiers, but only as one row of a
twelve-group feature. No task in any specification depended on that row, so nothing would
schedule it.

Owner direction: "Lets just create a specification focused on mtls and deliverying it - this
way everything else builds ontop of it."

The detailed design is [mtls.md](./mtls.md). It moved from spec 81 feature 00 (`auth.md`)
sections 5, 6, 7 and the mTLS part of 8.3, and it keeps the design unchanged. The Revision 2
table at its head records every substitution:

| Old (spec 81) | New |
|---|---|
| `Env` from `foundation_core` for the time | `ctx.clock()` from spec 76; the `Env` of spec 84 has no clock |
| `ConfidentialText` | `Secret` from spec 86 |
| `principal: Forwarded` | `AuthMethod::Forwarded` from spec 86 |
| Task numbers T8, T9, T10 | The tasks T1, T2, T3 of this specification |

## What is already in the code

| Item | File | Use for this specification |
|---|---|---|
| `TlsInfo` with the peer chain in DER | `foundation_netio/src/shared/context.rs:69` | The input of `MtlsAuth` |
| `ConnectionContext::tls` | `foundation_netio/src/shared/context.rs` | The per-call source of the certificate |
| `CertificateAuthority` | `foundation_agent_rpc/src/auth.rs:66` | Moves to `foundation_auth::pki` (T1, T6) |
| `MtlsConfig` | `foundation_agent_rpc/src/auth.rs:337` | Moves to `foundation_auth::mtls` (T3, T6) |
| `TrustedClientStore` | `foundation_agent_rpc/src/trust.rs:95` | Becomes `TrustStore` (T2, T6) |
| `SignedSecret` | `foundation_agent_rpc/src/secret.rs:427` | Stays in spec 81; the CA part moves |

## Next steps

1. T1 first. The CA has no dependency inside this specification and every later task uses it.
2. T2 next. The trust store is the second input of T4.
3. T3. The verifier needs T1 for the certificate chain and T2 for the trust store.
4. T4. `MtlsAuth` needs T3 and the `ClientAuth` trait of spec 86.
5. T5. The Cloudflare providers need T2 and the `AuthMethod::CloudflareMtls` variant.
6. T6 last. The library must be green before the agent RPC crate builds on it.

## Known issues to record

| Finding | Severity | State |
|---|---|---|
| Spec 77 points mTLS at spec 65, which no longer owns it | medium | This specification owns it; spec 77 points here |
| No task in any specification schedules the CA, trust store or verifier | high | This specification is the schedule |
| A Worker cannot run mTLS, and spec 77 question Q4 does not say so | low | Section 4.2 of `requirements.md` records the answer |

## Open questions

None. Section 12 of `requirements.md` records the five decisions.
