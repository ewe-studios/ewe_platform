---
feature: "`foundation_browser` is clean under `clippy -D warnings`"
status: "pending (CI proof)"
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

- [x] The clippy command above exits 0 with no warning.
- [x] `foundation_browser` denies clippy warnings in `ci/packages.toml`.
- [x] The backlog §33 entry is deleted.
- [ ] CI green.

## Result (2026-11-05)

The clippy command exits 0 and prints no warning.

| Run | Before | After |
|---|---|---|
| `cargo clippy --profile uat -p foundation_browser --all-targets --no-deps -- -D warnings` | 55 errors | 0 |
| `cargo test --profile uat -p foundation_browser` | 1 passed | 1 passed, 0 failed |

The feature file measured 56 errors. The current tree has 55: the counts per
file and lint kind are the same, so one diagnostic is no longer reported by the
current clippy.

Lint kinds fixed: `doc_markdown` (36), `cloned_instead_of_copied` (4),
`cast_possible_truncation` (4), `needless_pass_by_value` (3),
`match_same_arms` (2), and one each of `single_match_else`,
`missing_errors_doc`, `missing_panics_doc`, `doc_lazy_continuation`,
`manual_clamp` and `map_unwrap_or`.

- The 36 `doc_markdown` fixes add backticks in doc comments (`WebDriver`,
  `BiDi`, `DevTools`, `BrowserDriver`, `TestServer`, `foundation_http/SSE`).
- The lazy-continuation warning was an accidental `+ ` list marker in `lib.rs`;
  the text now reads "plus the CDP client".
- The four casts become one `wire_coord` helper with one `#[expect]`.
- Two `needless_pass_by_value` warnings are on public functions
  (`BrowserDriver::launch`, `Harness::setup`). A reference would break every
  caller, so each gets an `#[expect]` with its reason. The third
  (`BiDiBackend::perform`) is private and now takes `&Value`.

`ci/packages.toml` adds `[packages.foundation_browser]` with only
`clippy-deny-warnings = true`; the defaults merge in.

Three `#[expect]` attributes across two lint kinds, each with a reason:

1. `clippy::cast_possible_truncation` on `wire_coord` in
   `src/bidi/backend.rs`. The value is a viewport CSS pixel count, far inside
   the `i64` range. `as i64` saturates a malformed value instead of panicking.
2. `clippy::needless_pass_by_value` on `BrowserDriver::launch` and
   `Harness::setup`. Both are public API that takes its config by value; a
   reference would break every caller.

The tests run 1 test (`browser_sse_transport_delivers_the_whole_frame_to_a_slow_client`)
and it passes. `tests/smoke.rs` is gated on the `browser-tests` feature and
needs a real Chromium; the default run compiles it to 0 tests. The CI package
uses the default run, so those browser tests are skipped there too.
