---
feature: "Delete the stale commented clone_stream block in RawStream"
status: "planned"
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

- [ ] The block is gone; the file compiles under the default and all-feature sets.
- [ ] The backlog §2 entry is deleted.
- [ ] CI green.
