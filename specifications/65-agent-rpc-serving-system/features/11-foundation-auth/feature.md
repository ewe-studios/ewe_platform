---
feature: "Auth in foundation_auth"
description: "Signed secrets (v1 and v2), operator key files (proof of concept), the CA and certificates, the trusted-client store, rustls verifiers, and the pluggable ClientAuth providers with the connectrpc interceptor, all in foundation_auth."
status: "pending"
priority: "high"
depends_on: ["08-env-and-side-effects", "09-transport-prerequisites", "10-migration-groups"]
part_of: "07-pluggable-agent-rpc"
estimated_effort: "large"
created: 2026-10-27
last_updated: 2026-10-27
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0
---

# Auth in foundation_auth

_Split out of [feature 07](../07-pluggable-agent-rpc/feature.md) on 2026-10-27 (owner direction:
feature 07 was too large). Written in ASD-STE100 Simplified Technical English (STE). Code and
identifiers stay as they are._

The design of this feature is in the topic files of feature 07. They stay in one place, because
they link to each other. This file says what to build, what to read, and when it is done. Task
IDs keep their feature 07 numbers, so references such as "task T5" stay valid. The
[glossary in feature 07](../07-pluggable-agent-rpc/feature.md#2-glossary) explains each term.

## 1. What this feature does

The auth code moves out of agent-rpc into `foundation_auth`, on top of `foundation_db`, so other services can use it. The proof of concept loads operator key files (decision 11); key protection at rest is [feature 15](../15-key-protection-at-rest/feature.md).

Part of the agent-rpc proof of concept (features 07 to 13).

## 2. What to read

| Design file | Sections |
|---|---|
| [auth.md](../07-pluggable-agent-rpc/auth.md) | the whole file; section 4 for the operator key files |
| [feature 07 `feature.md`](../07-pluggable-agent-rpc/feature.md) | architecture (section 3), owner decisions (section 11) |
| [feature 07 `start.md`](../07-pluggable-agent-rpc/start.md) | the workflow and the rules to check before each push |

## 3. Tasks

| Task | What | Main files | Done when |
|---|---|---|---|
| T5 | `foundation_auth::secret`: `SignedSecret` v1 and v2, `SigningKey`, `VerifyingKey`, `SecretIssuer`, `SecretVerifier`; the clippy lint file | `foundation_auth/src/secret/*` | v1 secrets from the branch tests verify unchanged; verify-order tests pass |
| T7 | `foundation_auth::keys`: `KeySource`, `PrivateKey` (OpenSSH, PKCS#8 and SEC1 PEM), `KeyLoadError`, `VerifyingKey::list_from_json`; test keys from `SeededEntropy` | `foundation_auth/src/keys/*` | the key loading tests of [auth.md](../07-pluggable-agent-rpc/auth.md) section 10 pass |
| T8 | `foundation_auth::pki` (native): CA, server identity, CSR signing with `Env` time and randomness | `foundation_auth/src/pki/*` | same seed gives byte-identical certificates |
| T9 | `foundation_auth::trust`: `TrustStore`, `SqlTrustStore`, `KvTrustStore`, `TrustedClients` | `foundation_auth/src/trust/*` | `TrustStore` contract and concurrent-use tests pass |
| T10 | `foundation_auth::mtls` (native): pure `TrustedClientVerifier`, `PinnedServerVerifier`, rustls config builders | `foundation_auth/src/mtls/*` | verifier tests show zero store calls during a handshake |
| T11 | `foundation_auth::client_auth`: `ClientAuth`, providers, `ClientAuthInterceptor`, presets | `foundation_auth/src/client_auth/*` | provider tests pass with `SimClock` |

## 4. Done when

Version 1 secrets from the branch tests verify unchanged. The key loading, `TrustStore` contract, verifier and provider tests of [auth.md](../07-pluggable-agent-rpc/auth.md) section 10 pass. The same seed gives byte-identical certificates.

## 5. Pull requests

One PR, or two: T5 + T7 + T8 first, then T9 + T10 + T11 (was PR group D).
