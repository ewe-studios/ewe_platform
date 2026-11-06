---
feature: "`foundation_macros` reaches zero clippy warnings and enters the CI lint set"
status: "pending"
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

- [ ] The clippy command exits 0 with no warning.
- [ ] `chrono` is gone or is proven used.
- [ ] `ci/packages.toml` has `[packages.foundation_macros]` with
      `clippy-deny-warnings = true`.
- [ ] The tests pass.
- [ ] The backlog §30 entry is deleted.
- [ ] CI green.

## Result

Not started.
