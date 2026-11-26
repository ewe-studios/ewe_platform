---
feature: "Conformance"
status: "planned"
priority: "high"
depends_on: ["02-resumable-reader", "03-idempotent-producer", "04-fork-and-retention", "05-storage-backends", "06-http-rpc-surface", "07-durable-streams-client"]
crates: ["foundation_core", "foundation_http", "foundation_netio"]
---

# Feature 08: Conformance

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Continuous Integration (CI); Hypertext Transfer Protocol (HTTP);
JavaScript Object Notation (JSON); AeroSpace and Defence Industries Association of Europe (ASD),
the owner of the STE standard ASD-STE100.

## 1. Goal

Prove the core against the reference suites. The server suite runs against the HTTP adapter. The
client suite runs with the Rust adapter. Neither run needs valtron. A failing suite blocks the
merge.

## 2. Requirements

1. **Server suite.** Run the reference server suite against `HttpAdapter` on port 4437, with the
   valtron layer disabled.
2. **Client suite.** Run the reference client suite with the Rust adapter. The adapter drives the
   client core.
3. **Cross-runs.** Run our client against our server. Run the reference client against our
   server.
4. **Skip register.** Record every skipped test and its reason. A skip needs owner approval.
5. **Feature report.** Each run records the suite commit, the pass count and the failure list.
6. **CI.** A Linux job runs the server suite and the client suite. The job fails on any
   non-skipped failure.
7. **Conformance spike.** Before feature 06 is complete, a minimal in-memory server runs the
   server suite to find real failures early.
8. **No valtron in the gate.** The conformance jobs do not enable the valtron layer.
9. **Valtron suite.** A separate job runs the valtron layer behaviour tests.
10. **Runtime matrix.** The core and the client core run in a local embedded process, a DO, a
    Worker, a WebSocket server, wasm32 and the `foundation_ai` Session. Each target has a test.
11. **Surface neutrality.** A test calls `DurableService` with no HTTP adapter.
12. **BDD scenarios.** The behaviour suite is Given/When/Then. It calls the business API and runs
    on every driver.
13. **Driver matrix.** The same scenario runs on the embedded, HTTP, WebSocket, DO and Session
    drivers. Do not copy a scenario for each driver.

## 3. Design

```mermaid
flowchart TD
  A[Spec change] --> B[Unit and DST tests]
  B --> C[Server conformance: core + HTTP adapter]
  C --> D{All pass?}
  D -->|No| X[Fail]
  D -->|Yes| E[Client conformance: client core + adapter]
  E --> F{All pass?}
  F -->|No| X
  F -->|Yes| G[Cross-runs]
  G --> H{All pass?}
  H -->|No| X
  H -->|Yes| I[Valtron behaviour tests]
  I --> J[Ready]
```

_Caption: Conformance gates the merge. Valtron tests are separate._

## 4. Tasks

- [ ] **T1 — server suite harness.** Start the in-memory server and run the reference suite.
      Record every failure with a finding ID.
- [ ] **T2 — client suite harness.** Build the adapter and run the client suite against the
      reference server.
- [ ] **T3 — cross-runs.** Our client to our server. Reference client to our server.
- [ ] **T4 — CI.** Add the Linux jobs. Fail on a non-skipped failure.
- [ ] **T5 — skip register.** Record every skip and its reason. Get owner approval for each.
- [ ] **T6 — valtron suite.** Run the reader, writer, wait and wake tests separately.
- [ ] **T7 — report.** Publish the counts and the reference commit with each run.
- [ ] **T8 — runtime matrix.** Run the core in a local embedded process, a DO, a Worker, a
      WebSocket server and wasm32. Tests: `embedded_core_roundtrip`, `do_core_roundtrip`,
      `worker_core_roundtrip`, `websocket_core_roundtrip`, `wasm_core_roundtrip`.
- [ ] **T9 — `foundation_ai` Session.** Back a Session with the core. Test: `session_uses_core`.
- [ ] **T10 — surface neutrality.** Call `DurableService` with no HTTP adapter. Test:
      `service_runs_without_http`.
- [ ] **T11 — BDD suite.** Write the Given/When/Then scenarios for create, append, read, close,
      producer, `Stream-Seq`, fork and TTL.
- [ ] **T12 — driver matrix.** Run every scenario on the embedded, HTTP, WebSocket, DO and
      Session drivers. Test: `same_scenario_every_driver`.

## 5. Success criteria

- [ ] The server suite passes against `HttpAdapter` with valtron disabled.
- [ ] The client suite passes with the Rust adapter.
- [ ] Both cross-runs pass.
- [ ] Every skip has a recorded reason and owner approval.
- [ ] The runtime matrix passes: embedded, DO, Worker, WebSocket, wasm32 and Session.
- [ ] The valtron layer tests pass separately.
- [ ] CI fails on any non-skipped conformance failure.
