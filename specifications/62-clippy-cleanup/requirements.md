---
description: "Fix all cargo check and clippy warnings across the workspace; only four specific allows are permitted"
status: "in-progress"
priority: "high"
created: 2026-07-23
author: "Main Agent"
metadata:
  version: "1.0"
  last_updated: 2026-07-23
  estimated_effort: "large"
  tags:
    - cleanup
    - clippy
    - warnings
    - correctness
  skills: []
  tools: []
has_features: false
has_fundamentals: false
builds_on: "61-llama-vendoring-and-crate-publishing"
related_specs:
  - "61-llama-vendoring-and-crate-publishing"
---

# Overview

Zero the warning count across the workspace. Every `cargo check` warning and
every `cargo clippy` lint is either fixed or given a deliberate, justified
`#[allow]` — and the allowlist is locked to four lints. No blanket suppression,
no file-level `#![allow(clippy::all)]`, no warn-level trickery.

# Problem

The workspace has accumulated warnings over multiple features and branches.
These fall into three categories:

1. **Benign noise** — lints like `too_many_lines` and `too_many_arguments` that
   fire on valid, well-structured code and would make things worse if "fixed."
2. **Real bugs waiting to happen** — `cast_possible_wrap`, `cast_possible_truncation`,
   `unused_mut`, `dead_code`, etc. These can mask overflow, logic errors, and
   stale code paths.
3. **Cargo-level warnings** — unused manifest keys, stale dep specs, build-script
   diagnostics that should be `cargo:warning=` or silenced.

The goal is zero warnings after a clean `cargo check --profile uat` and
`cargo clippy --profile uat --all-features`, with only the four approved allows
remaining in the codebase.

# Decisions

| # | Decision | Rationale |
|---|---|---|
| 1 | Fix, never suppress, for `cast_possible_wrap` and `cast_possible_truncation` | These hide real overflow/truncation bugs. Use `try_from`/`from` or explicit narrowing with a comment |
| 2 | Fix, never suppress, for `unused_mut`, `unused_variables`, `dead_code` | These are always safe to remove |
| 3 | Allowlist exactly four lints: `too_many_lines`, `too_many_arguments`, `cast_sign_loss`, `cast_precision_loss` | These fire on intentional patterns that cannot be meaningfully refactored without making code worse |
| 4 | All other lints must be either fixed or given a single-site `#[allow]` with a comment explaining why | No carpet suppression |
| 5 | `cargo check` and `cargo clippy` run with `--profile uat` | dev profile uses cranelift which can't catch_unwind and has unrelated SIGABRT issues |
| 6 | Every crate in the workspace is covered — no crate is exempt | Consistency; a warning in one crate is still a warning |

# Requirements

## R1 — Baseline measurement ✅

- **cargo check**: 4 actionable Rust warnings (3 unused imports, 1 invalid feature
  `clap` in required-features) + C++ build warnings from vendored llama.cpp (out of scope)
- **cargo clippy**: 129 warnings across ~30 lint types.
  Top offenders: missing backticks in docs (51), needless pass-by-value (12),
  redundant closures (7), single-pattern match (5), wildcard single-variant (4).
- **Compilation blocker**: `foundation_packager --features cli` has 3 errors
  (`ProjectTemplates: EmbeddableDirectory` not satisfied).

Measured 2026-07-23 against `60-agentic-reliability-v3` HEAD.

## R2 — Fix all `cast_possible_wrap` and `cast_possible_truncation`

- Every site is replaced with `.try_into().expect("reason")`, an explicit
  `as` with a safety comment, or a `From`/`Into` impl.
- No new `#[allow(clippy::cast_possible_wrap)]` or `#[allow(clippy::cast_possible_truncation)]`.

## R3 — Fix all `unused_mut`, `unused_variables`, `unused_imports`, `dead_code`

- Remove them. No suppression.

## R4 — Fix or document-suppress all remaining lints outside the allowlist

- Fix wherever possible.
- If a lint genuinely cannot be fixed (e.g. `clippy::module_name_repetitions`
  in a crate that intentionally re-exports), add a single-site `#[allow]` with
  a `// WHY: …` comment.

## R5 — Clean up build-script warnings

- `cargo:warning=` diagnostics that are informational become `cargo:rerun-if-`
  or are silenced.
- Genuine warnings to the developer stay.

## R6 — Remove all crate-level blanket allows beyond the four

- Any crate whose `lib.rs` starts with `#![allow(clippy::…)]` beyond the
  permitted four must have those allows either removed (and the underlying
  warnings fixed) or justified individually.

## R7 — Zero-warning verification

- `cargo check --profile uat` produces 0 Rust warnings.
- `cargo clippy --profile uat --all-features` produces only the four allowlisted
  lint names (and zero other warnings).
- C++ build warnings from `infrastructure_llama_bindings` (jinja runtime.h,
  lexer.h) are out of scope — they come from vendored upstream C++ and are
  already suppressed in the Cargo.toml `[lints]` or build.rs.

## R8 — No feature-gated workarounds

- No new features, no new `#[cfg(…)]` gating. This is pure code cleanup.
- No behavioural change. The binaries, libraries, and tests must behave
  identically.

# Out of scope

- C++ build warnings from vendored llama.cpp (jinja/lexer headers).
- Adding new tests or changing test behaviour.
- Refactoring architecture or public API.
- Changing default features or feature resolution.
- Fixing `#[allow(clippy::pedantic)]` — pedantic is a workspace-level warn,
  and the blanket allow at crate roots covers intentional pedantic-triggering
  patterns. Individual pedantic fixes are fine but not required.
