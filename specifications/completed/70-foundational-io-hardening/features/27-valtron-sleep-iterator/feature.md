---
feature: "valtron SleepIterator returns Done for a reached deadline"
status: "done"
priority: "high"
depends_on: "—"
branch: "fix/io-27-valtron-sleep-iterator"
crates: ["foundation_core"]
backlog: "docs/bug-backlog.md §4 (1 entry)"
---

# Feature 27: `SleepIterator` reached-deadline bug

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** pull request (PR); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4.

## 1. Goal

`SleepIterator::next` must return `Done` when its deadline is reached. Today it returns
`Pending(0ns)` when the deadline equals the current time. That makes
`zero_duration_sleep_iterator_finishes_immediately` flake under parallel load.

## 2. Where

`foundation_core/src/valtron/delayed_iterators.rs`, `SleepIterator::next`:

```rust
let now = std::time::Instant::now();
let result = match self.0.checked_add(self.1) {
    Some(completed_at) => match completed_at.checked_duration_since(now) {
        Some(diff) => Delayed::Pending(self.0, self.1, diff), // diff can be zero
        _ => Delayed::Done(self.2.take().expect("item should not be taken yet")),
    },
    _ => Delayed::Done(self.2.take().expect("item should not be taken yet")),
};
```

`completed_at.checked_duration_since(now)` returns `Some(Duration::ZERO)` when
`completed_at == now`. The `Some(diff)` arm then returns `Pending(0ns)`. A deadline with no
time left is reported as still pending.

## 3. Fix

Return `Pending` only for a strictly positive remaining time. Return `Done` for `None` (past)
and for `Some(Duration::ZERO)` (exactly now).

## 4. Test (deterministic)

The reached-deadline decision must be testable without `Instant::now` advancing. Extract the
classification into a pure function. For example `fn remaining(completed_at: Option<Instant>,
now: Instant) -> Option<Duration>` returns `Some(d)` only when `d > Duration::ZERO`. Test it at
the boundary:

- `remaining(Some(now), now)` is `None` (Done).
- `remaining(Some(now + 1ms), now)` is `Some(1ms)` (Pending).
- `remaining(Some(now - 1ms), now)` is `None` (Done).
- `remaining(None, now)` is `None` (Done).

Keep `zero_duration_sleep_iterator_finishes_immediately`. After the fix it is deterministic.
It must pass 20 runs in a row in both feature sets.

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_core --profile dev
for i in $(seq 1 20); do
  cargo test -p foundation_core --profile dev --test mod valtron::delayed_iterators \
    || { echo "FAIL run $i"; exit 1; }
done
for i in $(seq 1 20); do
  cargo test -p foundation_core --profile dev --features multi --test mod valtron::delayed_iterators \
    || { echo "FAIL multi run $i"; exit 1; }
done
```

## Done when

- [x] `SleepIterator::next` returns `Done` for `completed_at == now`.
- [x] The boundary test passes. `zero_duration_sleep_iterator_finishes_immediately` passes
      20 runs in a row in both feature sets.
- [x] The backlog entry is deleted.
- [x] CI green.
