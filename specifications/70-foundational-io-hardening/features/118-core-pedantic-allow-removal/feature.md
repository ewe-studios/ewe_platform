---
feature: "`foundation_core` removes its crate-level clippy allows"
status: "pending"
lane: "after 115"
model: "sonnet"
branch: "spec70/f118-core-pedantic-allow"
crates: ["foundation_core"]
catalogue_rows: []  # from docs/bug-backlog.md §4; not a §8 catalogue row
---

# Feature 118: `foundation_core` removes its crate-level clippy allows

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (measured 2026-11-06, feature 116)

`backends/foundation_core/src/lib.rs` starts with four crate-level allows:

```rust
#![allow(clippy::pedantic)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::type_complexity)]
```

The workspace sets `pedantic = { level = "warn" }` (`Cargo.toml:110`). A crate-level
allow outranks the command line, so these four lines hide the pedantic group for the
whole crate.

Feature 116 made both clippy runs green and turned on `clippy-deny-warnings` for the
package. That zero count is measured with these allows in place. The engine is therefore
the one crate whose pedantic debt is not visible to CI, while every other crate holds
zero. The four lines come from commit `f6916c465` (2026-03-26).

`clippy::too_many_lines`, `clippy::type_complexity` and `clippy::pedantic` overlap: the
first two belong to the pedantic group. `clippy::cognitive_complexity` is a `nursery`
lint, which the workspace does not enable, so that line is a no-op today.

## Design

### 1. Measure before fixing

Delete the four lines in a working copy and run:

```bash
cargo clippy --profile uat -p foundation_core --all-targets --no-deps 2>&1 \
  | grep -oE 'clippy::[a-z_]+' | sort | uniq -c | sort -rn
```

Record the total and the per-lint counts in this file. If the total is above about 250,
split the work by module group (executors, extensions, io, state and tasks) and record a
table of counts per group first.

### 2. Fix at the cause, in slices

- Fix one lint kind at a time across the crate, so the diff is reviewable and the
  remaining count is easy to read.
- Where a fix would distort the engine (a long state machine in one function, a complex
  type that is the actual domain type), keep one `#[expect(clippy::..., reason = "...")]`
  on that item. The reason names the measured cause.
- No new crate-level or module-level `allow`. No `#[allow]` at all: the goal is a visible
  count.
- Every lint that stays as an `#[expect]` gets a backlog entry if a local change could
  remove it later.

### 3. Keep the gates green

After each slice, both clippy commands must exit 0 with no warning:

```bash
cargo clippy --profile uat -p foundation_core --all-targets --no-deps -- -D warnings
cargo clippy --profile uat -p foundation_core --all-targets --features multi --no-deps -- -D warnings
```

The package already denies warnings in CI (`ci/packages.toml`), so a regression fails the
run.

## Tests

No behaviour change. The existing suites must pass in both trees:

```bash
cargo test --profile uat -p foundation_core
cargo test --profile uat -p foundation_core --features multi
```

If a lint fix changes a signature (for example a `&Ctx` parameter), add no test: the
existing callers prove it. If a fix changes behaviour, that is a bug in the fix, not a
lint change: revert it and record it.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_core --all-targets --no-deps -- -D warnings
cargo clippy --profile uat -p foundation_core --all-targets --features multi --no-deps -- -D warnings
cargo test --profile uat -p foundation_core
cargo test --profile uat -p foundation_core --features multi
grep -n 'allow(clippy' backends/foundation_core/src/lib.rs   # no output
```

## Done when

- [ ] The four crate-level allows are gone from `src/lib.rs`.
- [ ] Both clippy commands exit 0 with no warning.
- [ ] The per-lint counts are recorded in this file.
- [ ] Each surviving `#[expect]` has a reason and, where a later local fix can remove it,
      a backlog entry.
- [ ] The tests pass in both trees.
- [ ] The backlog §4 entry for the crate-level allow is deleted.
- [ ] CI green.

## Result

Not started.
