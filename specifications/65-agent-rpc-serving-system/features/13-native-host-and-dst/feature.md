---
feature: "Native Host and Simulation Tests"
description: "The native agent-rpc host (config, listeners, mTLS, operator key paths, LocalCatalog with Candle devices) and the deterministic simulation test harness. Finishing this feature gives the working agent-rpc proof of concept."
status: "pending"
priority: "high"
depends_on: ["09-transport-prerequisites", "11-foundation-auth", "12-agent-rpc-core"]
part_of: "07-pluggable-agent-rpc"
estimated_effort: "large"
created: 2026-10-27
last_updated: 2026-10-27
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 2
  total: 2
  completion_percentage: 0
---

# Native Host and Simulation Tests

_Split out of [feature 07](../07-pluggable-agent-rpc/feature.md) on 2026-10-27 (owner direction:
feature 07 was too large). Written in ASD-STE100 Simplified Technical English (STE). Code and
identifiers stay as they are._

The design of this feature is in the topic files of feature 07. They stay in one place, because
they link to each other. This file says what to build, what to read, and when it is done. Task
IDs keep their feature 07 numbers, so references such as "task T17" stay valid. The
[glossary in feature 07](../07-pluggable-agent-rpc/feature.md#2-glossary) explains each term.

## 1. What this feature does

The native program: `#[valtron] async fn main`, configuration, TLS and mTLS listeners, operator key paths, and local models. Also the simulation harness (`SimRuntime`, `MemNetwork`, `Faulty*` stores, `TestWorld`) that runs the whole client-to-instance flow from one seed. **When this feature is done, the agent-rpc proof of concept works end to end on a native machine.**

Part of the agent-rpc proof of concept (features 07 to 13).

## 2. What to read

| Design file | Sections |
|---|---|
| [hosts.md](../07-pluggable-agent-rpc/hosts.md) | sections 1 to 4, 7, 8 and 9 (native host, native client) |
| [deterministic-simulation-testing.md](../07-pluggable-agent-rpc/deterministic-simulation-testing.md) | the harness and its parts |
| [feature 07 `feature.md`](../07-pluggable-agent-rpc/feature.md) | architecture (section 3), owner decisions (section 11) |
| [feature 07 `start.md`](../07-pluggable-agent-rpc/start.md) | the workflow and the rules to check before each push |

## 3. Tasks

| Task | What | Main files | Done when |
|---|---|---|---|
| T17 | Native host: config, listeners, mTLS, last-seen after handshake, `LocalCatalog` with Candle device, operator key paths (`--signing-key`, `--ca-key`, `--server-key`) | `host/native/*` | native end-to-end and revocation tests pass |
| T18 | DST: `SimRuntime`, `MemNetwork`, `MemTransport`, `Faulty*` wrappers, `ScriptedCatalog`, `TestWorld`; first DST tests | `testing` features in `foundation_core`, `foundation_connectrpc`, `foundation_db`, `foundation_agent_rpc` | DST tests pass for 32 fixed seeds, twice in a row with identical traces |

## 4. Done when

The native end-to-end and revocation tests pass. The DST tests pass for 32 fixed seeds, twice in a row with identical traces.

## 5. Pull requests

One PR (was PR group F).
