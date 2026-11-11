---
feature: "A stream that asks for its read wake source too early can ask again"
status: "done"
lane: "priority follow-up (found under feature 137)"
model: "haiku"
branch: "spec70/priority-followups"
crates: ["foundation_netio"]
catalogue_rows: ["2"]
---

# Feature 141: a negative read wake source is not cached

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **wake source**: the closure a body reader calls before it parks, so the transport
  registers the task's waker on the socket (`ReadWakeSource`).
- **negative answer**: the `None` that `read_wake_source` returns when the stream cannot
  register a waker now.

## Problem (found under feature 137 on 2026-11-06, backlog §2)

Feature 137 made `RawStream::read_wake_source` build its source once and cache it in a
`OnceLock<Option<ReadWakeSource>>` (`backends/foundation_netio/src/native/raw_stream.rs`).
The cache stores the negative answer as well.

The function returns `None` for three reasons: the stream does not support non-blocking
input, the socket is still blocking, or the fd is not valid. The first and third reasons
are permanent, but the second one is not. A caller that asks while the socket is blocking
caches `None`; after `set_nonblocking(true)`, every later call still returns `None`, so the
reader never parks on readiness.

Every current caller sets non-blocking first (`http_client_impl.rs`, `proxy/forward.rs`,
`proxy/aio.rs` and the server accept paths), so there is no live defect. The API shape
invites the mistake.

## Design

1. Cache only a successful source. When the answer is `None`, leave the cell empty and
   return `None`. A later call repeats the checks.
2. Keep the one-time build for the success path: two callers still share one `Arc` and one
   reactor token (feature 137).
3. State the behaviour in the `read_wake_source` doc comment: a negative answer is not
   cached, because the socket mode can change.

## Tests

In `backends/foundation_netio/tests/` (register in `tests/mod.rs`):

1. `a_negative_answer_is_not_cached`: build a loopback `RawStream`, ask before setting
   non-blocking, assert `None`; set non-blocking; ask again and assert `Some`. Fails on the
   code after feature 137, where the second answer is `None`.
2. Keep `read_wake_source_is_stable_across_calls` passing: a successful source is still
   built once.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_netio --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_netio --test foundation_netio_tests read_wake_source
cargo test --profile uat -p foundation_netio
scripts/ci/linux.sh foundation_netio
```

## Done when

- [x] A negative answer is not cached; the success path still builds once.
- [x] The new test exists and fails on the current code.
- [x] The backlog §2 entry is deleted.
- [ ] CI green.

## Result

2026-11-06. Implemented on `spec70/priority-followups`.

Files changed:

- `backends/foundation_netio/src/native/raw_stream.rs`: `RawStream::read_wake`
  is now `OnceLock<ReadWakeSource>`. `read_wake_source` returns the cached
  source when it exists, else builds it and stores it only on success. The doc
  comment says why a negative answer is not cached. `RawStream` is moved
  between threads but never shared as `&`, so no two calls build a source at
  the same time.
- `backends/foundation_netio/tests/read_wake_source_tests.rs`: added
  `a_negative_answer_is_not_cached`.
- `docs/bug-backlog.md`: deleted the §2 entry for the cached negative answer.

Evidence:

- The new test fails on the code from feature 137 (`39f4eedb3`): 0 passed,
  1 failed. The first `read_wake_source` on a blocking socket returns `None`;
  after `set_nonblocking(true)` the second call still returns `None` on the old
  code.
- After the change all three wake-source tests pass (3 passed).
- `cargo fmt --all --check`: exit 0.
- `cargo clippy --profile uat -p foundation_netio --all-targets --no-deps -- -D warnings`:
  exit 0.

CI proof is pending the push.
