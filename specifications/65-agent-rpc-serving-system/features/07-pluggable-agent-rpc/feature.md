---
feature: "Bring PR #126 onto master"
description: "Task T0: merge master into 65-f01-agent-rpc and adapt the agent-rpc crate to master's async-only storage traits and async AgentSession, so CI is green and features 08 to 14 can build on it."
status: "pending"
priority: "high"
depends_on: ["01-agent-rpc"]
builds_on:
  - "PR #126 (feature 01, branch 65-f01-agent-rpc)"
  - "PR #130, #131, #132, #133 (async-first valtron, async-only foundation_db)"
estimated_effort: "small"
created: 2026-10-22
last_updated: 2026-10-27
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 1
  total: 1
  completion_percentage: 0
---

# Bring PR #126 onto master

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are._

**Abbreviations in this file.** pull request (PR); continuous integration (CI); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

This feature does one thing: it brings the agent-rpc code of PR #126 onto current `master`, so
that the later features start from green CI. The overall design, the glossary and the owner
decisions are in [`agent-rpc-architecture.md`](../../agent-rpc-architecture.md). The work after
this feature is features 08 to 14; feature 15 is last.

## 1. Task

| Task | What | Main files | Done when |
|---|---|---|---|
| T0 | Merge `master` into `65-f01-agent-rpc`; adapt to async storage and async `AgentSession` | the crate | `ci.py run foundation_agent_rpc` passes |

State on 2026-10-27: paused by the owner. Three local commits in the `pr126` worktree fix
`foundation_db`, `foundation_ai` and most of `foundation_agent_rpc`; they are not pushed yet.


## 2. Done when

`python3 scripts/ci/ci.py run <crate>` passes for `foundation_db`, `foundation_ai`,
`foundation_agent_rpc`, `answerme-agent` and `ewe_platform`, and the CI checks of PR #126 are
green.

## 3. How to do it

1. Merge `master` into `65-f01-agent-rpc` with a merge commit (no rebase, no force-push).
2. Where the branch duplicates something that `master` now has (for example an async
   `TursoStorage` constructor or the async turn API), take `master`'s version.
3. Port `foundation_agent_rpc` to the async storage traits and the async `AgentSession`. Tests
   use `#[valtron_test]`, never tokio.
4. Run the CI script for each crate above, then `cargo check --workspace --tests`.
