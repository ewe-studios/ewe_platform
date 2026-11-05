---
feature: "`foundation_core` reaches zero clippy warnings and denies them in CI"
status: "pending"
lane: "priority (owner comment, PR #180, 2026-11-05)"
model: "haiku"
branch: "spec70/f116-core-clippy-debt"
crates: ["foundation_core", "foundation_nostd"]
catalogue_rows: []  # from docs/bug-backlog.md §4; not a §8 catalogue row
---

# Feature 116: zero clippy warnings in `foundation_core`

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (measured 2026-11-04 and 2026-11-05)

`foundation_core` is the engine every other crate builds on, and it is the last large
crate that does not hold its clippy output at zero. Feature 49 recorded this as E10 and
left the crate `partial`.

1. **The library fails `-D warnings`.** On `fix/io-41-50-lane-a` (2026-11-04),
   `cargo clippy -p foundation_core --no-deps --profile dev --lib -- -D warnings`
   reported 19 errors:
   - `double_must_use` at `src/valtron/executors/background.rs:283,299` and
     `src/valtron/task.rs:214`.
   - `arc_with_non_send_sync` at `src/valtron/executors/local.rs:374,375,391,392` and
     `src/valtron/future_task.rs:616`.
   - `question_mark` at `src/valtron/future_task.rs:936` and
     `src/valtron/task_iters.rs:127,449`.
   - `while_let_on_iterator` at `src/valtron/extensions/streams/non_sendable.rs:1296,1306`.
   - `new_ret_no_self` at `src/valtron/pipe.rs:114`.
   - `result_unit_err` and `len_without_is_empty` at `src/valtron/pipe_mapped.rs:43,81`.
   - Three more at `src/valtron/io/ioutils/mod.rs:950,1542,1705`.
2. **`--all-targets` adds 327 warnings.** Test targets and examples carry the rest. The
   examples report `clippy::drop_non_drop` on the guard the `#[valtron]` attribute
   expands to, and `clippy::needless_pass_by_value` on `Ctx` parameters. Seen
   2026-11-05 (feature 114).
3. **The bare `--no-default-features` build prints its `compile_error!` and then 24 more
   errors.** `foundation_core` refuses the configuration with a clear message (feature 78
   E3), but `pub mod valtron` and its executors still compile and emit unresolved-import
   errors for `Ctx`. Only the named error is actionable; the other 24 are noise.
4. **The package does not deny warnings.** `[packages.foundation_core]` in
   `ci/packages.toml` has no `clippy-deny-warnings`, so every run reports the debt and
   passes. `foundation_nostd` already sets `clippy-deny-warnings = true` and is clean.

The dependency direction makes this the root of the warning work: feature 47 and feature
49 fixed the crates above `foundation_core`, and the engine stayed the last noisy crate.

## Design

### 1. Fix each lint at its cause

- `double_must_use`: the macro or the function already returns a `#[must_use]` type;
  remove the outer attribute, or drop the inner one. Keep one `#[must_use]` per call.
- `arc_with_non_send_sync`: the local executor holds `Rc`/`Cell` state in an `Arc` that
  never crosses a thread. Keep the `Arc` (the queue type is shared with the multi
  executor) and put one `#[expect(clippy::arc_with_non_send_sync, reason = "...")]` with
  a measured reason, as `single/mod.rs:156` already does. Do not change the queue type.
- `question_mark`: replace `match … { Ok(v) => v, Err(e) => return Err(e.into()) }` with
  `?`.
- `while_let_on_iterator`: move the loop to `for` where the iterator is owned, or keep
  `while let Some(..) = it.next()` behind an `#[expect]` when the loop must keep the
  iterator for later.
- `new_ret_no_self`: `pipe.rs:114` returns a different type. Rename the constructor, or
  return `Self`.
- `result_unit_err` / `len_without_is_empty`: return a meaningful error type, or add
  `is_empty`.
- `drop_non_drop` in the examples: the guard type has a `Drop` impl under `multi`
  (`multi::PoolGuard`, `executors/multi/mod.rs:707`) and none under the single-threaded
  build (`single::PoolGuard`). The macro emits `drop(__valtron_guard)`, which is a no-op
  for the second type. Fix the macro: wrap the body in a block and let the guard drop at
  the block end. The multi build still tears down at the same point; the single build
  stops emitting a no-op `drop`.
- `needless_pass_by_value` on `Ctx` in the examples: take `Ctx` by reference where the
  example does not consume it. `Ctx` is cheap to clone, so a reference does not change
  behaviour.

### 2. Gate `pub mod valtron` on `std`

`lib.rs` gates the module with `#[cfg(feature = "std")]`; the `compile_error!` for the
bare build stays the only error. The valtron module needs `std` for its executors, so the
gate is true to the dependency.

### 3. Deny warnings in CI

`ci/packages.toml` gains `clippy-runs = ["", "multi"]` and `clippy-deny-warnings = true`
for `[packages.foundation_core]`. `runs` already covers `""` and `"multi"`; the clippy
list must cover the same two trees because the executors differ by feature. Keep
`foundation_nostd` as it is.

### 4. No new constant in the code path

Every fix is a lint fix. No bound, timeout or size becomes a constant (AGENTS rule 8).

## Tests

No behaviour changes, so the existing suites are the check. Add no test for a lint fix.
The two pieces below need care:

- The `#[valtron]` macro change reaches every example and every crate. Run the
  `foundation_core` tests in both feature trees, and one `#[valtron]` example, after the
  change.
- The `lib.rs` `std` gate hides `valtron` from the bare build. Check that the bare build
  prints only the `compile_error!`.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_core --all-targets --no-deps -- -D warnings
cargo clippy --profile uat -p foundation_core --all-targets --features multi --no-deps -- -D warnings
cargo clippy --profile uat -p foundation_nostd --all-targets --no-deps -- -D warnings
cargo check --profile uat -p foundation_core --no-default-features 2>&1 | head -20
cargo test --profile uat -p foundation_core
cargo test --profile uat -p foundation_core --features multi
```

Measured in the Linux CI image when the host is Apple Silicon:

```bash
python3 scripts/ci/ci.py run foundation_core --profile uat
```

## Done when

- [ ] Both `-D warnings` clippy commands for `foundation_core` print no warning.
- [ ] `foundation_nostd` stays at zero.
- [ ] The bare `--no-default-features` build prints only its `compile_error!`.
- [ ] `ci/packages.toml` sets `clippy-runs = ["", "multi"]` and
      `clippy-deny-warnings = true` for `foundation_core`.
- [ ] The `foundation_core` tests pass in both feature trees.
- [ ] The backlog §4 entries for the core lints are deleted; feature 49 moves to `done`
      with its E10 row updated.
- [ ] CI green.

## Result

Not started.
