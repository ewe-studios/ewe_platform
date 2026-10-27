---
feature: "Worker Host and Web Client"
description: "agent-rpc on Cloudflare Workers (FetchServe, FetchTransport, DoSqlStore, one Durable Object per instance, the cf-agent-rpc example), the browser client, and the history archive to R2 on DestroyInstance."
status: "pending"
priority: "medium"
depends_on: ["09-transport-prerequisites", "12-agent-rpc-core"]
part_of: "07-pluggable-agent-rpc"
estimated_effort: "large"
created: 2026-10-27
last_updated: 2026-10-27
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0
---

# Worker Host and Web Client

_Split out of [feature 07](../07-pluggable-agent-rpc/feature.md) on 2026-10-27 (owner direction:
feature 07 was too large). Written in ASD-STE100 Simplified Technical English (STE). Code and
identifiers stay as they are._

The design of this feature is in the topic files of feature 07. They stay in one place, because
they link to each other. This file says what to build, what to read, and when it is done. Task
IDs keep their feature 07 numbers, so references such as "task T19" stay valid. The
[glossary in feature 07](../07-pluggable-agent-rpc/feature.md#2-glossary) explains each term.

## 1. What this feature does

The same RPC surface on Cloudflare Workers and in the browser. Feature 05 (Cloudflare deployment) builds its end-to-end tests on this feature.

Not part of the native proof of concept. It can start once feature 12 is done.

## 2. What to read

| Design file | Sections |
|---|---|
| [hosts.md](../07-pluggable-agent-rpc/hosts.md) | sections 5 (Worker host) and 6 (web client) |
| [registry-and-models.md](../07-pluggable-agent-rpc/registry-and-models.md) | the Durable Object registry |
| [feature 07 `feature.md`](../07-pluggable-agent-rpc/feature.md) | architecture (section 3), owner decisions (section 11) |
| [feature 07 `start.md`](../07-pluggable-agent-rpc/start.md) | the workflow and the rules to check before each push |

## 3. Tasks

| Task | What | Main files | Done when |
|---|---|---|---|
| T19 | `FetchServe` and `FetchTransport` (wasm) | `foundation_connectrpc/src/wasm/*` | wasm fetch tests pass under Node |
| T20 | `DoSqlStore`: `foundation_db` storage over a DO's SQL | `foundation_db` | contract tests pass under Miniflare |
| T21 | Worker host: entry, instance DO, `DurableObjectRegistry`, `DoStubTransport`; `examples/cf-agent-rpc` | `host/worker/*` | the `cf-agent-rpc` Miniflare case passes |
| T22 | Web client: `WebClient` over `FetchTransport`; `examples/agent-rpc-web` | `client/web.rs` | the web client tests pass in a headless browser |
| T26 | History archive to R2 on `DestroyInstance` (Worker and native) | `host/worker/object.rs`, `core/archive.rs` | the archive tests pass; a failed upload deletes nothing |

## 4. Done when

The `cf-agent-rpc` Miniflare case passes. The web client tests pass in a headless browser. The archive tests pass, and a failed upload deletes nothing.

## 5. Pull requests

Two PRs: T19 + T20 + T22, then T21 + T26 (was PR group G).
