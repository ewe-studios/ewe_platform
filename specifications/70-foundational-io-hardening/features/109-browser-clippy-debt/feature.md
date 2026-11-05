---
feature: "`foundation_browser` is clean under `clippy -D warnings`"
status: "pending"
lane: "after 84"
model: "haiku"
branch: "spec70/f109-browser-clippy-debt"
crates: ["foundation_browser"]
catalogue_rows: []  # from docs/bug-backlog.md §33; not a §8 catalogue row
---

# Feature 109: `foundation_browser` clippy debt

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (measured 2026-11-05, feature 82)

The command

```bash
cargo clippy --profile uat -p foundation_browser --all-targets --no-deps -- -D warnings
```

reports 56 errors. Feature 82 cleaned `src/test/stream.rs`; the errors are in files it did
not touch. Counts per file: `bidi/backend.rs` 11, `cdp/launch.rs` 6, `browser.rs` 6,
`bidi/mod.rs` 6, `lib.rs` 4, `jsonrpc/engine.rs` 4, `geometry.rs` 4, `test/server.rs` 3,
`supervisor.rs` 3, `backend.rs` 3, `locator.rs` 2, `jsonrpc/mod.rs` 2, `test/mod.rs` 1,
`test/harness.rs` 1.

Kinds: missing doc backticks 36, `cloned` where `copied` applies 4, `f64`-to-`i64` casts 4,
needless pass-by-value 3, identical match arms 2, and one each of a single-pattern `match`,
a missing `# Panics` section, a missing `# Errors` section, doc list indentation, a clamp
pattern and `map(...).unwrap_or(...)` on a `Result`.

The project goal is zero warnings: a warning is a defect, not accepted debt (AGENTS rules 8
and 10). Until this crate is clean, its clippy job cannot deny warnings, so a new warning can
enter without notice.

## Design

Mechanical cleanup, one kind at a time. Do not change behaviour.

1. Add the missing backticks in doc comments (36).
2. Replace `clone()` with `copied()` where the item is `Copy` (4).
3. Replace the `f64`-to-`i64` casts with the checked conversion the crate already uses, or
   `#[expect]` with a measured reason when the value is bounded (4).
4. Fix the remaining lints as clippy names them. For a lint that needs a judgement (a
   missing `# Errors` or `# Panics` section), write the section in STE.
5. Check `ci/packages.toml`: if `foundation_browser` does not deny clippy warnings, set
   `clippy-deny-warnings = true` in the same change, so the debt cannot return.
6. Run the tests after every group: clippy fixes must not change behaviour.

## Tests

- `cargo clippy --profile uat -p foundation_browser --all-targets --no-deps -- -D warnings`
  exits 0 with no warning.
- `cargo test --profile uat -p foundation_browser` passes as before.
- No `#[allow(...)]` is added. An `#[expect(...)]` carries a comment with the measured
  reason.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_browser --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_browser
python3 scripts/ci/ci.py run foundation_browser --profile uat   # in the Linux CI image
```

## Done when

- [ ] The clippy command above exits 0 with no warning.
- [ ] `foundation_browser` denies clippy warnings in `ci/packages.toml`.
- [ ] The backlog §33 entry is deleted.
- [ ] CI green.
