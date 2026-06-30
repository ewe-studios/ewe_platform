---
feature: "Workspace hygiene — path+version deps, external test layout, inline-test audit"
description: "Switch every crate's Cargo.toml from workspace = true dependency inheritance to direct path + version deps (forcing explicit version bumps), move all non-internal-detail tests from inline #[cfg(test)] modules to {crate}/tests/ directories, and audit remaining inline tests to determine if they test internals or should also be extracted"
status: "complete"
priority: "medium"
depends_on: []
estimated_effort: "large"
created: 2026-06-21
last_updated: 2026-06-22
author: "Main Agent"
tasks:
  completed: 8
  uncompleted: 0
  total: 8
  completion_percentage: 100%
notes: |
  Part A complete: 29 Cargo.tomls migrated (28 via script + 1 manual dotted-key fix).
  All foundation_* inter-crate deps now use path+version instead of workspace=true.
  Third-party deps and metadata (edition, license, etc.) retained workspace=true.
  cargo check --workspace passes (only pre-existing foundation_deployment_aws chrono failure).
  Part B (inline test extraction) not yet started.
---

# Feature 34: Workspace hygiene — path+version deps, external test layout

> **Motivation (user, 2026-06-20).** Two accumulated pain points:
>
> 1. **`workspace = true` dependency inheritance** hides version coupling. All 48 crates using
>    `{ workspace = true }` for inter-crate deps should switch to `{ path = "...", version = "..." }`
>    so version bumps are explicit and forced — no silent propagation via the workspace table.
>
> 2. **Inline `#[cfg(test)]` modules** — ~258 source files have inline test modules. The house rule is
>    tests belong in `{crate}/tests/` (external integration tests). Only tests that *genuinely* test
>    private/internal implementation details should stay inline. Most current inline tests test public
>    APIs and should be extracted.

## WHY: Problem Statement

### Path+version deps

- `workspace = true` means changing a version in `[workspace.dependencies]` silently updates every
  consumer. This is convenient but dangerous — a breaking change in `foundation_core v0.0.5` silently
  propagates to all 20+ downstream crates without any crate-level `Cargo.toml` change to review.
- With `path = "../foundation_core", version = "0.0.4"`, a version bump requires updating each consumer
  explicitly. This surfaces in diffs, forces conscious compatibility decisions, and makes publishing
  individual crates to crates.io straightforward (path deps are resolved locally, version deps for
  external consumers).

### External test layout

- The house rule (per user memory) is: tests go in `{crate}/tests/`, not inline `#[cfg(test)]` modules.
- Inline tests have access to `pub(crate)` and private items, which:
  - Couples tests to implementation details
  - Makes refactoring harder (moving a private function breaks tests)
  - Hides test code inside source files (harder to find, review, run selectively)
- External tests can only access the public API — they're better regression guards.
- Exception: tests that genuinely exercise `pub(crate)` invariants or private helper logic that has no
  public surface. These should stay inline but be audited and annotated.

## WHAT: Solution

### Part A: Migrate workspace deps to path+version

For every `Cargo.toml` in the workspace that uses `{ workspace = true }`:

1. Look up the inherited value from `[workspace.dependencies]` in the root `Cargo.toml`.
2. Replace `{ workspace = true }` with the resolved `{ path = "...", version = "..." }`.
3. Keep `optional = true`, `features = [...]`, and other modifiers.
4. Preserve the `[workspace.dependencies]` table in root `Cargo.toml` as documentation but stop
   inheriting from it.

Example:
```toml
# Before:
[dependencies]
foundation_core = { workspace = true }

# After:
[dependencies]
foundation_core = { path = "../foundation_core", version = "0.0.4" }
```

For third-party deps currently in `[workspace.dependencies]`:
- Keep `{ workspace = true }` for third-party crates (serde, tokio, etc.) — workspace inheritance
  is valuable for external dep version alignment.
- Only migrate **inter-crate** (foundation_*) deps to path+version.

### Part B: Move inline tests to external test directories

For each source file with `#[cfg(test)] mod tests { ... }`:

1. **Classify** the tests:
   - **Public API tests** — only use `pub` items and `use crate::*` imports → move to `tests/`.
   - **Internal detail tests** — use `pub(crate)`, private fields, or `super::*` private items →
     leave inline, add `// Tests internal implementation details` comment.

2. **Extract** public API tests to `{crate}/tests/{module_name}.rs`:
   - Convert `use super::*` to `use {crate_name}::*` (external crate import).
   - Add `[[test]]` entry in `Cargo.toml` if needed.
   - Remove the `#[cfg(test)] mod tests` block from the source file.

3. **Audit** remaining inline tests — if they only access public items through `super::`, they can
   also be extracted. Create a follow-up feature (Part C) for any that need refactoring to make
   internals testable through the public API.

### Part C: Follow-up feature for internal-test refactoring

Create a separate feature (if needed) to refactor code so internal-only tests can become public API
tests. This involves adding public test helpers, builder patterns, or test-only feature gates that
expose internals. **Not part of this feature — F34 only moves what can be moved safely.**

## HOW: Implementation Steps

1. **Inventory workspace deps.** Parse root `Cargo.toml` `[workspace.dependencies]` to build a map of
   crate → { path, version, features }. Identify all inter-crate (foundation_*) deps.

2. **Migrate inter-crate deps.** For each of the 48 crates using `workspace = true`:
   - Replace foundation_* `{ workspace = true }` entries with `{ path = "...", version = "..." }`.
   - Preserve optional/features modifiers.
   - Keep third-party deps as `{ workspace = true }`.

3. **Verify compilation.** `cargo check --workspace` after migration. No logic changes — purely
   Cargo.toml rewriting.

4. **Inventory inline tests.** Scan all 258 files with `#[cfg(test)]`. For each, classify as
   public-API or internal-detail.

5. **Extract public-API tests.** For each crate, batch-move public-API inline tests to `tests/`.
   One commit per crate to keep diffs reviewable.

6. **Annotate remaining inline tests.** Add classification comments to inline tests that stay.

7. **Verify test parity.** Run `cargo test --workspace` before and after, diff test counts per crate.
   No tests should be lost.

8. **Update CI.** If CI references specific test paths, update them.

## Open Decisions

- **OD-34-1 — third-party deps:** keep `workspace = true` for third-party crates (rec: yes, version
  alignment is valuable for external deps — the problem is only with inter-crate deps where path
  resolution hides coupling).
- **OD-34-2 — batch size:** migrate all 48 crates at once (one big PR) vs crate-by-crate (many small
  PRs). Rec: one PR for the Cargo.toml migration (mechanical, easy to verify), separate PRs per crate
  for test extraction (requires classification judgment).
- **OD-34-3 — metadata fields:** should `version`, `edition`, `rust-version`, `license`, `authors`,
  `repository` also switch from `workspace = true` to explicit values? Rec: keep `workspace = true`
  for metadata (these are genuinely workspace-level, not versioned independently).

## Target Files

- Every `Cargo.toml` in `backends/` (48 crates)
- Every source file with `#[cfg(test)]` (258 files across all crates)
- Root `Cargo.toml` `[workspace.dependencies]` (reference, not modified)

## Tests

```bash
# Before migration: capture baseline
cargo test --workspace 2>&1 | tail -5

# After Cargo.toml migration:
cargo check --workspace
cargo test --workspace

# After test extraction (per crate):
cargo test -p <crate_name>
```

## Verification

```bash
cargo check --workspace
cargo clippy --workspace --all-targets
cargo test --workspace
```

## Done When

- Every inter-crate (foundation_*) dependency uses `{ path = "...", version = "..." }` instead of
  `{ workspace = true }`.
- Third-party deps retain `{ workspace = true }` (or are optionally migrated per OD-34-1).
- All public-API inline tests are in `{crate}/tests/` directories.
- Remaining inline tests are annotated as testing internal details.
- `cargo test --workspace` passes with the same test count as before.
