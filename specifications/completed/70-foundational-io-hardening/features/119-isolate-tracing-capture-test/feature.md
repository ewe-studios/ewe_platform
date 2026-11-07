---
feature: "Move the tracing capture test into its own test binary"
status: "done"
lane: "priority (blocks CI green on the branch)"
model: "haiku"
branch: "spec70/f119-tracing-capture-isolation"
crates: ["foundation_core"]
catalogue_rows: []  # from docs/bug-backlog.md §4; not a §8 catalogue row
---

# Feature 119: move the tracing capture test into its own binary

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (measured 2026-11-05)

GitHub run 42072422806 (commit `680b96e45`, a documentation-only change) failed the
`foundation_core` job in its test step. The failing test is
`io::readers_tests::batch_reader_would_block_is_not_an_error_log` at
`backends/foundation_core/tests/io/readers_tests.rs:878`. The panic message is "a real
read error must emit an ERROR record (proves capture works)" (line 924). The same test
passed in the `multi` tree of the same job, and the surrounding tests passed.

The cause is the process-wide callsite interest cache in `tracing`. The test installs a
scoped subscriber with `tracing::subscriber::with_default` and then calls
`tracing::callsite::rebuild_interest_cache`. Other test threads in the same binary reach
the same callsites with no subscriber, and the cache returns to `never` after the
rebuild. The test then records nothing. The test already documents this race; the rebuild
narrows the window but does not close it.

`tracing::error!` at `src/io/readers/mod.rs:226` is the callsite the second assertion
needs. The `WouldBlock` path uses no `error!` callsite, which is why the first assertion
holds.

## Design

Move the test into its own integration target: `backends/foundation_core/tests/tracing_error_capture.rs`.

1. Copy the test, the `ErrorCapture` layer, and the two local readers (`AlwaysWouldBlock`,
   `RealError`) into the new file. Add the imports it needs: `foundation_core::io::readers::*`,
   `std::io::{self, Read}`, `std::sync::{Arc, Mutex}`, `tracing::Level`,
   `tracing_subscriber::Layer`, `tracing_subscriber::layer::SubscriberExt` and
   `tracing_subscriber::util::SubscriberInitExt` (only what the file uses).
2. Keep the `rebuild_interest_cache` calls: they are correct and cheap. Add a comment that
   says the binary holds one test, so no other thread can change the cache.
3. Delete the test, the layer and the two reader structs from `readers_tests.rs`. Delete
   the imports that become unused there.
4. Put `#![cfg(not(target_family = "wasm"))]` at the top of the new file, as
   `tests/io/mod.rs` guards `readers_tests` today.
5. Do not change `BatchReader` or any source file.

This keeps the proof (a real read error emits an ERROR record) and removes the race.

## Tests

The new binary has one test. The existing readers tests lose one test each and keep their
count minus one.

```bash
cargo test --profile uat -p foundation_core
cargo test --profile uat -p foundation_core --features multi
cargo test --profile uat -p foundation_core --test tracing_error_capture
```

Run the new binary 20 times in a row to show it is stable:

```bash
for i in $(seq 20); do cargo test --profile uat -p foundation_core --test tracing_error_capture -- --exact || break; done
```

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_core --all-targets --no-deps -- -D warnings
cargo clippy --profile uat -p foundation_core --all-targets --features multi --no-deps -- -D warnings
cargo test --profile uat -p foundation_core
cargo test --profile uat -p foundation_core --features multi
```

## Done when

- [x] The test runs in `tests/tracing_error_capture.rs` and no longer in `readers_tests.rs`.
- [x] The new binary passes 20 consecutive runs.
- [x] Both clippy runs exit 0 with no warning.
- [x] Both test trees pass.
- [x] The backlog §4 entry is deleted.
- [ ] CI green.

## Result

2026-11-06. Done except the CI proof.

- `backends/foundation_core/tests/tracing_error_capture.rs` is a new integration
  target with one test. It holds `ErrorCapture`, the test and the two local readers.
  The file starts with `#![cfg(not(target_family = "wasm"))]` and a comment that
  states why the binary holds one test.
- `tests/io/readers_tests.rs` lost 89 lines: the log-level regression block, the
  four unused imports, `ErrorCapture` and the test. No other test changed.
- The default tree runs 743 tests in the `mod` target (was 744); the `multi` tree
  runs 784 (was 785). The new target runs 1 test in both trees.
- `cargo fmt --all --check`, both `-D warnings` clippy runs and both test runs
  pass with no warning. The new binary passed 20 consecutive runs on macOS.
- The backlog §4 entry is deleted. No source file changed, so no behaviour change.

The race remains possible for a future test that installs a scoped subscriber
inside the large `mod` binary. The pattern is: put such a test in its own target.
The Linux CI run is the cross-platform proof.

**CI proof, 2026-11-06.** GitHub audit run
[42099300473](https://github.com/ewe-studios/ewe_platform/actions/runs/42099300473)
attempt 2 on commit `9b1138a63` finished `success` (117 success, 1 neutral, 3
skipped); the `foundation_core` job runs the new single-test target on Linux.
