---
feature: "Transport Prerequisites"
description: "Two small library fixes the hosts need: connectrpc keeps request extensions through dispatch and takes its deadline clock from Ctx; foundation_http reports the TLS peer certificates after the handshake."
status: "pending"
priority: "high"
depends_on: ["specifications/84-context-injection"]
part_of: "specifications/83-agent-rpc-serving-system/agent-rpc-architecture.md"
estimated_effort: "small"
created: 2026-10-27
last_updated: 2026-11-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 2
  total: 2
  completion_percentage: 0
---

# Transport Prerequisites

_Split out of feature 07 on 2026-10-27 (owner direction: each feature is focused on one
thing). Written in ASD-STE100 Simplified Technical English (STE). Code and
identifiers stay as they are._

The design files of this feature are in this directory. The shared architecture, glossary and
owner decisions are in [`agent-rpc-architecture.md`](../../../83-agent-rpc-serving-system/agent-rpc-architecture.md). Task
IDs keep their old numbers, so references such as "task T3" stay valid. The
[glossary](../../../83-agent-rpc-serving-system/agent-rpc-architecture.md#2-glossary) explains each term.

## 1. What this feature does

Two small fixes in shared crates. `foundation_connectrpc` keeps request extensions through dispatch (the `build_ctx` fix), so client auth can read connection facts, and computes deadlines from the `Ctx` clock. `foundation_http` puts the peer certificates in `ConnectionContext::tls` after the TLS handshake, so mTLS auth can read them.

Part of the agent-rpc proof of concept.

## 2. What to read

| Design file | Sections |
|---|---|
| [mtls.md](../../../87-mutual-tls/mtls.md) | section 3.4 (the host must report the peer certificate) |
| [auth.md](../../../81-foundation-auth/features/00-foundation-auth/auth.md) | section 8.2 (the interceptor needs the `build_ctx` fix) |
| [84/requirements.md](../../../84-context-injection/requirements.md) | section 3 (the `Env` bundle; the clock is `ctx.clock()`) |
| [hosts.md](../../../83-agent-rpc-serving-system/features/04-native-host-and-dst/hosts.md) | section 4.3 (listeners and TLS) and the `foundation_connectrpc` row of section 5.4 |
| [agent-rpc-architecture.md](../../../83-agent-rpc-serving-system/agent-rpc-architecture.md) | architecture (section 3), owner decisions (section 11) |
| [agent-rpc-architecture.md](../../../83-agent-rpc-serving-system/agent-rpc-architecture.md) | section 13: the workflow and the rules to check before each push |

## 3. Tasks

| Task | What | Main files | Done when |
|---|---|---|---|
| T3 | connectrpc: keep request extensions through dispatch (`build_ctx` fix); `CallHooks::with_clock` | `foundation_connectrpc/src/shared/router/dispatch.rs` | dispatch tests pass; deadlines follow the `Ctx` clock |
| T4 | `ConnectionContext::tls` holds the peer certificates after the handshake | `foundation_http` | `tls_peer_info_tests` pass |

## 4. Done when

The dispatch tests pass and deadlines follow the `Ctx` clock. `tls_peer_info_tests` pass.

## 5. Pull requests

One PR (part of PR group C).
