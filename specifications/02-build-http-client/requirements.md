---
# Identification
spec_name: "02-build-http-client"
spec_number: 02
description: Create an HTTP/1.1 client reusing the existing simple_http module structures, using iterator-based patterns with valtron executors and pluggable TLS/DNS/resolution components.

# Location Context
workspace_name: "ewe_platform"
spec_directory: "specifications/02-build-http-client"
this_file: "specifications/02-build-http-client/requirements.md"

# Status
status: in-progress
priority: high
created: 2026-01-18
author: Main Agent

# Context Optimization (MANDATORY)
machine_optimized: true
machine_prompt_file: ./machine_prompt.md
context_optimization: true
compact_context_file: ./COMPACT_CONTEXT.md
context_reload_required: true

# Metadata
metadata:
  version: '5.3' # Revised to align with agent rules & verification workflow
  last_updated: 2026-02-18
  estimated_effort: medium
  tags:
    - http-client
    - networking
    - rust
    - iterator-patterns
    - valtron-executors
  stack_files:
    - .agents/stacks/rust.md
  skills: []
  tools:
    - Rust
    - cargo

# Dependencies
builds_on:
  - ../04-condvar-primitives
related_specs:
  - ../03-wasm-friendly-sync-primitives

# Structure
has_features: true
has_fundamentals: true

# Progress Tracking
features:
  completed: 6
  uncompleted: 8
  total: 14
  completion_percentage: 43

# Files & Rules Required by Agents (MUST match .agents/AGENTS.md)
files_required:
  main_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/05-coding-practice-agent-orchestration.md
      - .agents/rules/06-specifications-and-requirements.md
      - .agents/rules/14-machine-optimized-prompts.md
      - .agents/rules/15-instruction-compaction.md
    files:
      - ./requirements.md
      - ./LEARNINGS.md
      - ./PROGRESS.md
      - ./machine_prompt.md
      - ./COMPACT_CONTEXT.md

  implementation_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/13-implementation-agent-guide.md
      - .agents/rules/14-machine-optimized-prompts.md
      - .agents/rules/15-instruction-compaction.md
      - .agents/stacks/rust.md
    files:
      - ./machine_prompt.md
      - ./COMPACT_CONTEXT.md
      - ./features/*/feature.md

  verification_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/08-verification-workflow-complete-guide.md
      - .agents/rules/14-machine-optimized-prompts.md
      - .agents/rules/15-instruction-compaction.md
      - .agents/stacks/rust.md
    files:
      - ./requirements.md
      - ./PROGRESS.md
      - ./COMPACT_CONTEXT.md

# Important: Retrieval-Led Reasoning (MANDATORY)
All agents working on this specification MUST follow retrieval-led reasoning and demonstrate it in their work logs and PR descriptions.

Before modifying or implementing any code you MUST (in this order):
1. Search the codebase for similar implementations (use `grep`, `rg`, `find_path`)
2. Read the source files you discover and extract relevant patterns
3. Read `.agents/stacks/rust.md` for language and crate conventions
4. Read module documentation listed below
5. Validate assumptions against actual code and document file references used

Forbidden:
- Implementing based on pretraining assumptions without concrete references
- "I assumed..." statements in PRs or verification reports — every decision must cite discovered code references

# Machine-Optimized Prompts & Context Compaction (RULE 14 & RULE 15) — MANDATORY
Main Agent responsibilities before spawning sub-agents:
1. Generate `machine_prompt.md` derived from this `requirements.md` (machine-optimized).
2. Commit `machine_prompt.md` together with human-readable `requirements.md`.
3. Create an initial `COMPACT_CONTEXT.md` containing:
   - the embedded machine_prompt for the current task
   - FILES list required for the task
   - a short PROGRESS summary (target 500–800 tokens)
4. CLEAR the agent context and RELOAD from `COMPACT_CONTEXT.md` before spawning sub-agents.

Sub-agent responsibilities on startup:
1. Load the provided `COMPACT_CONTEXT.md` (self-contained) and then only the FILES listed in it.
2. When PROGRESS changes, regenerate `COMPACT_CONTEXT.md`, CLEAR context, and RELOAD from it.
3. Do not read the full human-readable spec unless explicitly listed in FILES.

COMPACT_CONTEXT.md lifecycle:
- Generated per active task
- Regenerated after each PROGRESS update
- Deleted or archived when task completes
- Embeds only the current task (no full history)

See `.agents/rules/14-machine-optimized-prompts.md` and `.agents/rules/15-instruction-compaction.md` for exact formatting and lifecycle details.

# CRITICAL: Verification Agent Workflow (MANDATORY)
Main Agent MUST spawn verification agents with the exact instructions specified in `.agents/rules/08-verification-workflow-complete-guide.md`. Verification MUST perform the FIRST MANDATORY CHECK described below before any other checks.

FIRST MANDATORY CHECK — Incomplete Implementation Scan:
- Scan all modified/affected files for:
  - `TODO`, `FIXME`, `unimplemented!()`, `todo!()`
  - Stub or placeholder implementations (returns default values without real logic)
  - State-machine states that are left unimplemented or perpetually pending
- If ANY incomplete implementation markers or stubs are found → verification MUST FAIL immediately.

Suggested verification commands (to be executed by verification agent in the workspace or on the modified files path):
```bash
# Repo-wide or modified-files scan
grep -rn "TODO\|FIXME\|unimplemented!\|todo!" <path>

# Rust-specific fast scan (ripgrep)
rg "unimplemented!\|todo!\|FIXME|TODO" --type rust <path>
```

Verification agent MUST:
1. Include the Incomplete Implementation Scan as Check #1 in the report
2. Only run other checks if Check #1 passes
3. Return PASS only if ALL checks pass
4. Return FAIL if any check fails (including incomplete implementations)

# Sub-Agent Spawning (Constraints)
When spawning sub-agents, the Main Agent MUST:
- Instruct sub-agent to load Rules: 01–04, 14, 15, 12 and the sub-agent doc at `.agents/agents/[name].md`
- Provide `machine_prompt.md` and `COMPACT_CONTEXT.md` paths
- Forbid sub-agents from committing directly to the repo
- Forbid sub-agents from spawning verification agents (only Main Agent may spawn them)
- Require sub-agents to compact context as per Rule 15 and regenerate `COMPACT_CONTEXT.md` after PROGRESS updates

# Overview & High-Level Approach
Goal: Implement a robust, idiomatic HTTP/1.1 client that:
- Reuses `simple_http` module shapes and types
- Uses iterator-based streaming patterns internally (TaskIterator / ExecutionAction) — the public API must be ergonomic; async/await may be used internally where justified, but task iterator patterns are preferred for the module design
- Uses `valtron` executors (`single` / `multi`) with a feature flag (e.g., `multi`) to select multi-threaded execution
- Provides a pluggable DNS resolver trait and default implementations
- Supports optional connection pooling (configurable), redirect handling, proxy support, and compression
- Reuses TLS functionality from `netcap` and foundation core crates
- Prefers generic types to avoid unnecessary boxing in hot paths, but favors ergonomics on the public API surface

Implementation location and module layout guidance:
- Primary implementation: `backends/foundation_core/src/wire/simple_http/client/`
- Feature files: implemented and documented under `specifications/02-build-http-client/features/*/feature.md`
- Documentation: update `documentation/simple_http/doc.md`, `documentation/valtron/doc.md` and `documentation/netcap/doc.md` as changes land

### Feature Index (authoritative feature files live in features/*)

Implement features in dependency order. Each feature must include its own `files_required` and verification steps. Use the feature files under `specifications/02-build-http-client/features/*/feature.md` as the authoritative source of per-feature tasks and verification commands.

| #  | Feature | Description | Dependencies | Status |
|----|---------|-------------|--------------|--------|
| 0  | [valtron-utilities](./features/valtron-utilities/feature.md) | Reusable ExecutionAction types, unified executor, and state machine helpers | None | ✅ Complete |
| 1  | [tls-verification](./features/tls-verification/feature.md) | Verify and fix TLS backends (rustls, openssl, native-tls) | 0 | ✅ Complete |
| 2  | [foundation](./features/foundation/feature.md) | Error types, DNS resolution, and common foundations | 1 | ✅ Complete |
| 3  | [compression](./features/compression/feature.md) | gzip, deflate, brotli support and streaming integration | 2 | ⬜ Pending |
| 4  | [connection](./features/connection/feature.md) | URL parsing, TCP, TLS handshakes (HTTP/HTTPS connection layer) | 2 | ✅ Complete |
| 5  | [proxy-support](./features/proxy-support/feature.md) | HTTP/HTTPS/SOCKS5 proxy handling and configuration | 4 | ⬜ Pending |
| 6  | [request-response](./features/request-response/feature.md) | Request builder, response types, headers and body handling | 4 | ✅ Complete |
| 7  | [auth-helpers](./features/auth-helpers/feature.md) | Basic, Bearer, Digest auth helpers and flows | 6 | ⬜ Pending |
| 8  | [task-iterator](./features/task-iterator/feature.md) | TaskIterator, ExecutionAction types and executor integration | 0, 6 | ✅ Complete |
| 9  | [public-api](./features/public-api/feature.md) | User-facing API (SimpleHttpClient), ergonomics and integration | 8 | ⬜ Pending |
| 10 | [connection-pooling](./features/connection-pooling/feature.md) | Connection pool design, checkout/checkin, cleanup and metrics | 4 | ⬜ Pending |
| 11 | [cookie-jar](./features/cookie-jar/feature.md) | Automatic cookie storage and policy handling | 9 | ⬜ Pending |
| 12 | [middleware](./features/middleware/feature.md) | Request/response interceptors and middleware pipeline | 9 | ⬜ Pending |
| 13 | [websocket](./features/websocket/feature.md) | WebSocket client and server | 4, 9 | ⬜ Pending |

Status Key: ⬜ Pending | 🔄 In Progress | ✅ Complete

Notes:
- Implement features in dependency order. Do not start a feature until its dependencies are verified present and functional.
- Each `feature.md` must include:
  - A `files_required` frontmatter section listing rules and files for implementation and verification agents.
  - Verification commands and expected outcomes.
  - Sample PROGRESS.md updates to drive COMPACT_CONTEXT regeneration.
- Features that change shared types MUST coordinate with `foundation` and `valtron-utilities` crates and add integration tests.
- Implement features in dependency order. Do not start on a feature until its dependencies are verified present and functional.
- Each `feature.md` must include:
  - A `files_required` frontmatter section listing rules and files for implementation and verification agents.
  - Verification commands and expected outcomes.
  - Sample PROGRESS.md updates to drive COMPACT_CONTEXT regeneration.
- Features that change shared types MUST coordinate with `foundation` and `valtron-utilities` crates and add integration tests.

# Success Criteria (Spec-wide)
- All feature `feature.md` files completed and marked done
- All unit and integration tests pass for affected crates
- Zero warnings from `cargo clippy -- -D warnings` for impacted crates
- `cargo test --package foundation_core` and related crate tests pass
- `cargo fmt -- --check` passes
- End-to-end integration tests demonstrate:
  - Basic HTTP requests (GET, POST)
  - TLS + connection pooling + redirects + compression interoperability
  - Proxy behavior (HTTP/HTTPS/SOCKS) where configured
- Documentation:
  - `LEARNINGS.md` updated with design decisions and trade-offs
  - `VERIFICATION.md` produced by the verification agent with Check #1 included
  - `REPORT.md` created at completion
  - `fundamentals/00-overview.md` created when public API is stable

# Module Documentation References (MUST be read before coding)
- `documentation/simple_http/doc.md`
- `documentation/valtron/doc.md`
- `documentation/netcap/doc.md`
- `.agents/stacks/rust.md`

# Verification Checklist (template for Verification Agent after passing incomplete-implementation scan)
- Build: `cargo build --workspace --locked`
- Format: `cargo fmt --all -- --check`
- Lints: `cargo clippy --all -- -D warnings`
- Tests: `cargo test --workspace`
- Integration: run end-to-end HTTP verification tests described in `features/*/feature.md`
- Report: produce `VERIFICATION.md` with:
  - Check #1: Incomplete Implementation Scan (results)
  - Check #2..N: Build, Lints, Tests, Integration results
  - Final PASS/FAIL and actionable remediation steps

# Lifecycle & Maintenance Notes
- `machine_prompt.md` MUST be regenerated and committed whenever `requirements.md` changes.
- `COMPACT_CONTEXT.md` must be maintained while active work is in progress and regenerated after PROGRESS updates.
- Main Agent is responsible for spawning verification agents only after implementation is marked ready in `PROGRESS.md`.
- Do not merge changes without a passing verification agent report.

_Created: 2026-01-18_
_Last Revised: 2026-02-18 (aligned with agent rules, verification workflow, and context compaction lifecycle)_
---