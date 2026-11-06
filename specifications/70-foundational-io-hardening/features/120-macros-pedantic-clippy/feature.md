---
feature: "`foundation_macros` reaches zero clippy warnings and enters the CI lint set"
status: "done"
lane: "after 118"
model: "haiku"
branch: "spec70/f120-macros-clippy"
crates: ["foundation_macros"]
catalogue_rows: []  # from docs/bug-backlog.md §30; not a §8 catalogue row
---

# Feature 120: `foundation_macros` reaches zero clippy warnings

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (measured 2026-11-06, feature 115)

`cargo clippy --profile uat -p foundation_macros --all-targets --no-deps -- -D warnings`
fails with 62 pedantic diagnostics. The workspace sets `clippy::pedantic = "warn"`
(`Cargo.toml`), and the crate inherits it.

The crate is not listed in `ci/packages.toml`, so CI never lints it. That is why the debt
survived: no run reports it.

Measured groups: 20 missing backticks in doc comments (`doc_markdown`), 6
single-pattern `match` (`single_match_else`), 4 wildcard matches
(`wildcard_enum_match_arm`), 4 by-value arguments (`needless_pass_by_value`), 4 similar
binding names (`similar_names`), 3 `format!` inline arguments
(`uninlined_format_args`), 6 `too_many_lines`, and smaller groups. Four are in
`valtron_entry.rs` at lines 245, 246, 285 and 308 and predate feature 115.

The manifest also reports `chrono` as an unused dependency.

## Design

1. Fix the lints at their cause. Most are mechanical:
   - `doc_markdown`: put backticks around identifiers in doc comments.
   - `single_match_else` / `wildcard_enum_match_arm`: add the missing arm, or rewrite as
     `if let` / `match` with an explicit arm.
   - `needless_pass_by_value`: take a reference where the value is not consumed.
   - `similar_names`: rename one binding.
   - `uninlined_format_args`: inline the argument.
   - `too_many_lines`: split the function. For a single linear table or expansion, keep
     one `#[expect(clippy::too_many_lines, reason = "...")]` on that function.
2. Remove `chrono` if no target or feature uses it. Check every `use chrono` in the crate
   and its tests before removing it.
3. Add `[packages.foundation_macros]` to `ci/packages.toml` with
   `clippy-deny-warnings = true`, so the count stays at zero. Add no `runs` entry unless
   the crate has a second feature tree to test.
4. No crate-level or module-level `allow`. Every surviving `#[expect]` names its reason.

## Tests

No behaviour change, so the existing tests are the proof:

```bash
cargo test --profile uat -p foundation_macros
```

The 40 doc tests stay ignored by design.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_macros --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_macros
python3 scripts/ci/ci.py run foundation_macros --profile uat
```

## Done when

- [x] The clippy command exits 0 with no warning.
- [x] `chrono` is gone or is proven used.
- [x] `ci/packages.toml` has `[packages.foundation_macros]` with
      `clippy-deny-warnings = true`.
- [x] The tests pass.
- [x] The item in GitHub issue #200 is ticked.
- [ ] CI green.

## Result

Done on branch `spec70/priority-followups` (2026-11-06), commit `1ba90e017`.

- The count went from 62 to 0 in the lib target. Once the lib compiled, `--all-targets`
  exposed 7 more (3 `doc_markdown` in an inline test module, 4 `assert_is_empty` in
  `tests/`), so 69 diagnostics were addressed.
- Six `too_many_lines` sites keep one `#[expect(clippy::too_many_lines, reason = "...")]`
  each, because each is one linear macro expansion.
- `chrono` had no user in code, tests, targets or features. It is removed from
  `backends/foundation_macros/Cargo.toml`, and `Cargo.lock` loses that dependency.
- `ci/packages.toml` gains `[packages.foundation_macros]` with
  `clippy-deny-warnings = true`.
- Verify: `cargo clippy --profile uat -p foundation_macros --all-targets --no-deps --
  -D warnings` exit 0; `cargo test --profile uat -p foundation_macros` exit 0 (98 tests
  pass, 40 doc tests ignored by design); `python3 scripts/ci/ci.py run foundation_macros
  --profile uat` exit 0.
- `cargo fmt --all --check` failed only in `backends/foundation_core/src/valtron/executors/local.rs`,
  which belongs to feature 118 and was in flight.
- Out-of-scope finding: 105 `unused_must_use` warnings from ignored `.close();` results.
  Filed as issue #211.
- The five inline `#[cfg(test)] mod tests` modules stay: a proc-macro crate cannot expose
  its private helpers to `tests/`. Issue #199 tracks the move.
