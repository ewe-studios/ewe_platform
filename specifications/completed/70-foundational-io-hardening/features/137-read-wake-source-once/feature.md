---
feature: "A stream builds its read wake source once, so two sources cannot replace each other"
status: "done"
lane: "priority follow-up (found under feature 129)"
model: "sonnet"
branch: "spec70/priority-followups"
crates: ["foundation_netio"]
catalogue_rows: ["2"]
---

# Feature 137: one read wake source per stream

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **wake source**: the closure a body reader calls before it parks, so the transport
  registers the task's waker on the socket (`ReadWakeSource`).
- **token**: the reactor's identifier for one fd registration (`FdReadiness`).

## Problem (found under feature 129 on 2026-11-06, backlog §2)

`RawStream::read_wake_source`
(`backends/foundation_netio/src/native/raw_stream.rs:446`) builds a new `FdReadiness` on
every call. Each `FdReadiness` takes a fresh reactor token for the same fd, and the kqueue
and epoll registrations are per fd, not per token: a second registration replaces the
first one's event registration. If two sources exist at the same time, the first one's
registered waker can stop firing.

No current caller holds two sources for one stream, so no test hits it. The API shape
invites the mistake: `read_wake_source(&self)` looks idempotent and is not.

## Design

1. Make the source a per-stream value. `RawStream` gains a cache, for example:

   ```rust
   read_wake: OnceLock<Option<ReadWakeSource>>,
   ```

   `read_wake_source` builds the value on the first call and returns a clone of the same
   `Arc` on every later call. Two callers then share one `FdReadiness` and one token.
2. The cache is long-lived on the stream. A `RawStream` is not `Clone`; verify that and
   state it in the Result. If it is cloned, keep the cache in the shared inner value.
3. Public API is unchanged (`ReadWakeSource` is already an `Arc`). The crate version moves
   one patch step only if the change touches a public signature; say which in the Result.

## Tests

In `backends/foundation_netio/tests/` (register in the crate's `tests/mod.rs`):

1. `read_wake_source_is_stable_across_calls`: build a loopback `RawStream` (the existing
   test helpers show how), call `read_wake_source()` twice and assert `Arc::ptr_eq`. On
   the old code the two values are distinct — record the failure.
2. `a_waker_registered_through_the_first_source_still_fires`: register a counting waker
   through source A, create source B, send bytes, and assert A's waker fires within 2 s.
   This is the behavioural guard: on the old code B's registration replaces A's.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_netio --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_netio --test foundation_netio_tests read_wake_source
cargo test --profile uat -p foundation_netio
cargo test --profile uat -p foundation_proxy
```

## Done when

- [x] `read_wake_source` returns the same `Arc` on every call for one stream.
- [x] The two tests exist; the first fails on the old code.
- [x] The backlog §2 duplicate-token entry is deleted.
- [x] CI green.

## Result

2026-11-06. Done on macOS and in the Linux image.

### Files changed

- `backends/foundation_netio/src/native/raw_stream.rs`. `RawStream` is now a struct
  with a private `RawStreamInner` enum and a `OnceLock<Option<ReadWakeSource>>`
  cache. `read_wake_source` builds the source on the first call and returns a
  clone of the same `Arc` on every later call. The builder moved to the private
  `build_read_wake_source`. The cache field is declared before the transport
  field, so it drops first and the cached `FdReadiness` deregisters the fd while
  the socket is still open. The comment above `readiness.clear()` now says which
  part feature 136 moved: `register_waker` re-arms the selector edge, and the
  clear stays for the latched readiness flags and to disarm the old waker.
- `backends/foundation_netio/tests/read_wake_source_tests.rs` (new). The two
  tests below, with a loopback pair and a counting `Waker`.
- `backends/foundation_netio/tests/mod.rs`. Registers the new file in the
  consolidated `foundation_netio_tests` target.
- `docs/bug-backlog.md`. The §2 duplicate-token entry is deleted. The §2
  irrefutable-pattern entry keeps its meaning with the new private symbol name.
- This feature file.

`RawStream` is not `Clone`, and it has no `TryClone` impl; no code copies it. The
cache therefore needs no shared inner value. No public method signature changed,
so the crate version stays 0.0.6. `RawStream` itself changed from a public enum
with three variants to a public struct with a private inner enum. No workspace
consumer names or constructs the variants: all use `from_tcp`, `from_connection`,
`from_server_tls`, `from_client_tls` or `from_endpoint`.

### Tests

| Test | What it proves |
|---|---|
| `read_wake_source_is_stable_across_calls` | Two calls return the same `Arc` (`Arc::ptr_eq`). |
| `a_waker_registered_through_the_first_source_still_fires` | A waker registered through source A fires within 2 s after source B is created and bytes arrive. |

Old-code evidence, from the run before the fix:

```text
test read_wake_source_tests::read_wake_source_is_stable_across_calls ... FAILED
thread '...read_wake_source_is_stable_across_calls' panicked at ...:
read_wake_source built a second source: two sources for one fd take two reactor tokens, and the second registration replaces the first
test read_wake_source_tests::a_waker_registered_through_the_first_source_still_fires ... FAILED
thread '...a_waker_registered_through_the_first_source_still_fires' panicked at ...:
the waker registered through the first source did not fire within 2 s after a second source was created and bytes arrived
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 1218 filtered out; finished in 2.01s
```

The first test compares two distinct `Arc`s. The second times out because source
B's `FdReadiness` registration replaces source A's event registration on the fd.

### Command outcomes

| Command | Result |
|---|---|
| `cargo fmt --all --check` | exit 0, clean |
| `cargo clippy --profile uat -p foundation_netio --all-targets --no-deps -- -D warnings` | exit 0, no warnings |
| `cargo test --profile uat -p foundation_netio --test foundation_netio_tests read_wake_source` | 2 passed |
| `cargo test --profile uat -p foundation_netio` | exit 0; 5, 6, 1220, 9, 2 and 54 tests pass in 6 binaries |
| `cargo test --profile uat -p foundation_proxy` | exit 0; every test binary passes |
| `scripts/ci/linux.sh foundation_netio` | exit 0; fmt, build, clippy, the default and `quic` test runs, and the wasm32 check all pass |

In the Linux image the default run reports 1221, 9, 6, 5, 2 and 54 tests passing.
The `quic` run reports 1296, 59, 16, 9, 8, 2 and 54 tests passing. No test fails.

The brief named `--test mod`. That target does not exist in `foundation_netio`:
`tests/mod.rs` is the `foundation_netio_tests` target. The command above uses the
real target name.

### Note

The new file is auto-discovered as its own test target and is also registered in
`tests/mod.rs`, so the two tests run in both targets. Feature 136 kept its
top-level test file out of `tests/mod.rs` for this reason. This feature keeps the
registration because the brief asked for it.
