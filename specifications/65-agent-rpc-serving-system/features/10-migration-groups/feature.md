---
feature: "Migration Groups"
description: "foundation_db migrations as ordered groups per database (shared, then specific), declared in code or by directory, each numbered from 1 and recorded by group, plus a FleetRunner across databases."
status: "pending"
priority: "high"
depends_on: ["07-pluggable-agent-rpc"]
part_of: "agent-rpc-architecture.md"
estimated_effort: "medium"
created: 2026-10-27
last_updated: 2026-10-27
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 1
  total: 1
  completion_percentage: 0
---

# Migration Groups

_Split out of feature 07 on 2026-10-27 (owner direction: each feature is focused on one
thing). Written in ASD-STE100 Simplified Technical English (STE). Code and
identifiers stay as they are._

The design files of this feature are in this directory. The shared architecture, glossary and
owner decisions are in [`agent-rpc-architecture.md`](../../agent-rpc-architecture.md). Task
IDs keep their old numbers, so references such as "task T25" stay valid. The
[glossary](../../agent-rpc-architecture.md#2-glossary) explains each term.

## 1. What this feature does

Migrations run per database as ordered groups: template groups such as `shared` first, then the database's own group. A group comes from code, from embedded files or from a directory. Each group is numbered from 1 and recorded by group. `FleetRunner` runs the groups of many databases, one after another or in parallel.

Part of the agent-rpc proof of concept (features 07 to 13).

## 2. What to read

| Design file | Sections |
|---|---|
| [migrations.md](./migrations.md) | the whole file |
| [agent-rpc-architecture.md](../../agent-rpc-architecture.md) | architecture (section 3), owner decisions (section 11) |
| [agent-rpc-architecture.md](../../agent-rpc-architecture.md) | section 13: the workflow and the rules to check before each push |

## 3. Tasks

| Task | What | Main files | Done when |
|---|---|---|---|
| T25 | Migration runner by groups: `MigrationSet` from code, embedded files and directories; `MigrationRunner::from_static` / `add_from_static`; per-group numbering from 1 with no gaps; checksums; one transaction per migration; record rows keyed by group; upgrade of today's table with the 023 repair; `embed_migrations!` | `foundation_db/src/core/schema/*`, `foundation_macros` | [migrations.md](./migrations.md) section 6 tests pass |

## 4. Done when

The tests of [migrations.md](./migrations.md) section 6 pass, including the layout and fleet tests.

## 5. Pull requests

One PR (part of PR group C). It has no dependency on features 08 and 09 and can start at once.
