---
feature: "crates/channels and crates/domain: no println on wake, capacities from config, correct is_closed"
status: "dropped"
lane: "B"
model: "haiku"
branch: "fix/io-51-60-lane-b"
crates: ["crates/channels", "crates/domain"]
catalogue_rows: [27]
---

> **Dropped by the owner (2026-11-04).** Old code; not scheduled. Do not work on it. Its backlog entries move to the backlog "Paused" section (feature 60).

# Feature 57: channels and domain

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

- **E1** `crates/channels/src/executor.rs` ~40: `println!("Sent wake action")` on every
  wake. Delete it (a `tracing::trace!` only if the crate already uses tracing).
- **E2** `DEFAULT_TASK_PENDING_CAPACITY = 10` (channels executor) and
  `DEFAULT_SUBSCRIBER_START_CAPACITY = 10` (`crates/domain/src/servicer.rs` ~15): move to
  config fields with builder methods and documented defaults (rule 8). Keep the default value.
- **E3** `ReceiveChannel::is_closed` (`crates/channels/src/mspc.rs`) returns `Ok(false)`
  after every sender dropped. Return the underlying channel's closed state
  (`async_channel::Receiver::is_closed` or the crate's own channel), keeping the local
  close flag too. Test `is_closed_after_all_senders_drop` (fails on master).

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo test -p ewe_channels -p ewe_domain --profile dev
cargo clippy -p ewe_channels -p ewe_domain --profile dev --all-targets
```
(Check the package names in each `Cargo.toml`.)

## Done when

- [ ] Three fixes, new test fails on master. §18 and §19 entries deleted.
