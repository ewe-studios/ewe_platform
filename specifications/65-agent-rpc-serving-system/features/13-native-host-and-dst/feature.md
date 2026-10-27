---
feature: "Native Host and Simulation Tests"
description: "The native agent-rpc host (config, listeners, mTLS, operator key paths, LocalCatalog with Candle devices) and the deterministic simulation test harness. Finishing this feature gives the working agent-rpc proof of concept."
status: "pending"
priority: "high"
depends_on: ["09-transport-prerequisites", "11-foundation-auth", "12-agent-rpc-core"]
part_of: "agent-rpc-architecture.md"
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

_Split out of feature 07 on 2026-10-27 (owner direction: each feature is focused on one
thing). Written in ASD-STE100 Simplified Technical English (STE). Code and
identifiers stay as they are._

The design files of this feature are in this directory. The shared architecture, glossary and
owner decisions are in [`agent-rpc-architecture.md`](../../agent-rpc-architecture.md). Task
IDs keep their old numbers, so references such as "task T17" stay valid. The
[glossary](../../agent-rpc-architecture.md#2-glossary) explains each term.

## 1. What this feature does

The native program: `#[valtron] async fn main`, configuration, TLS and mTLS listeners, operator key paths, and local models. Also the simulation harness (`SimRuntime`, `MemNetwork`, `Faulty*` stores, `TestWorld`) that runs the whole client-to-instance flow from one seed. **When this feature is done, the agent-rpc proof of concept works end to end on a native machine.**

Part of the agent-rpc proof of concept (features 07 to 13).

## 2. What to read

| Design file | Sections |
|---|---|
| [hosts.md](./hosts.md) | sections 1 to 4, 7, 8 and 9 (native host, native client) |
| [deterministic-simulation-testing.md](../08-env-and-side-effects/deterministic-simulation-testing.md) | the harness and its parts |
| [agent-rpc-architecture.md](../../agent-rpc-architecture.md) | architecture (section 3), owner decisions (section 11) |
| [agent-rpc-architecture.md](../../agent-rpc-architecture.md) | section 13: the workflow and the rules to check before each push |

## 3. Tasks

| Task | What | Main files | Done when |
|---|---|---|---|
| T17 | Native host: config, listeners, mTLS, last-seen after handshake, `LocalCatalog` with Candle device, operator key paths (`--signing-key`, `--ca-key`, `--server-key`) | `host/native/*` | native end-to-end and revocation tests pass |
| T18 | DST: `SimRuntime`, `MemNetwork`, `MemTransport`, `Faulty*` wrappers, `ScriptedCatalog`, `TestWorld`; first DST tests | `testing` features in `foundation_core`, `foundation_connectrpc`, `foundation_db`, `foundation_agent_rpc` | DST tests pass for 32 fixed seeds, twice in a row with identical traces |

## 4. Done when

The native end-to-end and revocation tests pass. The DST tests pass for 32 fixed seeds, twice in a row with identical traces.

## 5. Pull requests

One PR (was PR group F).
