---
feature: "Transport Prerequisites"
description: "Two small library fixes the hosts need: connectrpc keeps request extensions through dispatch and takes its deadline clock from Env; foundation_http reports the TLS peer certificates after the handshake."
status: "pending"
priority: "high"
depends_on: ["08-env-and-side-effects"]
part_of: "07-pluggable-agent-rpc"
estimated_effort: "small"
created: 2026-10-27
last_updated: 2026-10-27
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 2
  total: 2
  completion_percentage: 0
---

# Transport Prerequisites

_Split out of [feature 07](../07-pluggable-agent-rpc/feature.md) on 2026-10-27 (owner direction:
feature 07 was too large). Written in ASD-STE100 Simplified Technical English (STE). Code and
identifiers stay as they are._

The design of this feature is in the topic files of feature 07. They stay in one place, because
they link to each other. This file says what to build, what to read, and when it is done. Task
IDs keep their feature 07 numbers, so references such as "task T3" stay valid. The
[glossary in feature 07](../07-pluggable-agent-rpc/feature.md#2-glossary) explains each term.

## 1. What this feature does

Two small fixes in shared crates. `foundation_connectrpc` keeps request extensions through dispatch (the `build_ctx` fix), so client auth can read connection facts, and computes deadlines from the `Env` clock. `foundation_http` puts the peer certificates in `ConnectionContext::tls` after the TLS handshake, so mTLS auth can read them.

Part of the agent-rpc proof of concept (features 07 to 13).

## 2. What to read

| Design file | Sections |
|---|---|
| [auth.md](../07-pluggable-agent-rpc/auth.md) | section 7.4 (the host must report the peer certificate) and section 8.2 (the interceptor needs the `build_ctx` fix) |
| [hosts.md](../07-pluggable-agent-rpc/hosts.md) | section 4.3 (listeners and TLS) and the `foundation_connectrpc` row of section 5.4 |
| [feature 07 `feature.md`](../07-pluggable-agent-rpc/feature.md) | architecture (section 3), owner decisions (section 11) |
| [feature 07 `start.md`](../07-pluggable-agent-rpc/start.md) | the workflow and the rules to check before each push |

## 3. Tasks

| Task | What | Main files | Done when |
|---|---|---|---|
| T3 | connectrpc: keep request extensions through dispatch (`build_ctx` fix); `CallHooks::with_clock` | `foundation_connectrpc/src/shared/router/dispatch.rs` | dispatch tests pass; deadlines follow the `Env` clock |
| T4 | `ConnectionContext::tls` holds the peer certificates after the handshake | `foundation_http` | `tls_peer_info_tests` pass |

## 4. Done when

The dispatch tests pass and deadlines follow the `Env` clock. `tls_peer_info_tests` pass.

## 5. Pull requests

One PR (part of PR group C).
