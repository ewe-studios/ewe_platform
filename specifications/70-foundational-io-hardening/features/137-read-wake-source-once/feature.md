---
feature: "A stream builds its read wake source once, so two sources cannot replace each other"
status: "pending"
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
cargo test --profile uat -p foundation_netio --test mod read_wake_source
cargo test --profile uat -p foundation_netio
cargo test --profile uat -p foundation_proxy
```

## Done when

- [ ] `read_wake_source` returns the same `Arc` on every call for one stream.
- [ ] The two tests exist; the first fails on the old code.
- [ ] The backlog §2 duplicate-token entry is deleted.
- [ ] CI green.

## Result

2026-11-06. Plan only. Not started. Written after feature 129 found the second registration
hazard by reading the source.
