# Feature Start File

## Feature: 03-shutdown-mechanism-fixes

**Purpose:** Implement proper sleeper notification during shutdown

## Agent Workflow

### Phase 1: Read Requirements
- Read `feature.md` for detailed requirements
- Understand the sleeping task problem
- Read parent spec context

### Phase 2: Study Current Implementation
- Read `backends/foundation_core/src/valtron/executors/local.rs`
- Identify SleepersList and DurationWaker structures
- Find where sleepers are processed

### Phase 3: Update DurationWaker
- Add interrupted: AtomicBool field
- Implement interrupt() method
- Update should_wake() to check interrupted

### Phase 4: Update SleepersList
- Add interrupt_all() method
- Update process() to use should_wake()
- Ensure thread-safe interrupt

### Phase 5: Update Shutdown Sequence
- Call sleepers.interrupt_all() before wait
- Ensure ordering: kill_signal -> interrupt -> signal
- Update both multi and single-threaded

### Phase 6: Write Tests
- Test single delayed task shutdown
- Test multiple delayed tasks shutdown
- Measure timing to verify speed

### Phase 7: Verify
- Tests show < 1s shutdown with 10s delayed task
- Tests show < 200ms with 100 tasks
- All existing tests pass

## Critical Implementation Details

### DurationWaker
```rust
pub struct DurationWaker {
    entry_id: TaskEntryId,
    deadline: Instant,
    interrupted: AtomicBool,  // NEW
}

impl DurationWaker {
    pub fn should_wake(&self) -> bool {
        Instant::now() >= self.deadline 
            || self.interrupted.load(Ordering::Relaxed)
    }
    
    pub fn interrupt(&self) {
        self.interrupted.store(true, Ordering::Relaxed);
    }
}
```

### Shutdown Sequence
```rust
pub fn shutdown(&self) {
    self.kill_signal.turn_on();
    self.sleepers.interrupt_all();     // NEW: Wake sleepers
    self.interrupt_all_yielders();     // From Feature 01
    self.latch.signal_all();
    self.waitgroup.wait();
    self.join_all_threads();
}
```

## Success Criteria

- [ ] DurationWaker has interrupt flag
- [ ] should_wake() checks interrupt
- [ ] SleepersList::interrupt_all() implemented
- [ ] shutdown() calls interrupt_all
- [ ] Test: < 1s with 10s delayed task
- [ ] Test: < 200ms with 100 tasks
- [ ] No regressions

---

*Version: 1.0 | Created: 2026-05-11*
