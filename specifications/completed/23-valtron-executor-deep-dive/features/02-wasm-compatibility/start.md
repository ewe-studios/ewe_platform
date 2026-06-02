# Feature Start File

## Feature: 02-wasm-compatibility

**Purpose:** Design no_std-compatible yielding for WASM and single-threaded

## Agent Workflow

### Phase 1: Read Requirements
- Read `feature.md` for detailed requirements
- Understand WASM constraints (no std::thread, no blocking)
- Understand no_std constraints (limited std)

### Phase 2: Study Current Implementation
- Read `backends/foundation_core/src/valtron/executors/single/mod.rs`
- Identify NoThreadController::yield_for() (currently no-op)
- Study how it integrates with executor

### Phase 3: Design Platform Abstraction
- Create `PlatformYielder` trait
- Design `StdYielder`, `WasmYielder`, `NoStdYielder`
- Use #[cfg] for platform selection

### Phase 4: Implement StdYielder
- Uses InterruptibleWait from Feature 01
- Full functionality for std environments

### Phase 5: Implement WasmYielder Stub
- Define interface for WASM
- Mark as unimplemented!() with clear docs
- Note async runtime requirement

### Phase 6: Implement NoStdYielder
- Spin-loop with yield hints
- Interrupt mechanism
- Document power consumption trade-off

### Phase 7: Update NoThreadController
- Replace no-op implementation
- Use PlatformYielderFactory
- Ensure proper yield_for behavior

### Phase 8: Verify
- Native tests pass
- WASM compiles (even if tests don't run)
- No busy-waiting in single-threaded

## Critical Implementation Details

### Platform Selection
```rust
#[cfg(target_arch = "wasm32")]
type Yielder = WasmYielder;

#[cfg(all(not(target_arch = "wasm32"), feature = "std"))]
type Yielder = StdYielder;

#[cfg(not(feature = "std"))]
type Yielder = NoStdYielder;
```

### NoStd Yield
```rust
fn yield_for(&self, dur: Duration) {
    let start = platform_now();
    while platform_now() - start < dur {
        core::hint::spin_loop();
        if interrupted { break; }
    }
}
```

## Success Criteria

- [ ] PlatformYielder trait defined
- [ ] StdYielder implemented
- [ ] WasmYielder stub implemented
- [ ] NoStdYielder implemented
- [ ] NoThreadController uses platform yielder
- [ ] Native tests pass
- [ ] WASM compiles

---

*Version: 1.0 | Created: 2026-05-11*
