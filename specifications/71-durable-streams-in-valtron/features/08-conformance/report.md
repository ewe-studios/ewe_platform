# Feature 08: Conformance report

_Date: 2026-11-27. Branch: `spec-durable-streams`. Pull request: #1025 (draft)._

**Abbreviations.** Continuous Integration (CI); Durable Object (DO); Hypertext Transfer Protocol
(HTTP); JavaScript Object Notation (JSON); Server-Sent Events (SSE); virtual file system (VFS);
WebAssembly (wasm).

**Terms.**

- **Gate.** The `durable-conformance` CI entry that runs both reference suites and the two
  cross-runs.
- **Direction.** One pairing of our side and the reference side in the four-direction matrix.
- **Runtime row.** One target in the runtime matrix: embedded, wasm32, Worker, DO, WebSocket or
  `foundation_ai` Session.
- **Proof.** A command or a recorded real-service run that shows the same core works in a row.

## 1. Result

| Item | Result | Evidence |
|---|---|---|
| Server suite | 338 passed (338), 0 skipped, 0 failed | Recorded. |
| Client suite | 267 passed, 0 failed, 2 skipped of 269 | Recorded. |
| Direction 3 cross-run | 6 scenarios pass | Recorded test target. |
| Direction 4 cross-run | 5 scenarios pass | Recorded test target. |
| Runtime matrix | 3 rows have a runtime proof, 1 row has a build proof, 2 rows are gaps | Section 5. |
| Clock parameter guard | 0 violations | Re-run in this change. |
| Time source guard | 0 violations | Re-run in this change. |

The two suite counts are recorded evidence. This change did not re-run the strict gate. The host
is shared and loaded; the strict gate result is recorded separately. Section 3 lists the sources.

## 2. Gate command

```text
cargo run --locked --profile uat -p foundation_ci -- run durable-conformance --step test
```

The `durable-conformance` job in `.github/workflows/check.yaml` runs this command. The job uses
`ubuntu-latest`, Rust and Node 20, and has a 20-minute timeout. It sets a concurrency group. A
new push cancels a superseded pull request run. Full runs queue.

The gate is a first-class entry in `ci/packages.toml`:

```toml
[gates.durable-conformance]
recipe = "durable-conformance"
packages = [
  "foundation_core", "foundation_http", "foundation_netio",
  "foundation_event_stream", "foundation_nativeapis",
  "foundation_connectrpc", "foundation_db",
]
paths = ["integration/"]
```

A pull request and a push pass `--if-affected`. `foundation-ci affected` selects the gate from
the changed files. The nightly schedule and a manual dispatch run the gate in full.

The recipe runs `ci/durable_conformance.sh` for directions 1 and 2. It then runs the two
cross-run test targets for directions 3 and 4.

```mermaid
flowchart LR
  S[Source change] --> A{foundation-ci affected}
  A -->|selected| G[durable-conformance job]
  A -->|not selected| N[No gate run]
  G --> D1[1: our server, reference server suite]
  G --> D2[2: our client, reference client suite]
  G --> D3[3: our server, reference clients]
  G --> D4[4: our server, our client]
  D1 --> P{All pass?}
  D2 --> P
  D3 --> P
  D4 --> P
  P -->|yes| OK[Job passes]
  P -->|no| F[Job fails]
```

_Caption: The gate selects from the change and runs all four directions in one job._

## 3. Recorded numbers

The numbers come from these recorded entries:

- `specifications/71-durable-streams-in-valtron/progress.md`, entry "2026-11-27: feature 07
  Server-Sent Events (SSE) landed": server 338 pass, 0 skip; client 267 pass, 2 suite-forced
  skips, 0 fail.
- `specifications/71-durable-streams-in-valtron/progress.md`, entry "2026-11-27: feature 08 CI
  wiring landed": the gate bar is 338 passed (338) and 267 passed with 2 suite-forced skips.
- `specifications/71-durable-streams-in-valtron/handoff.md`, section 4.4: the same gate passed
  338 of 338 and 267 of 269 runnable on a quiet host.
- `ci/durable_conformance.sh`: the script fails on any other count.

| Suite | Package | Runnable | Passed | Failed | Skipped |
|---|---|---|---|---|---|
| Server | `@durable-streams/server-conformance-tests` 0.3.7 | 338 | 338 | 0 | 0 |
| Client | `@durable-streams/client-conformance-tests` 0.2.13 | 269 | 267 | 0 | 2 |

The server suite runs with `subscriptions: true`. The reference repository pin for the Rust
client is
`461b40267aabd644558f9b19dbb9507dd5f691cf` of
`https://github.com/durable-streams/durable-streams`.

## 4. Four-direction matrix

| Direction | Our side | Reference side | Command | CI path |
|---|---|---|---|---|
| 1 | Our server | TypeScript server suite | `ci/durable_conformance.sh` (server section) | `foundation-ci run durable-conformance --step test` |
| 2 | Our client | TypeScript client suite; it starts the TypeScript reference server | `ci/durable_conformance.sh` (client section) | The same gate step |
| 3 | Our server | TypeScript `fetch` client (inside direction 1) and the reference Rust client | `cargo test -p foundation_http --features multi --test durable_reference_cross_run` | The same gate step |
| 4 | Our server | Our client | `cargo test -p foundation_http --features multi --test durable_cross_client` | The same gate step |

Direction 3 covers the TypeScript `fetch` client through direction 1. Its Rust part has 6
scenarios. Direction 4 has 5 scenarios. A failing test fails the job.

## 5. Runtime matrix

The core takes `Ctx` and a store. It has no global runtime, socket, HTTP type or executor of its
own. The rows below show how the same core runs in each target.

| Row | How the core runs | Proof | Status |
|---|---|---|---|
| Local embedded | Direct async calls | `durable_core_no_valtron` (1 test) and `durable::runtime_matrix` (2 tests) | Runtime proof. |
| wasm32 | The core compiles for the wasm32 target | Six `cargo check` commands pass | Build proof only. Issue #1080. |
| Cloudflare Worker | The Worker fetch handler calls the core | `foundation_event_stream` wasm build, and `/run-d1` in the DO fixture | Runtime proof. |
| Durable Object | DO SQLite storage and one fetch handler | `/run` in the DO fixture | Runtime proof. |
| WebSocket server | Adapter events | No durable adapter or test | Gap. Issue #1077. |
| `foundation_ai` Session | The core as the session log | No durable log driver or test | Gap. Issue #1079. |

### 5.1 Local embedded

The embedded proof makes no HTTP call and uses no socket.

```text
cargo test -p durable_core_no_valtron
```

Result: 1 passed. The crate depends on `foundation_core` with `default-features = false` and no
`valtron` feature. It runs create, append and read over `MemoryVfs` with `futures-lite`.

```text
cargo test -p foundation_core --test mod -- durable::runtime_matrix
```

Result: 2 passed. `embedded_core_roundtrip` runs create, two appends, a byte-exact read, close
and a read that reports the closure. `service_runs_without_http` calls `DurableService` with no
HTTP adapter.

### 5.2 wasm32

The core compiles for `wasm32-unknown-unknown`. These commands pass on this branch:

```text
cargo check -p foundation_core --target wasm32-unknown-unknown --no-default-features --features std
cargo check -p foundation_core --target wasm32-unknown-unknown --no-default-features --features std,multi
cargo check -p foundation_core --target wasm32-unknown-unknown --no-default-features --features std,js-foundation-wasm
cargo check -p foundation_event_stream --lib --target wasm32-unknown-unknown
cargo check -p foundation_db --target wasm32-unknown-unknown --no-default-features --features do-sql
cargo check -p foundation_db --target wasm32-unknown-unknown --no-default-features --features wasm-bindgen-storage
```

Result: all six commands pass. The `foundation_core` checks print the known warning for the
unused `scoped::*` re-export, issue #1063. The CI wasm gate also runs these sets.

The row has no runtime test. `wasm_core_roundtrip` does not exist. Issue #1080 records the gap.
The `backends/foundation_core/integration/wasm_tests` crate runs valtron executor tests on the
Deno host. It does not run the durable core.

### 5.3 Cloudflare Worker

Build proof:

```text
cargo check -p foundation_event_stream --lib --target wasm32-unknown-unknown
cargo check -p foundation_http --target wasm32-unknown-unknown --no-default-features --features wasm
```

Both pass. `foundation_event_stream::cf` exposes the shared subscription core to a Worker host.
`foundation_http/src/wasm/bridge/cf.rs` is the Worker HTTP bridge.

Runtime proof: `backends/foundation_db/integration/do_sql/` is a standalone Worker outside the
workspace. Its top-level `#[event(fetch)]` handler serves `GET /run-d1`. That route calls
`run_d1`, which runs the shared VFS conformance kit over `D1Executor` and the real local D1
binding. The recorded result is `{"ok":true,"backend":"d1","scenarios":[...]}` with all nine
scenarios: `create`, `append`, `read_byte_exact`, `bounded_pages`, `fork_prefix`, `fork_append`,
`soft_delete_cascade`, `ttl_slide` and `version_conflict`.

The run uses `wrangler dev --local`. The result is recorded in `progress.md`, entry "feature 05
T6 done". This change did not re-run it. The fixture is outside the workspace and outside the CI
gate.

### 5.4 Durable Object

Build proof:

```text
cargo check -p foundation_db --target wasm32-unknown-unknown --no-default-features --features do-sql
```

Runtime proof: `DoSqlObject` in the same fixture runs the kit over `DoSqlExecutor` and the real
DO SQLite storage. `GET /run` reaches it through the Worker. The migration declares
`new_sqlite_classes = ["DoSqlObject"]`. The recorded result is `{"ok":true,...}` with the same
nine scenarios. `GET /rollback` returns `{"failed":true,"ok":true,"remaining":0}`: a failing
second statement rolls the first insert back.

The same recorded run and the same caveat as section 5.3 apply.

### 5.5 WebSocket server

This row is a gap. `foundation_netio::websocket` has a server and a client.
`foundation_http::native::upgrade::accept_websocket` completes the upgrade. No code maps a
WebSocket message to `DurableService` or to the client transport. There is no
`websocket_core_roundtrip` test and no WebSocket entry in the driver matrix. Issue #1077 records
the exact missing piece and a likely fix.

### 5.6 `foundation_ai` Session

This row is a gap. `AgentSession` uses `foundation_db::traits::DocumentStore` and
`agentic::memory_store::MemoryStore`. No implementation of either trait uses a durable stream as
the session log. There is no `session_uses_core` test. Issue #1079 records the missing driver and
a likely fix.

## 6. Skip register

| Suite | Test | File | Reason | Forced by | Author and commit |
|---|---|---|---|---|---|
| Server | None | - | - | - | - |
| Client | `sse-base64-data-integrity-multiline` | `read-sse-base64.yaml` | `Swift implementation broken` | `@durable-streams/client-conformance-tests` 0.2.13 | Valter Balegas, `82a566ace620b1b8d0d43cdf181356e6a6f6f4aa` |
| Client | `sse-base64-large-payload-4kb` | `read-sse-base64.yaml` | `Swift implementation broken` | `@durable-streams/client-conformance-tests` 0.2.13 | Valter Balegas, `82a566ace620b1b8d0d43cdf181356e6a6f6f4aa` |

The server suite has no skip. The two client skips are in the suite file, not in our adapter. The
suite marks them for every client. No skip comes from our code. Our client-core tests cover
multiline and large base64 payloads. Issue #1082 tracks the two upstream skips. The owner
acceptance bar in `feature.md` section 5.1 approves these two skips.

The skip lines are present at the pinned reference commit
`461b40267aabd644558f9b19dbb9507dd5f691cf`, in
`packages/client-conformance-tests/test-cases/consumer/read-sse-base64.yaml` at lines 55 and
367.

## 7. Open issues

| Issue | Severity | Subject | Matrix row |
|---|---|---|---|
| #1077 | medium | No WebSocket driver for the core or the client | WebSocket |
| #1079 | medium | `foundation_ai` Session has no Durable Streams log driver | Session |
| #1080 | low | No wasm32 runtime roundtrip test | wasm32 |
| #1081 | medium | The gate builds the server with the valtron feature on | Gate |
| #1082 | low | Two suite-forced client skips (`Swift implementation broken`) | Skip register |
| #1063 | low | The wasm check warns on the unused `scoped::*` re-export | wasm32 build |

## 8. Environment caveat

The strict gate needs a quiet host. On 2026-11-27 a shared machine ran an Android emulator and a
headless browser at high load. Under that load, the property tests were interrupted by the test
timeout, not by an assertion, and the server suite reported 337 of 338. The same gate passed 338
of 338 and 267 of 269 runnable on a quiet host.

At the time of this report the host load average was 169. This change did not run the strict
gate. Run `ci/durable_conformance.sh` on a quiet host before the pull request becomes ready. The
script keeps a 30-second vitest timeout. Do not weaken a test for a loaded host. Record the load
and run the gate again.

## 9. What remains

- **T6 — valtron suite.** Run the reader, writer, wait and wake tests as a separate step.
- **T8 — runtime matrix.** Add the WebSocket driver (#1077), the Session log driver (#1079) and
  the wasm32 runtime test (#1080).
- **T9 — `foundation_ai` Session.** The same work as the Session row.
- **T11 to T21.** The behaviour-driven development (BDD) suite, the driver matrix, the model,
  differential, metamorphic, property, fault, crash, timing and VFS oracles.
- **The valtron-disabled gate.** Decide the form of requirement 8 (#1081). The separate
  `durable_core_no_valtron` crate proves the core runs without valtron. The gate does not.

The runtime matrix success criterion stays unticked until the WebSocket, Session and wasm32
runtime rows pass.
