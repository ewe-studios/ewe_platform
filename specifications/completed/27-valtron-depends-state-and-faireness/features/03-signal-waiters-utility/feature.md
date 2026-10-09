---
feature: "SignalWaiters Utility"
description: "Standalone signal waiter map utility (not used by valtron executor), available as general-purpose tool"
status: "pending"
priority: "medium"
depends_on: []
estimated_effort: "small"
created: 2026-05-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# Feature 03: SignalWaiters Utility

## WHY: Problem Statement

Other code may need a general-purpose way to manage keyed atomic signals -- registering signals, checking which ones have fired, and updating them. While valtron's executor uses `Sleepers<Sleepable>` (which already supports `Sleepable::Atomic`), a standalone utility is useful for other consumers.

User: "i like those suggestions for the first idea, but lets also add this construct but not use it, just implement it, add tests so it can be used for other usecases, we can add it to foundation_core/src/synca/"

**Important:** This is NOT used by valtron's executor. The executor uses `Sleepers<Sleepable>` instead. This is a standalone utility.

## WHAT: Solution

### SignalWaiters<K> Struct

```rust
pub struct SignalWaiters<K: Eq + Hash + Clone + Send> {
    map: Arc<RwLock<HashMap<K, Arc<AtomicBool>>>>,
}

impl<K: Eq + Hash + Clone + Send + 'static> SignalWaiters<K> {
    /// Register a new signal. Returns Err if key already exists.
    pub fn add(&self, key: K, signal: Arc<AtomicBool>) -> Result<(), ErrorReport<CustomError>>;

    /// Update an existing signal. Returns Err if key does NOT exist.
    pub fn update(&self, key: K, signal: Arc<AtomicBool>) -> Result<(), ErrorReport<CustomError>>;

    /// Get keys whose signal is now true. Atomically removes them from the map.
    /// Returns None if no signals are ready.
    pub fn get_ready(&self) -> Option<Vec<K>>;

    /// Return the number of registered signals.
    pub fn count(&self) -> usize;

    /// Check if a key is registered.
    pub fn contains(&self, key: &K) -> bool;
}
```

### API Design Decisions

**Inverted add/update semantics:**
- `add()` -- Err if key already exists (prevents duplicates)
- `update()` -- Err if key does NOT exist (requires existing registration)

User agreed: "Ok make sense"

**`get_ready()` atomicity:**
The method atomically identifies all keys whose `AtomicBool` is `true` and removes them from the map in a single lock acquisition. This prevents race conditions where a signal fires between the check and the removal.

**Error handling:**
Uses `foundation_errstack` for errors, consistent with the project's error handling patterns.

## HOW: Implementation Steps

### Step 1: Create file

Create `backends/foundation_core/src/synca/signal_waiters.rs`

### Step 2: Implement `SignalWaiters<K>`

- Constructor: `new()` returns empty map
- `add()`: acquire write lock, check key not present, insert, release
- `update()`: acquire write lock, check key present, replace signal, release
- `get_ready()`: acquire write lock, iterate, check each AtomicBool, collect ready keys, remove them, release
- `count()`: acquire read lock, return len
- `contains()`: acquire read lock, check key

### Step 3: Error types

Define `SignalWaitersError` enum:
```rust
pub enum SignalWaitersError {
    KeyAlreadyExists,
    KeyNotFound,
}
```

Wrap in `ErrorReport<SignalWaitersError>` using `foundation_errstack`.

### Step 4: Export from synca

Update `backends/foundation_core/src/synca/mod.rs`:
```rust
mod signal_waiters;
pub use signal_waiters::{SignalWaiters, SignalWaitersError};
```

### Step 5: Unit Tests

Test cases:
1. `add()` new key -> success, count increases
2. `add()` duplicate key -> Err(KeyAlreadyExists)
3. `update()` existing key -> success, signal replaced
4. `update()` missing key -> Err(KeyNotFound)
5. `get_ready()` with no ready signals -> None
6. `get_ready()` with ready signals -> returns keys, removes them, count decreases
7. `contains()` for registered key -> true
8. `contains()` for unregistered key -> false
9. Multi-threaded test: multiple threads call `add()`, then one thread flips signals, `get_ready()` collects all

## Target Files

- `backends/foundation_core/src/synca/signal_waiters.rs` (new) -- SignalWaiters implementation
- `backends/foundation_core/src/synca/mod.rs` -- export

## Tests

```bash
cargo test --package foundation_core -- synca::signal_waiters
cargo test --package foundation_core
```

## Verification

```bash
cargo build --package foundation_core
cargo clippy --package foundation_core -- -D warnings
cargo fmt -- --check
cargo test --package foundation_core
```
