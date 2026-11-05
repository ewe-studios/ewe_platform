---
feature: "`foundation_connectrpc` arrow-codec tests and example compile again"
status: "pending (CI proof)"
lane: "after 84"
model: "haiku"
branch: "spec70/f112-connectrpc-arrow-codec-result"
crates: ["foundation_connectrpc"]
catalogue_rows: []  # from docs/bug-backlog.md §1; not a §8 catalogue row
---

# Feature 112: `foundation_connectrpc` arrow targets compile

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (measured 2026-11-05, feature 84)

`cargo check --profile uat -p foundation_connectrpc --all-targets` fails with `E0308` at
`tests/arrow_codec_tests.rs:79` and `:86`, and at `examples/codecs/main.rs:123` and `:130`.
The `ToArrow::to_arrow` and `to_arrow_batch` implementations return
`IpcResult<RecordBatch>`, which feature 67 changed to `ErrorTrace<ArrowIpcError>`, but they
return `RecordBatch::try_new(..)` directly, whose error is `ArrowError`.

The error predates feature 84: the same failure exists at base `a94fe307b`. Because the test
target does not compile, `cargo test -p foundation_connectrpc` and
`cargo clippy -p foundation_connectrpc --all-targets` cannot run. That hides the crate's
tests and clippy result from CI and from every agent.

## Design

1. In both `ToArrow` implementations (`src/` of `foundation_connectrpc`), map the arrow
   error into the crate's error standard: `.map_err(Into::into)` or
   `.map_err(ErrorTrace::new)`.
2. Do not change the trait signature or the public API. No version bump is needed for a
   private implementation detail.
3. Check the rest of the crate for the same pattern: any `?` on an `ArrowError` in a
   function that returns `IpcResult<_>`.

## Tests

- `cargo check --profile uat -p foundation_connectrpc --all-targets` exits 0.
- `cargo test --profile uat -p foundation_connectrpc` runs its arrow-codec tests and they
  pass. If a test asserts an error value, update it to the `ErrorTrace` form and check the
  current context, not the display string.

## Verify

```bash
cargo fmt --all --check
cargo check --profile uat -p foundation_connectrpc --all-targets
cargo clippy --profile uat -p foundation_connectrpc --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_connectrpc
python3 scripts/ci/ci.py run foundation_connectrpc --profile uat   # in the Linux CI image
```

## Done when

- [x] The all-targets check and clippy pass.
- [ ] The crate's tests run in CI (pending the run on the pushed commit).
- [x] The backlog §1 entry is deleted.
- [ ] CI green (pending the run on the pushed commit).

## Result (2026-11-05)

The two `ToArrow` implementations in `tests/arrow_codec_tests.rs` and
`examples/codecs/main.rs` now map the result with `.map_err(arrow_err)`, which is
`foundation_arrow`'s public conversion to `ErrorTrace<ArrowIpcError>`. No trait or public API
changed, so no version bump. A search of the crate found no other `?` on an `ArrowError`
inside an `IpcResult` function.

Verified locally:

- `cargo check --profile uat -p foundation_connectrpc --all-targets` exits 0.
- `cargo clippy --profile uat -p foundation_connectrpc --all-targets --no-deps -- -D warnings`
  exits 0.
- `cargo test --profile uat -p foundation_connectrpc` exits 0; 15 test binaries pass,
  including `arrow_codec_tests` (2 tests).

Backlog §1 is deleted.
