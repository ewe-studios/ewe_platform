# COMPACT_CONTEXT.md
feature: public-api
source: specifications/02-build-http-client/features/public-api/feature.md
created: 2026-02-18
generator: main-agent (machine-optimized embed)
purpose: Ultra-compact self-contained context for sub-agents implementing `public-api` feature.
note: Sub-agents MUST clear their context and reload only from this file before beginning work.

--------------------------------------------------------------------------------
MACHINE_PROMPT (pipe-delimited compressed representation)
--------------------------------------------------------------------------------
TASK|Implement public user-facing HTTP client API (ClientRequest, SimpleHttpClient, optional ConnectionPool, module integration).|GOAL|Hide TaskIterator internals; provide ergonomic methods: introduction(), body(), send(), parts(), collect(), convenience verbs (get/post/put/etc.), builder/config methods, pooling toggle.|CONSTRAINTS|Follow existing project patterns; use retrieval-led reasoning; generate machine_prompt.md & COMPACT_CONTEXT.md lifecycle rules; obey .agents rules.|VERIFICATION|Unit + integration tests, cargo fmt/clippy, build with features (multi, ssl-*).|ERRORS|Reuse foundation errors, consistent HttpClientError mapping.|FILES|spec/feature.md|required code files in backends/foundation_core: src/wire/simple_http/mod.rs and client/*.|TIMEBOX|Start by searching codebase for similar APIs then implement small, testable steps.

--------------------------------------------------------------------------------
CURRENT STATUS (minimal progress snapshot)
--------------------------------------------------------------------------------
- From `feature.md`: tasks.total=17, completed=0, priority=high.
- Context-optimization required: sub-agents MUST generate their own COMPACT_CONTEXT.md before work and reload from it.
- No PROGRESS.md present in spec folder; assume initial state (work not started).
- Verification and implementation agents must follow .agents rules referenced in feature.md.

--------------------------------------------------------------------------------
FILES TO READ (only these; read and reference exact paths)
--------------------------------------------------------------------------------
1. specifications/02-build-http-client/features/public-api/feature.md
2. specifications/02-build-http-client/requirements.md
3. backends/foundation_core/src/wire/simple_http/mod.rs
4. backends/foundation_core/src/wire/simple_http/client/api.rs
5. backends/foundation_core/src/wire/simple_http/client/impls.rs
6. backends/foundation_core/src/wire/simple_http/client/pool.rs (if exists)
7. backends/foundation_core/src/wire/simple_http/client/errors.rs
8. .agents/stacks/rust.md
9. .agents/rules/14-machine-optimized-prompts.md
10. .agents/rules/15-instruction-compaction.md

(Only load the files above. Do not load the entire repo.)

--------------------------------------------------------------------------------
MANDATORY RULES (must be followed)
--------------------------------------------------------------------------------
- Retrieval-led reasoning: show grep/find/read steps and reference exact files in reports.
- Machine prompt lifecycle: generate `machine_prompt.md` from `feature.md` and commit it with human file.
- COMPACT_CONTEXT lifecycle: generate fresh per task, embed machine_prompt section for current task, contain only 500-800 tokens ideally.
- Clear and reload context from COMPACT_CONTEXT.md before work (sub-agent enforcement).
- Follow repository naming, error handling, and test patterns observed in `client/api.rs` and `impls.rs`.
- Make 1-2 attempts at fixing diagnostics; then defer to user.

--------------------------------------------------------------------------------
IMMEDIATE NEXT ACTIONS (for implementation agent)
--------------------------------------------------------------------------------
1. Search (grep) for existing `ClientRequest`, `SimpleHttpClient`, `ConnectionPool`, and similar public APIs — record matches and patterns. Reference exact files found.
2. Read the files in FILES TO READ (above). Extract naming conventions, error enums, iterator patterns, and tests.
3. Produce `machine_prompt.md` from `feature.md` using pipe-delimited compression (58% token reduction). Commit alongside human file.
4. Generate updated COMPACT_CONTEXT.md (this file) embedding the machine_prompt content and current PROGRESS.md segment once PROGRESS.md exists.
5. Implement public API scaffolding:
   - Create `client/api.rs` (if missing) exposing `ClientRequest` wrappers matching patterns in `client/api.rs` (existing file present; reuse).
   - Create `client/client.rs` for `SimpleHttpClient` and builder methods.
   - Add `pool.rs` if enabling pooling.
6. Write unit tests mirroring patterns in `client/api.rs` tests; run `cargo fmt`, `cargo clippy`, `cargo test`.
7. Update `simple_http/mod.rs` to `pub mod client;` integration already present—ensure `pub mod client` exported.
8. Update Cargo.toml features: add `multi` and `ssl-*` feature flags per feature.md.
9. Report progress by creating PROGRESS.md summarizing actions taken and next steps; regenerate COMPACT_CONTEXT.md and clear/reload context.

--------------------------------------------------------------------------------
VERIFICATION AGENT NOTES (short)
--------------------------------------------------------------------------------
- Use existing tests and add coverage for:
  - `introduction()` returns (ResponseIntro, SimpleHeaders)
  - `body()` returns SimpleBody
  - `send()` returns SimpleResponse<SimpleBody>
  - `parts()` yields IncomingResponseParts iterator
  - Builder and convenience methods compile and behave
- Run verification commands listed in feature.md.

--------------------------------------------------------------------------------
LIMITS & FORMAT
--------------------------------------------------------------------------------
- This file is intentionally minimal. Keep responses and commits concise and reference exact files/lines when reporting.
- When producing code snippets in reports, reference file paths and line ranges using project path style.

--------------------------------------------------------------------------------
CONTACT / AUTHOR
--------------------------------------------------------------------------------
author: Main Agent
last_updated: 2026-02-18