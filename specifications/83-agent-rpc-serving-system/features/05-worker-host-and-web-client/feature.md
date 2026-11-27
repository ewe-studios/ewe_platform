---
feature: "Worker Host and Web Client"
description: "agent-rpc on Cloudflare Workers (the host entry over foundation_http's CfHttpApp and the ConnectService adapter, DoSqlStore, one Durable Object per instance, the cf-agent-rpc example), the browser client over H1Transport::default(), and the history archive to R2 on DestroyInstance."
status: "pending"
priority: "medium"
depends_on: ["specifications/completed/85-wasm-fetch-transport", "specifications/completed/80-platform-prerequisites", "03-agent-rpc-core"]
part_of: "agent-rpc-architecture.md"
estimated_effort: "large"
created: 2026-10-27
last_updated: 2026-11-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0
---

# Worker Host and Web Client

_Split out of feature 07 on 2026-10-27 (owner direction: each feature is focused on one
thing). Written in ASD-STE100 Simplified Technical English (STE). Code and
identifiers stay as they are._

The design files of this feature are in this directory. The shared architecture, glossary and
owner decisions are in [`agent-rpc-architecture.md`](../../agent-rpc-architecture.md). Task
IDs keep their old numbers (T19 is now spec 85's task T2), so references such as "task T21" stay valid. The
[glossary](../../agent-rpc-architecture.md#2-glossary) explains each term.

## 1. What this feature does

The same RPC surface on Cloudflare Workers and in the browser. Feature 06 (Cloudflare host) builds its end-to-end tests on this feature.

Not part of the native proof of concept. It can start once feature 03 is done.

## 2. What to read

| Design file | Sections |
|---|---|
| [worker-and-web.md](./worker-and-web.md) | sections 5 (Worker host), 6 (web client) and 9 (tests); sections 1 to 3 of [hosts.md](../04-native-host-and-dst/hosts.md) also apply |
| [registry-and-models.md](../03-agent-rpc-core/registry-and-models.md) | the Durable Object registry |
| [agent-rpc-architecture.md](../../agent-rpc-architecture.md) | architecture (section 3), owner decisions (section 11) |
| [agent-rpc-architecture.md](../../agent-rpc-architecture.md) | section 13: the workflow and the rules to check before each push |

## 3. Tasks

| Task | What | Main files | Done when |
|---|---|---|---|
| T20 | `DoSqlStore`: `foundation_db` storage over a DO's SQL | `foundation_db` | contract tests pass under Miniflare |
| T21 | Worker host: entry, instance DO, `DurableObjectRegistry`; `examples/cf-agent-rpc` | `host/worker/*` | the `cf-agent-rpc` Miniflare case passes |
| T22 | Web client: `WebClient`; `examples/agent-rpc-web` | `client/web.rs` | the web client tests pass in a headless browser |
| T26 | History archive to R2 on `DestroyInstance` (Worker and native) | `host/worker/object.rs`, `core/archive.rs` | the archive tests pass; a failed upload deletes nothing |

Task T19 (`FetchServe`, `FetchTransport`) and the transport part of task T21 (`DoStubTransport`) moved to [spec 85](../../../completed/85-wasm-fetch-transport/requirements.md) on 2026-11-15. Spec 85 now delivers the wasm gaps inside the existing `HttpClient`, `DynNetClient` and `H1Transport` instead: the streaming response body (T1), the exchange over its own client (T2), `streaming_response_with` (T3), the streaming bridges and sink (T4) and `StubHttpClient` (T5). This feature uses those and writes no fetch pump. T21 here is the host wiring only.

## 4. Rules

### 4.1 Invariants

| # | Invariant |
|---|---|
| I1 | The Worker holds no state between requests except the valtron pool and the `Ctx`. An instance's state lives in its Durable Object. |
| I2 | Every limit is a field of `WorkerHostConfig` with a documented default (`hosts.md` section 8). |
| I3 | A body over `max_request_bytes` is refused with a Connect error before the core sees it. |
| I4 | A forged `ewe-forwarded-principal` header has no effect. The principal comes from the verified token or the client certificate only. |
| I5 | The archive upload happens before the instance is destroyed. A failed upload deletes nothing. |
| I6 | A dropped browser response cancels the handler. |
| I7 | No code writes a model provider key to a log, a record or the plan. |

## 5. Tests

All tests are modules of each crate's `tests/` directory (root rule 7).

| Test | Crate and file | What it proves |
|---|---|---|
| `ConnectService` | `foundation_agent_rpc/tests/wasm/worker_serve_tests.rs` (`wasm-bindgen-test`, Node) | Unary, server stream, an error status, a deadline, a body over the limit and a cancel. |
| Browser transport | `foundation_connectrpc/tests/wasm/h1_transport_tests.rs` | `H1Transport` over a test `NetClient`, against a `ConnectService` in the same test. |
| Worker host | `foundation_toolings/tests/cf_examples.rs`, the case `cf_agent_rpc` (Miniflare, feature `cf-worker-tests`) | Bearer auth, a revoked secret, CORS preflight, create, stream, list, destroy; a forged header has no effect; a DO restart keeps the history. |
| Cloudflare mTLS | the same case, with a fake `request.cf.tlsClientAuth` | `CfMtlsAuth` accepts a registered fingerprint and refuses `FAILED:...`. |
| Web client | `foundation_agent_rpc/tests/wasm/web_client_tests.rs` (headless browser) | The payload rules and a full turn against a Miniflare Worker. |
| Archive | `foundation_agent_rpc/tests/archive_tests.rs` | Upload, then destroy; a failed upload deletes nothing (I5). |

## 6. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_agent_rpc --target wasm32-unknown-unknown --no-default-features --features worker -- -D warnings
cargo test -p foundation_agent_rpc --features testing --profile uat archive_
cargo test -p foundation_toolings --features cf-worker-tests --profile uat cf_agent_rpc
```

The wasm tests run under `wasm-bindgen-test` in Node and in a headless browser, through the `mise` wasm tasks. The Miniflare case needs Node and the pinned `wrangler`.

## 7. Done when

The `cf-agent-rpc` Miniflare case passes. The web client tests pass in a headless browser. The archive tests pass, and a failed upload deletes nothing.

## 8. Pull requests

Two PRs: T20 + T22, then T21 + T26 (was PR group G). T19 is spec 85's task T2.
