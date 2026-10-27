---
feature: "Env and Injected Side Effects"
description: "The Env bundle (clock, entropy, IDs, timer) in valtron's root Ctx, the #[valtron(env = …)] / #[valtron_test(env = …)] macro argument, the side-effect lint, and foundation_ai reading IDs and time through Env."
status: "pending"
priority: "high"
depends_on: ["07-pluggable-agent-rpc"]
part_of: "07-pluggable-agent-rpc"
estimated_effort: "medium"
created: 2026-10-27
last_updated: 2026-10-27
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 3
  total: 3
  completion_percentage: 0
---

# Env and Injected Side Effects

_Split out of [feature 07](../07-pluggable-agent-rpc/feature.md) on 2026-10-27 (owner direction:
feature 07 was too large). Written in ASD-STE100 Simplified Technical English (STE). Code and
identifiers stay as they are._

The design of this feature is in the topic files of feature 07. They stay in one place, because
they link to each other. This file says what to build, what to read, and when it is done. Task
IDs keep their feature 07 numbers, so references such as "task T1" stay valid. The
[glossary in feature 07](../07-pluggable-agent-rpc/feature.md#2-glossary) explains each term.

## 1. What this feature does

Every side effect (time, randomness, IDs, timers) comes from one `Env` bundle that valtron carries in its context. Core code never reads a global clock or random number generator. Tests put in a seeded `Env`, so a run repeats exactly from a seed.

Part of the agent-rpc proof of concept (features 07 to 13).

## 2. What to read

| Design file | Sections |
|---|---|
| [env.md](../07-pluggable-agent-rpc/env.md) | the whole file: the traits, the `Env` bundle, valtron `Ctx`, the macro argument (section 7.1), the lint |
| [deterministic-simulation-testing.md](../07-pluggable-agent-rpc/deterministic-simulation-testing.md) | why every side effect is a trait |
| [feature 07 `feature.md`](../07-pluggable-agent-rpc/feature.md) | architecture (section 3), owner decisions (section 11) |
| [feature 07 `start.md`](../07-pluggable-agent-rpc/start.md) | the workflow and the rules to check before each push |

## 3. Tasks

| Task | What | Main files | Done when |
|---|---|---|---|
| T1 | `Clock`, `EntropySource`, `IdSource` traits; `SystemClock`, `SimClock`, `OsEntropy`, `SeededEntropy`, `Scru128Source` | `foundation_compact/src/{clock,entropy,ids}` | contract tests pass natively and on wasm |
| T2 | `Env` bundle; `Env` in valtron's root `Ctx`; `current_ctx`, `with_ctx`; `Timer` on the `Env` clock; `single::initialize_pool_with`; `#[valtron(env = …)]` / `#[valtron_test(env = …)]` macro argument ([env.md](../07-pluggable-agent-rpc/env.md) 7.1) | `foundation_core/src/{env.rs,valtron/*}` | `env_tests`, `env_install_tests`, `sim_time_tests` pass |
| T12 | `foundation_ai` reads IDs and time through `Env` (`SessionId::from_id`) | `foundation_ai/src/agentic/*` | agent loop runs under `SimRuntime` with stable IDs |

## 4. Done when

The contract tests of `Clock`, `EntropySource` and `IdSource` pass natively and on wasm. `env_tests`, `env_install_tests` and `sim_time_tests` pass. The agent loop of `foundation_ai` runs with stable IDs under a seeded `Env`.

## 5. Pull requests

One PR (was PR group B, plus T12).
