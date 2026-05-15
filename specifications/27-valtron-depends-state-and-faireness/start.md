# Workflow: Valtron — TaskStatus::Depends & Worker Fairness Tracker

## Entry Point

This specification adds two capabilities to Valtron:
1. **`TaskStatus::Depends`** — signal-based task waiting via `Arc<AtomicBool>`
2. **Worker Fairness Tracker** — CAS-based global queue access control

Start by reading **`Spec.md`** for the full architecture and design decisions, then **`analysis.md`** for the current state and gap analysis.

## Implementation Order

```
01-taskstatus-depends-state ──┐
02-packed-atomic-utility      │
03-signal-waiters-utility     │
                              │
04-worker-fairness-tracker ───┘ (depends on 02)
                              │
05-integration-and-testing ───┘ (depends on 01, 04)
```

### Phase 1: Independent Features (can be done in any order)

| Feature | File | Description |
|---------|------|-------------|
| [01](features/01-taskstatus-depends-state/feature.md) | `features/01-taskstatus-depends-state/feature.md` | Add `TaskStatus::Depends`, wire to `Sleepable::Atomic`, zombie detection |
| [02](features/02-packed-atomic-utility/feature.md) | `features/02-packed-atomic-utility/feature.md` | `PackedAtomic<T>` generic CAS wrapper in foundation_nostd |
| [03](features/03-signal-waiters-utility/feature.md) | `features/03-signal-waiters-utility/feature.md` | `SignalWaiters<K>` standalone utility in foundation_core |

### Phase 2: Dependent Features

| Feature | Depends On | File |
|---------|-----------|------|
| [04](features/04-worker-fairness-tracker/feature.md) | 02 | `features/04-worker-fairness-tracker/feature.md` |
| [05](features/05-integration-and-testing/feature.md) | 01, 04 | `features/05-integration-and-testing/feature.md` |

## Task Tracking

All tasks are tracked in **`requirements.md`**. Update it after completing each task.

## Target Files

| File | Features |
|------|----------|
| `backends/foundation_core/src/valtron/task.rs` | 01 |
| `backends/foundation_core/src/valtron/executors/local.rs` | 01, 05 |
| `backends/foundation_core/src/valtron/executors/threads.rs` | 04, 05 |
| `backends/foundation_core/src/synca/mod.rs` | 03 |
| `backends/foundation_core/src/synca/signal_waiters.rs` | 03 (new) |
| `backends/foundation_nostd/src/lib.rs` | 02, 04 |
| `backends/foundation_nostd/src/atomics/mod.rs` | 02, 04 (new) |
| `backends/foundation_nostd/src/atomics/packed.rs` | 02 (new) |
| `backends/foundation_nostd/src/atomics/time_tracker.rs` | 04 (new) |

## Verification

After each feature:

```bash
cargo build --package foundation_core
cargo build --package foundation_nostd
cargo clippy --package foundation_core -- -D warnings
cargo clippy --package foundation_nostd -- -D warnings
cargo fmt -- --check
cargo test --package foundation_core
cargo test --package foundation_nostd
```

## Agent Rules

- `.agents/rules/01-rule-naming-and-structure.md`
- `.agents/rules/02-rules-directory-policy.md`
- `.agents/rules/03-dangerous-operations-safety.md`
- `.agents/rules/04-work-commit-and-push-rules.md`
- `.agents/rules/13-implementation-agent-guide.md`
- `.agents/stacks/rust.md`

## Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `DEPENDS_ZOMBIE_CYCLE_THRESHOLD` | 10,000 | Max cycles before forced re-poll |
| `DEPENDS_TRUE_PANIC_THRESHOLD` | 3 | Consecutive `Depends(true)` violations before panic |

---

*See `Spec.md` for architecture. See `requirements.md` for task tracking.*
