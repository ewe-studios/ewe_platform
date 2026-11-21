---
feature: "Auth in foundation_auth"
description: "Signed secrets (v1 and v2), operator key files (proof of concept), the signed-secret, JWT and plaintext ClientAuth providers, and the connectrpc interceptor, all in foundation_auth. The trait and result types come from spec 86; the CA, trust store, verifiers and mTLS providers come from spec 87."
status: "pending"
priority: "high"
depends_on: ["specifications/86-auth-primitives", "specifications/87-mutual-tls", "specifications/80-platform-prerequisites"]
part_of: "specifications/83-agent-rpc-serving-system/agent-rpc-architecture.md"
estimated_effort: "large"
created: 2026-10-27
last_updated: 2026-11-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 3
  total: 3
  completion_percentage: 0
---

# Auth in foundation_auth

_Split out of feature 07 on 2026-10-27 (owner direction: each feature is focused on one
thing). Reduced on 2026-11-15: the primitives moved to spec 86 and mTLS to spec 87. Written in
ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are._

The design files of this feature are in this directory. The shared architecture, glossary and
owner decisions are in [`agent-rpc-architecture.md`](../../../83-agent-rpc-serving-system/agent-rpc-architecture.md). Task
IDs keep their old numbers, so references such as "task T5" stay valid. The
[glossary](../../../83-agent-rpc-serving-system/agent-rpc-architecture.md#2-glossary) explains each term.

## 1. What this feature does

The proof of concept loads operator key files (decision 11); key protection at rest is
[spec 83 feature 08](../../../83-agent-rpc-serving-system/features/08-key-protection-at-rest/feature.md).
The remaining auth code moves out of agent-rpc into `foundation_auth`, on top of
`foundation_db`, so other services can use it.

This feature does not define the auth primitives or mutual TLS:

| Item | Owner |
|---|---|
| `ClientAuth`, `AuthInfo`, `CallCredentials`, `Principal`, `AuthMethod`, `AuthError`, `Secret`, `constant_time_eq`, the bearer parser | [Spec 86](../../../86-auth-primitives/requirements.md) |
| The CA, the trust store, the rustls verifiers, `MtlsAuth`, `CfTlsClientAuth`, `CfMtlsAuth` | [Spec 87](../../../87-mutual-tls/requirements.md) |

It builds on them: the providers here implement the spec 86 trait, and the provider set holds
the spec 87 mTLS providers.

Part of the agent-rpc proof of concept.

## 2. What to read

| Design file | Sections |
|---|---|
| [auth.md](./auth.md) | the whole file; section 4 for the operator key files. Sections 5, 6, 7, 8.1 and 8.3 are pointers to spec 86 and spec 87. |
| [86/requirements.md](../../../86-auth-primitives/requirements.md) | section 3 (the trait and the result types) |
| [87/requirements.md](../../../87-mutual-tls/requirements.md) | section 4 (the CA, the trust store, the verifiers, the mTLS providers) |
| [agent-rpc-architecture.md](../../../83-agent-rpc-serving-system/agent-rpc-architecture.md) | architecture (section 3), owner decisions (section 11) |
| [agent-rpc-architecture.md](../../../83-agent-rpc-serving-system/agent-rpc-architecture.md) | section 13: the workflow and the rules to check before each push |

## 3. Tasks

| Task | What | Main files | Done when |
|---|---|---|---|
| T5 | `foundation_auth::secret`: `SignedSecret` v1 and v2, `SigningKey`, `VerifyingKey`, `SecretIssuer`, `SecretVerifier`; the clippy lint file | `foundation_auth/src/secret/*` | v1 secrets from the branch tests verify unchanged; verify-order tests pass |
| T7 | `foundation_auth::keys`: `KeySource`, `PrivateKey` (OpenSSH, PKCS#8 and SEC1 PEM), `KeyLoadError`, `VerifyingKey::list_from_json`; test keys from the seeded entropy (spec 84) | `foundation_auth/src/keys/*` | the key loading tests of [auth.md](./auth.md) section 10 pass |
| T11 | `foundation_auth::client_auth`: `SignedSecretAuth`, `JwtAuth`, `InsecureNoAuth`, `ClientAuthInterceptor`, presets; the provider set names the spec 87 mTLS providers | `foundation_auth/src/client_auth/*` | provider tests pass with the simulated clock |

Tasks T8, T9 and T10 of the original list are now spec 87's tasks T1, T2 and T3. Do not
implement them here.

## 4. Done when

Version 1 secrets from the branch tests verify unchanged. The key loading and provider tests of
[auth.md](./auth.md) section 10 pass. The provider set includes the spec 87 mTLS providers and
defines none of them.

## 5. Pull requests

One PR for T5 and T7, then one for T11 (was PR group D).
