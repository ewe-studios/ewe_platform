---
feature: "PackedAtomic Utility"
description: "Generic CAS wrapper over AtomicU64 with AtomicPackable trait, enabling CAS on any type that fits in u64"
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "medium"
created: 2026-05-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# Feature 02: PackedAtomic Utility

## WHY: Problem Statement

Rust's stable only supports CAS on fixed-width atomic types (`AtomicU8`, `AtomicU16`, `AtomicU32`, `AtomicU64`, `AtomicBool`, `AtomicUsize`). There is no `Atomic<T>` for arbitrary `T` on stable Rust.

The fairness tracker needs to atomically update worker stats (total_tasks, active_tasks, total_waiters, elapsed_ms) stored as a packed struct. We need a reusable CAS wrapper that:
1. Works with any type that can be packed into `u64`
2. Provides a clean API with load/store/swap/update/compare_and_swap
3. Is generic enough for anyone to use with their own types

Why not a generic `AtomicValue<T>` with a generic struct? Because Rust's CPU only supports CAS on fixed-width registers. `PackedAtomic<T>` uses `AtomicU64` internally and the trait `AtomicPackable` handles encode/decode. Anyone can implement `AtomicPackable` for their own packed types.

User wanted: "the construct in foundation_nostd should not care and just require input to have a to_u64 method from a trait it defines, this way anyone can use this construct whenever." Also: "I want this in a struct construct that owns and has a nice API with methods to make this easy and encapsulated away."

Renamed from `AtomicValue` to `PackedAtomic<T>` to avoid conflict with std.

## WHAT: Solution

### AtomicPackable Trait

```rust
pub trait AtomicPackable: Copy + Send + Sync {
    /// Encode self into a u64 representation
    fn to_u64(&self) -> u64;
    /// Decode from a u64 representation
    fn from_u64(v: u64) -> Self;
}
```

### PackedAtomic<T> Struct

```rust
pub struct PackedAtomic<T: AtomicPackable> {
    inner: AtomicU64,
    _marker: PhantomData<T>,
}

impl<T: AtomicPackable> PackedAtomic<T> {
    /// Create a new PackedAtomic with the given value
    pub fn new(value: T) -> Self;

    /// Load the current value
    pub fn load(&self, ordering: Ordering) -> T;

    /// Store a new value
    pub fn store(&self, value: T, ordering: Ordering);

    /// Atomically swap with a new value, returning the old
    pub fn swap(&self, value: T, ordering: Ordering) -> T;

    /// CAS retry loop: apply f until successful, returns the final value
    pub fn update(&self, f: impl FnMut(T) -> T) -> T;

    /// Single CAS attempt: returns the current value (whether swap succeeded or not)
    pub fn compare_and_swap(&self, current: T, new: T, ordering: Ordering) -> T;
}
```

### StatsSnapshot Struct (used by Feature 04)

```rust
/// Bitfield layout (u64):
/// [bits 0-15]   total_tasks    (u16) - cumulative tasks this worker has handled
/// [bits 16-31]  active_tasks   (u16) - tasks currently being processed
/// [bits 32-47]  total_waiters  (u16) - sleeping + signal-waiting combined
/// [bits 48-63]  elapsed_ms     (u16) - ms since current task started, truncated to 65535
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatsSnapshot {
    pub total_tasks: u16,
    pub active_tasks: u16,
    pub total_waiters: u16,
    pub elapsed_ms: u16,
}

impl AtomicPackable for StatsSnapshot {
    fn to_u64(&self) -> u64 {
        (self.elapsed_ms as u64) << 48
            | (self.total_waiters as u64) << 32
            | (self.active_tasks as u64) << 16
            | (self.total_tasks as u64)
    }

    fn from_u64(v: u64) -> Self {
        StatsSnapshot {
            total_tasks: (v & 0xFFFF) as u16,
            active_tasks: ((v >> 16) & 0xFFFF) as u16,
            total_waiters: ((v >> 32) & 0xFFFF) as u16,
            elapsed_ms: ((v >> 48) & 0xFFFF) as u16,
        }
    }
}
```

## HOW: Implementation Steps

### Step 1: Create module

Create `backends/foundation_nostd/src/atomics/mod.rs`:
```rust
mod packed;
mod time_tracker;

pub use packed::{AtomicPackable, PackedAtomic};
pub use time_tracker::TimeTracker;
```

### Step 2: Implement `AtomicPackable` trait

In `backends/foundation_nostd/src/atomics/packed.rs`:
- Define trait with `to_u64` and `from_u64`
- Add `Send + Sync + Copy` bounds

### Step 3: Implement `PackedAtomic<T>`

In the same file:
- Struct with `AtomicU64` inner and `PhantomData<T>`
- `new(value)` -- encode and store initial value
- `load(ordering)` -- load u64, decode to T
- `store(value, ordering)` -- encode, store to u64
- `swap(value, ordering)` -- encode, atomic swap, decode old
- `update(f)` -- CAS retry loop: load -> apply f -> compare_and_swap, retry on failure
- `compare_and_swap(current, new, ordering)` -- single CAS attempt

### Step 4: Implement `StatsSnapshot`

In the same file:
- Define struct with 4 u16 fields
- Implement `AtomicPackable` with bitfield encode/decode
- Add unit tests for round-trip correctness

### Step 5: Export from foundation_nostd

Update `backends/foundation_nostd/src/lib.rs`:
```rust
pub mod atomics;
pub use atomics::{AtomicPackable, PackedAtomic, TimeTracker};
```

### Step 6: Unit Tests

Test cases:
1. `PackedAtomic<T>::new` -> `load()` returns initial value
2. `store()` -> `load()` returns new value
3. `swap()` returns old value, stores new
4. `update()` CAS retry loop converges under contention (multi-threaded test)
5. `compare_and_swap()` returns current value when mismatch
6. `StatsSnapshot` bitfield round-trip: encode -> decode -> same values
7. `StatsSnapshot` max values (65535) encode/decode correctly
8. `StatsSnapshot` zero values encode/decode correctly

## Target Files

- `backends/foundation_nostd/src/atomics/mod.rs` (new) -- module declaration
- `backends/foundation_nostd/src/atomics/packed.rs` (new) -- AtomicPackable, PackedAtomic, StatsSnapshot
- `backends/foundation_nostd/src/lib.rs` -- export atomics module

## Tests

```bash
cargo test --package foundation_nostd -- atomics::packed
cargo test --package foundation_nostd
```

## Verification

```bash
cargo build --package foundation_nostd
cargo clippy --package foundation_nostd -- -D warnings
cargo fmt -- --check
cargo test --package foundation_nostd
```
