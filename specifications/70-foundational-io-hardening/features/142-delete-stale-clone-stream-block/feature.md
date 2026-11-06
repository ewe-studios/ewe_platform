---
feature: "Delete the stale commented clone_stream block in RawStream"
status: "done"
lane: "priority follow-up (found under feature 137)"
model: "haiku"
branch: "spec70/priority-followups"
crates: ["foundation_netio"]
catalogue_rows: ["2"]
---

# Feature 142: the commented clone_stream block is deleted

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (found under feature 137 on 2026-11-06, backlog §2)

`backends/foundation_netio/src/native/raw_stream.rs` keeps a commented-out
`clone_stream` method at lines 751 to 762. The block names `RawStream::AsPlain` and
`RawStream::AsTls`, which no longer exist: `RawStream` is a struct over the private
`RawStreamInner` enum since feature 137.

The text has no compile effect. It is a false example for the next reader, who will
believe the enum has two public variants.

## Design

Delete the block. Do not restore the method: no caller needs it, and a clone of a
`RawStream` must not copy the `OnceLock<ReadWakeSource>` (feature 141), because the copy
would share a source with the original or build a second one for the same fd.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_netio --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_netio
```

## Done when

- [x] The block is gone; the file compiles under the default and all-feature sets.
- [x] The backlog §2 entry is deleted.
- [ ] CI green. (The local checks pass; the Linux CI runs after the push.)

## Result

- Deleted the commented-out `clone_stream` block in
  `backends/foundation_netio/src/native/raw_stream.rs`, between `set_read_timeout`
  and `addrs`. The method is not restored. Nothing else in the file changed.
- `grep -rn "RawStream::AsPlain\|RawStream::AsTls" backends/foundation_netio/src/`
  returns no match. The other `clone_stream` methods in the crate belong to
  `SharedByteBufferStream`, the SSE stream and the HTTP connection. They are not
  related and stay as they are.
- Deleted the backlog §2 entry "The commented-out `clone_stream` block names
  removed variants".
- Commands, from the repository root, on 2026-11-06:
  - `cargo fmt --all --check`: exit 0, no output.
  - `cargo clippy --profile uat -p foundation_netio --all-targets --no-deps -- -D warnings`:
    exit 0, no warning.
  - `cargo test --profile uat -p foundation_netio`: exit 0. Every binary
    reports 0 failed: unittests 5 passed, `cookie_tests` 6 passed, `tests/mod.rs`
    1221 passed, `load_tracker_tests` 9 passed, `read_wake_source_tests` 3
    passed, doc-tests 54 passed. That is 1298 passed in all, 0 failed, 0
    ignored.
  - `cargo check --profile uat -p foundation_netio --all-features`: exit 0.
    Cargo prints one unused dependency warning for `openssl`. The TLS imports
    prefer rustls over openssl, so the openssl crate is not used when both
    features are on. The default feature set is clean.
- No public API changed; the version stays 0.0.6.
