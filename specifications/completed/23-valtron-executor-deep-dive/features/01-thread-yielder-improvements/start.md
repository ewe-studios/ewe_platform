# Feature Start File

## Feature: 01-thread-yielder-improvements

**Purpose:** Fix CondVar/park_timeout mismatch in ThreadYielder

## Agent Workflow

### Phase 1: Read Requirements
- Read `feature.md` for detailed requirements
- Read parent spec `requirements.md` for context
- Understand the park_timeout vs CondVar mismatch

### Phase 2: Study Current Implementation
- Read `backends/foundation_core/src/valtron/executors/threads.rs`
- Identify ThreadYielder::yield_for() implementation
- Identify PoolGuard::shutdown() sequence

### Phase 3: Implement InterruptibleWait
- Create `backends/foundation_core/src/synca/interruptible_wait.rs`
- Implement using CondVar + wait_timeout
- Write comprehensive unit tests

### Phase 4: Integrate into ThreadYielder
- Add interruptible field to ThreadYielder
- Replace park_timeout with wait_timeout
- Add interrupt() method

### Phase 5: Update Shutdown Sequence
- Add interrupt_all_yielders() to ThreadRegistry
- Call from PoolGuard::shutdown()
- Ensure ordering: interrupt -> signal -> wait

### Phase 6: Verify
- Run new shutdown tests
- Verify < 1s shutdown with delayed tasks
- All existing tests pass

## Critical Implementation Details

### InterruptibleWait Structure
```rust
pub struct InterruptibleWait {
    condvar: Condvar,
    state: Mutex<WaitState>,
}
```

### ThreadYielder Changes
```rust
impl ProcessController for ThreadYielder {
    fn yield_for(&self, dur: Duration) {
        self.interruptible.wait_timeout(dur);  // NOT park_timeout!
    }
}
```

### Shutdown Sequence
```rust
pub fn shutdown(&self) {
    self.kill_signal.turn_on();
    self.interrupt_all_yielders();  // NEW: Wake yielding threads
    self.latch.signal_all();        // Wake CondVar waits
    self.waitgroup.wait();
    self.join_all_threads();
}
```

## Success Criteria

- [ ] InterruptibleWait created with tests
- [ ] ThreadYielder uses wait_timeout
- [ ] Shutdown calls interrupt_all
- [ ] Test: < 1s shutdown with 10s delayed task
- [ ] No regressions in existing tests

---

*Version: 1.0 | Created: 2026-05-11*
