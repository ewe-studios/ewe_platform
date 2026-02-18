---
feature: public-api
path: specifications/02-build-http-client/features/public-api/feature.md
status: in-progress
started: 2026-02-18
last_updated: 2026-02-18
author: Implementation Agent (placeholder)
progress:
  completed_tasks: 0
  total_tasks: 17
  percent_complete: 0
context_optimization_required: true
compact_context_file: ./COMPACT_CONTEXT.md
machine_prompt_file: ./machine_prompt.md
---

# PROGRESS — Public API (SimpleHttpClient / ClientRequest)

This file is the canonical, ephemeral progress record for the `public-api` feature while it is being implemented. Follow the "Retrieval-Led Reasoning" and token/context optimization rules in the feature spec before making code changes.

Keep this file up-to-date. When progress is made, append a timestamped entry describing:
- what you read (paths and short rationale),
- what you changed (file paths + short summary),
- verification commands run and results,
- next immediate action.

---

## Current status (high-level)

- Overall status: in-progress (implementation not started)
- Tasks completed: 0 / 17
- Percent complete: 0%
- Blocking issues: none currently blocking, but MUST perform thorough retrieval in codebase before any implementation.

---

## Immediate next actions (ordered — retrieval-led)

1. Search the codebase for existing client patterns and types:
   - Look for `simple_http` module layout and existing client implementations.
   - Files to inspect (minimum):
     - `backends/foundation_core/src/wire/simple_http/mod.rs`
     - `backends/foundation_core/src/wire/simple_http/impls.rs`
     - `backends/foundation_core/src/wire/simple_http/client/api.rs` (existing ClientRequest internals)
     - `backends/foundation_core/src/wire/simple_http/client/*` (other client internals)
     - `specifications/02-build-http-client/requirements.md`
     - Dependent feature specs: `foundation`, `connection`, `request-response`, `task-iterator`
2. Collect patterns for:
   - Naming conventions
   - Error types and propagation (`HttpClientError` etc.)
   - Test structure and harness used in `simple_http` tests
   - Module export patterns (`pub mod` usage, `mod.rs` structure)
3. Produce `machine_prompt.md` (pipe-delimited compressed summary of this feature) and commit it alongside `feature.md`.
4. Produce initial `COMPACT_CONTEXT.md` containing:
   - Embedded `machine_prompt.md` content (current task only)
   - Files list to read
   - Current PROGRESS snapshot
5. Implement minimal public API files (create stubs) in `backends/foundation_core/src/wire/simple_http/`:
   - `client/api.rs` (public wrapper types and docs) — note: an internal `api.rs` already exists; reconcile with spec
   - `client/client.rs` (SimpleHttpClient)
   - `client/pool.rs` (ConnectionPool — optional, feature-guarded)
6. Wire `pub mod client;` into `simple_http/mod.rs` (if not already present) and ensure `pub mod client` is exported from crate root.
7. Add feature flags to `Cargo.toml`:
   - Add `multi` and SSL feature groups described by spec
8. Add unit tests and integration tests scaffolding; run `cargo fmt`, `cargo clippy`, `cargo test` iteratively.

Note: Steps 1–4 are mandatory before any coding (retrieval-led reasoning).

---

## Retrieval checklist (to be completed and recorded here)

- [ ] What similar features exist in this project? (grep + file list)
- [ ] What patterns do they follow? (short summary + file references)
- [ ] What naming conventions are used? (observed from code)
- [ ] How are errors handled in similar code? (file references)
- [ ] What testing patterns exist? (test files + how they run)
- [ ] Are there helper functions to reuse? (list symbols and files)

When you complete each item above, paste the minimal evidence (file paths and the specific lines or symbols you relied on) under a timestamped progress entry.

---

## Implementation task list (derived from feature requirements)

Each list item should be checked off with a timestamped progress entry when completed.

- [ ] Create `backends/foundation_core/src/wire/simple_http/client/client.rs` — `SimpleHttpClient` and builder methods
- [ ] Create/adjust `backends/foundation_core/src/wire/simple_http/client/api.rs` — public `ClientRequest` methods:
      - `introduction()`
      - `body()`
      - `send()`
      - `parts()` -> iterator adapter
      - `collect()`
- [ ] Create `backends/foundation_core/src/wire/simple_http/client/pool.rs` — `ConnectionPool` (feature-gated / optional)
- [ ] Add `pub mod client` to `backends/foundation_core/src/wire/simple_http/mod.rs` (if missing)
- [ ] Update crate-level exports to expose the new public API
- [ ] Add feature flags to `Cargo.toml` (`multi`, `ssl-rustls`, `ssl-openssl`, `ssl-native-tls`)
- [ ] Ensure generic `DnsResolver` parameterization is present on `SimpleHttpClient`
- [ ] Reuse existing types: `SimpleResponse`, `IncomingResponseParts`, `PreparedRequest`, `ClientRequestBuilder`
- [ ] Add unit tests for `ClientRequest` and `SimpleHttpClient` (happy path + failure)
- [ ] Add integration tests that exercise plain HTTP end-to-end
- [ ] Add TLS-enabled integration tests behind feature flags (when TLS features enabled)
- [ ] Run and pass `cargo fmt`, `cargo clippy` (warnings treated as errors)
- [ ] Update documentation comments and README where applicable

---

## Verification commands (to run locally while implementing)

- cargo fmt -- --check
- cargo clippy -- -D warnings
- cargo test --package foundation_core
- cargo build --package foundation_core
- cargo build --package foundation_core --features multi
- cargo build --package foundation_core --features ssl-rustls
- cargo build --package foundation_core --all-features

Record outputs of the commands you run in this file under a timestamped entry.

---

## Risks / Notes / Assumptions

- MUST follow retrieval-led reasoning. Implementation without reading existing code is forbidden.
- There is an existing `client/api.rs` file containing a mostly complete `ClientRequest` implementation. The implementer must reconcile the spec with that existing file (prefer reusing/adjusting existing implementations rather than re-implementing from scratch).
- Token/context optimization protocols (machine_prompt.md and COMPACT_CONTEXT.md) must be followed for sub-agents.
- Connection pooling is optional — implement minimal pool interface and gate its behavior behind `ClientConfig.pool_enabled` or a feature flag as appropriate.
- Keep public API ergonomic and hide TaskIterator complexities.

---

## Progress log (chronological)

- 2026-02-18T00:00Z — CREATED: Initial PROGRESS.md (this file). No retrieval or code changes performed yet. Next: run retrieval (grep/read) and produce machine_prompt.md + COMPACT_CONTEXT.md before coding.

(When you make changes: append entries in the same format with timestamps and the minimal retrieval evidence.)

---

## Where to record evidence

When you complete each retrieval step, update this PROGRESS.md with:
- the exact command used (e.g., `git grep 'ClientRequest'` or `rg 'SimpleHttpClient'`),
- the file paths you opened,
- the short quote or symbol used to derive the decision,
- a short note: "I will reuse X from file Y because ..."

This record is mandatory per the feature spec and will be examined by verification agents.

---

End of PROGRESS.md for `public-api`.