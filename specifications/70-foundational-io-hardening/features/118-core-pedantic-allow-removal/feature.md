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

Done on 2026-11-06 on branch `spec70/priority-followups`.

### Measured before

The four crate-level allows were deleted first. These counts are from
`cargo clippy --profile uat -p foundation_core --all-targets --no-deps` with
the other pre-existing `#[allow]` attributes still in place.

| Group | Default features | `multi` |
|---|---:|---:|
| `valtron` | 300 | 382 |
| `synca` | 81 | 81 |
| `url` | 10 | 10 |
| `io` | 5 | 5 |
| `retries` | 3 | 3 |
| crate root | 2 | 2 |
| **Total** | **401** | **483** |

Per-lint counts, default features:

| Lint | Count |
|---|---:|
| `doc_markdown` | 103 |
| `missing_panics_doc` | 67 |
| `missing_errors_doc` | 50 |
| `type_complexity` | 41 |
| `must_use_candidate` | 40 |
| `match_same_arms` | 25 |
| `redundant_closure_for_method_calls` | 19 |
| `manual_let_else` | 11 |
| `single_match_else` | 10 |
| `needless_pass_by_value` | 8 |
| `too_many_lines` | 4 |
| `missing_fields_in_debug` | 4 |
| `if_not_else` | 4 |
| `uninlined_format_args` | 2 |
| `match_wildcard_for_single_variants` | 2 |
| `manual_assert` | 2 |
| `ignored_unit_patterns` | 2 |
| `unnested_or_patterns` | 1 |
| `needless_continue` | 1 |
| `format_push_string` | 1 |
| `filter_map_next` | 1 |
| `cast_sign_loss` | 1 |
| `cast_possible_wrap` | 1 |
| `cast_possible_truncation` | 1 |

Per-lint counts, `multi`:

| Lint | Count |
|---|---:|
| `doc_markdown` | 133 |
| `missing_panics_doc` | 74 |
| `missing_errors_doc` | 58 |
| `must_use_candidate` | 45 |
| `type_complexity` | 40 |
| `redundant_closure_for_method_calls` | 38 |
| `match_same_arms` | 26 |
| `single_match_else` | 11 |
| `needless_pass_by_value` | 11 |
| `manual_let_else` | 11 |
| `too_many_lines` | 5 |
| `missing_fields_in_debug` | 5 |
| `match_wildcard_for_single_variants` | 4 |
| `if_not_else` | 4 |
| `used_underscore_binding` | 2 |
| `uninlined_format_args` | 2 |
| `return_self_not_must_use` | 2 |
| `manual_assert` | 2 |
| `ignored_unit_patterns` | 2 |
| `unnested_or_patterns` | 1 |
| `needless_continue` | 1 |
| `items_after_statements` | 1 |
| `format_push_string` | 1 |
| `filter_map_next` | 1 |
| `cast_sign_loss` | 1 |
| `cast_possible_wrap` | 1 |
| `cast_possible_truncation` | 1 |

### Measured after

Both clippy trees exit 0 with no warning:

```text
cargo clippy --profile uat -p foundation_core --all-targets --no-deps -- -D warnings
cargo clippy --profile uat -p foundation_core --all-targets --features multi --no-deps -- -D warnings
```

The work also removed the pre-existing module-level `#![allow(clippy::...)]`
attributes and the item-level allows for pedantic lints. This exposed 19 more
warnings: 6 from the module-level allows and 13 from the item-level allows.
All of them are fixed.

### Work done

- Fixed one lint kind at a time across the crate.
- Added `# Panics` and `# Errors` sections to the public API, in STE.
- Added type aliases for the split and broadcast return pairs, the boxed
  resolver builders, and the boxed predicate and transform parameters.
- Merged identical match arms and nested the resulting or-patterns.
- Converted manual `if let ... else` blocks to let-else.
- Replaced the `filter_map(..).next()` call with `find_map`.
- Replaced float percentage checks in `io/ioutils` with exact integer
  cross-products.
- Split `ThreadRegistry::spawn_worker` into a helper that reports the worker
  stop, and kept one `#[expect]` on the local executor state machine.

### Surviving `#[expect]`

| Item | Lint | Reason |
|---|---|---|
| `ExecutorState::do_work` (`valtron/executors/local.rs`) | `too_many_lines` | One ordered pass over the task state machine. A split moves the state transitions away from the state they read. |

### Surviving `#[allow]`

These are item-level, not crate-level or module-level.

| Item | Lint | Reason |
|---|---|---|
| `PoolStatsSnapshot::hit_ratio` | `cast_precision_loss` | A hit ratio above 2^53 buffers cannot occur. |
| `IntoString::into_string` | `wrong_self_convention` (style, not pedantic) | The trait name is public API used by other crates. |
| `IntoStr::into_str` | `wrong_self_convention` (style, not pedantic) | The trait name is public API used by other crates. |

The other item-level allows stay and cover non-pedantic lints only:
`from_over_into`, `arc_with_non_send_sync`, `len_without_is_empty`,
`new_ret_no_self`, `result_unit_err`, `self_named_constructors`,
`too_many_arguments` and `missing_const_for_thread_local`.

### Tests

```text
cargo test --profile uat -p foundation_core                 # 870 passed, exit 0
cargo test --profile uat -p foundation_core --features multi # 916 passed, exit 0
```

The tests count each test binary, the doctests and the `multi` variants.
