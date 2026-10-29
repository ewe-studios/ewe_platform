---
feature: "Limits as configuration (valtron, buffer pool, background jobs, logs, SpinWaiter)"
status: "done"
priority: "medium"
depends_on: ["01-valtron-parking"]
branch: "fix/io-09-limits-as-config"
crates: ["foundation_core", "foundation_nostd"]
backlog: "docs/bug-backlog.md §4 Hard-coded valtron limits; BackgroundJobRegistry; BytesPool has no bounds; BatchReader logs every WouldBlock at error!; Release coupling; §5 SpinWaiter"
---

# Feature 09: Limits as configuration

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** pull request (PR); AeroSpace and Defence Industries Association
of Europe (ASD), the owner of the STE standard ASD-STE100.

`CLAUDE.md` rule 8: size caps, timeouts, buffer and batch sizes are fields on a config with a
builder method and a documented default that names its source. Never a `const` in the code path.

## 1. Items

| # | Item | Where | Do |
|---|---|---|---|
| L1 | `MAX_YIELD_DURATION = 100 ms` | `foundation_core/src/valtron/executors/local.rs:79` | A field on the executor config (the one `initialize_pool_with` / `#[valtron(...)]` builds), default 100 ms, doc: why (the idle re-check cap). Pass it to the code at line ~2883. |
| L2 | `DEFAULT_YIELD_WAIT_TIME = 4 s`, `BACK_OFF_JITER`, `BACK_OFF_THREAD_FACTOR`, `BACK_OFF_MIN_DURATION`, `BACK_OFF_MAX_DURATION` | `executors/constants.rs:11, 66–69` | Fields on the pool config with these defaults. Keep the constants only as the default values, named `DEFAULT_…`, used in exactly one place (the config's `Default`). |
| L3 | `ThreadedIterFuture::new` queue size 16 and 10 ms back-pressure wait | `executors/future_task.rs` | Builder methods exist. Document the defaults on them; make `new` use a `ThreadedIterFutureConfig` (or the existing builder) default, not literals. After feature 01 the back-pressure value is a park timeout, not a sleep; name it so. |
| L4 | `BackgroundJobRegistry` queue unbounded; `new` panics if a thread cannot spawn | `executors/background.rs:207, 235` | Add a capacity (config, default documented); a full queue returns an error to the caller. `new` returns `Result<Arc<Self>, BackgroundJobError>`; update callers. |
| L5 | `BytesPool` unbounded and keeps buffers of any size | `foundation_core/src/io/buffer_pool.rs:44` | Config: `max_pooled_buffers` and `max_buffer_size` (defaults documented; for example 64 buffers and 64 KiB, as Go's `bufio` defaults are 4 KiB to 64 KiB; pick and name the source). A returned buffer larger than `max_buffer_size`, or beyond the count, is dropped. |
| L6 | `acquire_with_capacity` counts a hit and an allocation when the pooled buffer is too small | `buffer_pool.rs:132` | Count one outcome: a hit only when the pooled buffer is used as is; a miss + allocation otherwise. |
| L7 | `PooledBuffer::owned` builds a throwaway pool for every buffer | `buffer_pool.rs:232` | Hold `Option<Arc<BytesPool>>`; `owned` has `None` and returns nothing on drop. |
| L8 | `PooledBuffer` has no `Debug` | `buffer_pool.rs` | Implement `Debug` (length and capacity, not the bytes). |
| L9 | `BatchReader` logs every `WouldBlock` at `error!` | `foundation_core/src/io/readers/mod.rs` (the `WouldBlock`/`TimedOut` arm) | `trace!` for `WouldBlock`; keep `TimedOut` at `debug`. |
| L10 | `SpinWaiter` rounds sub-millisecond waits to zero; arbitrary spin calibration | `foundation_nostd/src/primitives/spin_waiter.rs:32, 105` | Keep nanosecond precision in the wait; calibration constants become fields with documented defaults; a wait of 500 µs waits at least 500 µs. |
| L11 | Release coupling: `foundation_macros` calls `foundation_core::valtron::initialize_current_thread` | `foundation_macros` | Not code: add a "Publishing" note to both crates' `README.md` (or the release doc that exists) saying they must be bumped together, and a `release-plz.toml` group if the tool supports it (check). |

## 2. Tasks

- [x] T1. Branch from the base after feature 01.
- [x] T2. L1–L3 (valtron config). One commit.
- [x] T3. L4 (`BackgroundJobRegistry`). One commit.
- [x] T4. L5–L8 (`BytesPool`). One commit.
- [x] T5. L9, L10, L11. One commit each.
- [x] T6. Tests (each crate's `tests/`, registered in `mod.rs`):
  - `foundation_core`: `pool_config_overrides_yield_and_backoff` (a pool built with a custom
    `max_yield` uses it: measure an idle re-check interval with a 20 ms setting; assert
    20 ms ± 10 ms).
  - `background_registry_full_queue_returns_error`; `background_registry_spawn_failure_is_an_error`
    (inject a spawn failure through a test hook or a thread limit; if not possible without
    unsafe tricks, test the error type path and say so).
  - `bytes_pool_drops_oversized_buffers`; `bytes_pool_caps_pooled_count`;
    `bytes_pool_counts_one_outcome_per_acquire`; `owned_buffer_has_no_pool`;
    `pooled_buffer_debug_shows_len_and_capacity`.
  - `batch_reader_would_block_is_not_an_error_log` (capture logs with a test subscriber; assert no
    `ERROR` record for `WouldBlock`).
  - `foundation_nostd`: `spin_waiter_waits_at_least_sub_millisecond` (500 µs request, measured
    ≥ 500 µs, 100 runs).
- [x] T7. Delete the backlog entries. Update `progress.md`, `learnings.md`.

## 3. Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_core --profile dev
python3 scripts/ci/ci.py run foundation_nostd --profile dev
cargo check --workspace --all-targets --profile dev   # L4 changes a signature; every caller must build
# 20-run flake check for each timing test.
rg -n 'const .*(Duration|_SIZE|_CAPACITY|_SPINS)' backends/foundation_core/src/valtron/executors backends/foundation_core/src/io/buffer_pool.rs
#   every hit is a DEFAULT_… used only by a config Default impl
```

## 4. Done when

- [x] All §3 commands pass; timing tests passed 20 runs in a row.
- [x] Every item L1–L11 is done or has a written reason.
- [x] No caller of `BackgroundJobRegistry::new` ignores its error.
- [x] CI green.

## 5. Review checklist (main session)

- [ ] Defaults unchanged in value (no behaviour change unless the item says so).
- [ ] Each default's doc comment names its source.
- [ ] L4's new error is handled, not unwrapped, at every caller.
