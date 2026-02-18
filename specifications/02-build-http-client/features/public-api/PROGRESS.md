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

- 2026-02-18T00:10Z — RETRIEVAL: Searched and read referenced code (evidence below). Based on retrieval-led reasoning (required by feature spec), I inspected the exact files listed in `machine_prompt.md` / `COMPACT_CONTEXT.md` and recorded concrete findings to guide implementation. No code was modified in this step.

  Evidence (file → short excerpt / symbol used):
  - Found `ClientRequest` implementation
    - backends/foundation_core/src/wire/simple_http/client/api.rs
      - Symbol: `pub fn introduction(&mut self) -> Result<(ResponseIntro, SimpleHeaders), HttpClientError>`
      - Use: This method already implements the "introduction()" behavior required by the spec and will be reused/adapted for the public API.
  - Found `SimpleHttpClient` implementation
    - backends/foundation_core/src/wire/simple_http/client/client.rs
      - Symbol: `pub struct SimpleHttpClient`
      - Use: Client builder methods and convenience verbs (`get`, `post`, etc.) are present and follow project patterns (builder chaining + `ClientConfig`).
  - Found a stubbed `ConnectionPool` with TODOs
    - backends/foundation_core/src/wire/simple_http/client/pool.rs
      - Symbols & lines:
        - `pub struct ConnectionPool` (stub)
        - `pub fn checkout(&self, _host: &str, _port: u16) -> Option<SharedByteBufferStream<RawStream>> {` (returns `None` - TODO)
        - Several `// TODO:` comments describing intended behavior
      - Use: Pool is intentionally stubbed; implementation must be added (spec allows optional/feature-gated pooling).
  - Module export already present
    - backends/foundation_core/src/wire/simple_http/mod.rs
      - Contains: `pub mod client;` and `pub use ...` re-exports
      - Use: No changes needed to expose the client module; verify re-exports in final step.
  - Diagnostics observed (informational for next implement step)
    - api.rs had lints/warnings reported (examples):
      - missing `# Errors` doc for a function returning `Result`
      - a redundant `continue` expression flagged
      - doc markdown items missing backticks
    - Use: Fix 1-2 diagnostics during implementation as mandated by rules (make 1-2 repair attempts, then defer if more complex).

  Short rationale and mapping to spec tasks:
  - The `ClientRequest` APIs required by the feature (introduction(), body(), send(), parts(), collect()) are already implemented in `client/api.rs`. I will reuse these implementations and ensure their signatures and visibility match the feature requirements and crate exports.
  - `SimpleHttpClient` (client/client.rs) already provides builder-style configuration and convenience methods (`get`, `post`, etc.). I will ensure it is generic over `R: DnsResolver` and that `ClientConfig` exposes the required fields (timeouts, redirects, pool toggles).
  - `ConnectionPool` is currently a stub; per the spec this is optional. I will implement a minimal, well-documented pooling layer (Arc + Mutex + HashMap per-host) and gate full behavior behind `ClientConfig.pool_enabled` and/or the existing `multi` feature flag. Tests for stub behavior exist and will be expanded.
  - `pub mod client;` is present in `simple_http/mod.rs`; no module wiring changes required now, but I will verify that `pub use` re-exports expose the public types as the spec expects.

  Retrieval commands used (record for verifier):
  - Searched repo for symbols and read files listed in `machine_prompt.md` / `COMPACT_CONTEXT.md`.
    - Examples of searches performed: looked for `ClientRequest`, `SimpleHttpClient`, `ConnectionPool`, `introduction(` in `backends/foundation_core/src/wire/simple_http/client/`.
  - Opened and read:
    - `backends/foundation_core/src/wire/simple_http/client/api.rs`
    - `backends/foundation_core/src/wire/simple_http/client/client.rs`
    - `backends/foundation_core/src/wire/simple_http/client/pool.rs`
    - `backends/foundation_core/src/wire/simple_http/mod.rs`
    - `specifications/02-build-http-client/features/public-api/feature.md` (spec)
    - `specifications/02-build-http-client/features/public-api/COMPACT_CONTEXT.md` (compact context)

- 2026-02-18T00:20Z — NEXT ACTIONS (ordered, retrieval-led)
  1. Generate and commit (if missing) `machine_prompt.md` and ensure `COMPACT_CONTEXT.md` is present (both are present in the feature folder). Confirm machine_prompt is the canonical short machine instruction to hand to sub-agents.
  2. Implement minimal `ConnectionPool` behavior (checkout/checkin + simple eviction) in `backends/foundation_core/src/wire/simple_http/client/pool.rs` while keeping current stub tests passing; mark pool feature as optional and gate heavy behavior.
  3. Re-run lints and fix 1-2 diagnostics found in `api.rs` (documented above). If fixes cascade into larger changes, stop after 1-2 attempts and document remaining issues for review.
  4. Add/adjust unit tests where public API signatures differ from spec expectations (update `client/api.rs` or `client/client.rs` visibility only if necessary).
  5. Run verification commands:
     - `cargo fmt -- --check`
     - `cargo clippy -- -D warnings` (fix up to 1-2 issues then report)
     - `cargo test --package foundation_core`
     - `cargo build --package foundation_core --features multi`
  6. Update `PROGRESS.md` with timestamps, exact commands run, file diffs (paths + symbol lines) and test outputs.

- 2026-02-18T00:25Z — RISKS / NOTES
  - Pool implementation may require additional helpers in `task.rs`/`connection.rs`—if so, I will make minimal changes and record them in the progress log.
  - Some clippy warnings in `api.rs` are documentation-only and low-risk; runtime-critical TODOs exist in `pool.rs` (explicitly noted).
  - Per `.agents/AGENTS.md` rules, verification agents will perform an "Incomplete Implementation Scan" that will fail if TODO/unimplemented markers remain in files under active feature scope. I will attempt to remove or mitigate the `TODO` markers in the pool implementation (1-2 attempts) or wrap unimplemented internals behind clear feature gating and tests that reflect current behavior.


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

- 2026-02-18T00:35Z — IMPLEMENTATION: Minimal ConnectionPool + verification runs

  Summary:
  - Implemented a minimal, testable `ConnectionPool` (checkout/checkin/cleanup/clear) to replace the previous stub and reduce incomplete-implementation surface for verification.
  - Performed workspace-scoped verification commands and addressed quick, low-risk clippy/doc issues that blocked progress.

  Changes made:
  - `backends/foundation_core/src/wire/simple_http/client/pool.rs` — added thread-safe pool implementation (Arc<Mutex<HashMap<...>>>), `new`, `checkout`, `checkin`, `cleanup_stale`, `clear`, and basic unit tests.
  - `backends/foundation_nostd/src/primitives/wait_duration.rs` — fixed doc-markdown by wrapping code identifiers in backticks.
  - `crates/watchers/src/handlers.rs` — added minimal `# Errors` / `# Panics` doc sections for public functions to satisfy clippy.
  - `crates/config/src/lib.rs` — added `# Errors` documentation for `value_from_path` and `from_path`.
  - `specifications/02-build-http-client/features/public-api/PROGRESS.md` — this progress entry (recording retrieval, implementation, and next steps).

  Commands executed (local runs):
  - `cargo fmt -- --check` → OK
  - `cargo clippy -- -D warnings` → Partial: resolved quick doc-markdown and missing-doc issues; full workspace clippy still reports warnings in other crates (e.g., `backends/foundation_wasm`) that require larger, separate fixes.
  - `cargo clippy -p foundation_core -- -D warnings` → Showed warnings in unrelated crates but allowed focused checks on `foundation_core`.
  - `cargo test --package foundation_core` → OK (foundation_core unit tests passed)
  - `cargo build --package foundation_core --features multi` → OK (built with warnings)

  Notes / rationale:
  - The public API (`ClientRequest` and `SimpleHttpClient`) already exists and aligns with the feature spec; I reused the existing implementations instead of reimplementing them.
  - The prior `ConnectionPool` stub contained TODOs that would cause verification to fail; the minimal pool reduces that risk and provides a testable surface for now. Full-featured pooling (LRU, background cleanup, async optimizations) is a later enhancement.
  - I limited changes to quick, low-risk edits required to proceed (docs and light fixes). Large-scale clippy fixes in other crates were intentionally deferred to avoid scope creep.

  Next steps (recommended, in order):
  1. Spawn a verification agent to run the "Incomplete Implementation Scan" (TODO/unimplemented markers) and the verification workflow described in `.agents/rules/08-verification-workflow-complete-guide.md`. Provide the agent with:
     - Files to verify: the client module (`backends/foundation_core/src/wire/simple_http/client/*`) and this spec folder.
  2. If verification reports remaining incomplete implementations relevant to `public-api` (redirect handling TODO in `task.rs`), implement the minimal redirect-handling state per the TODO action list in `task.rs` or document as an explicit allowed TODO with gating.
  3. If a full workspace clippy pass is required, create a separate task to address the larger warnings in `backends/foundation_wasm` and other crates.
  4. After verification agent PASS, add any additional integration tests that exercise `SimpleHttpClient` end-to-end (plain HTTP and TLS under feature flags) as needed.

  Files touched (quick list):
  - backends/foundation_core/src/wire/simple_http/client/pool.rs
  - backends/foundation_nostd/src/primitives/wait_duration.rs
  - crates/watchers/src/handlers.rs
  - crates/config/src/lib.rs
  - specifications/02-build-http-client/features/public-api/PROGRESS.md

End of PROGRESS.md for `public-api`.