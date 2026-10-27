---
feature: "Transport-Agnostic Agent RPC Core"
description: "The new agent.proto, model selection rules, the ModelCatalog trait, the InstanceRegistry trait with LocalInstanceRegistry, and the AgentService handlers and enrollment service over those traits."
status: "pending"
priority: "high"
depends_on: ["08-env-and-side-effects", "11-foundation-auth"]
part_of: "07-pluggable-agent-rpc"
estimated_effort: "large"
created: 2026-10-27
last_updated: 2026-10-27
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0
---

# Transport-Agnostic Agent RPC Core

_Split out of [feature 07](../07-pluggable-agent-rpc/feature.md) on 2026-10-27 (owner direction:
feature 07 was too large). Written in ASD-STE100 Simplified Technical English (STE). Code and
identifiers stay as they are._

The design of this feature is in the topic files of feature 07. They stay in one place, because
they link to each other. This file says what to build, what to read, and when it is done. Task
IDs keep their feature 07 numbers, so references such as "task T13" stay valid. The
[glossary in feature 07](../07-pluggable-agent-rpc/feature.md#2-glossary) explains each term.

## 1. What this feature does

The service logic of agent-rpc, with no host code. The handlers see only traits: `ModelCatalog`, `InstanceRegistry`, `ClientAuth` and the `Env`. A client can ask which model kinds and devices a server supports, and choose one.

Part of the agent-rpc proof of concept (features 07 to 13).

## 2. What to read

| Design file | Sections |
|---|---|
| [proto.md](../07-pluggable-agent-rpc/proto.md) | the wire contract and the bootstrap payload |
| [registry-and-models.md](../07-pluggable-agent-rpc/registry-and-models.md) | model catalog, kinds and devices, the instance registry |
| [feature 07 `feature.md`](../07-pluggable-agent-rpc/feature.md) | architecture (section 3), owner decisions (section 11) |
| [feature 07 `start.md`](../07-pluggable-agent-rpc/start.md) | the workflow and the rules to check before each push |

## 3. Tasks

| Task | What | Main files | Done when |
|---|---|---|---|
| T13 | New `agent.proto`; `InstanceSpec::from_wire`; model selection rules | `proto/`, `core/model/mod.rs` | one test per selection rule passes |
| T14 | `ModelCatalog` trait, `ModelHandle`, `RemoteCatalog` | `core/model/*` | load-once and remote catalog tests pass |
| T15 | `InstanceRegistry` trait, `Scope`, `RegistryConfig`, `TurnLimits`, `LocalInstanceRegistry` | `core/registry/*` | registry contract tests pass on the local registry |
| T16 | `core/service.rs`: `AgentService` handlers over the traits; `GetCapabilities`; `core/enroll.rs` | `core/{service,enroll}.rs` | handler tests pass with fakes |

## 4. Done when

One test per model selection rule passes. The registry contract tests pass on `LocalInstanceRegistry`. The handler tests pass with fakes.

## 5. Pull requests

One PR (was PR group E without T12).
