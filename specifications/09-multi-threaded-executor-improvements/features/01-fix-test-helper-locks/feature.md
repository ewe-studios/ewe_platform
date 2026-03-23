---
feature: "Fix Test Helper Lock Usage"
description: "Optimize DCounter and Counter lock patterns in existing multi-threaded executor tests"
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "small"
created: 2026-03-23
author: "Main Agent"
tasks:
  completed: 4
  uncompleted: 0
  total: 4
  completion_percentage: 100%
---

# Fix Test Helper Lock Usage

## WHY: Problem Statement

In `backends/foundation_core/src/valtron/executors/multi/mod.rs`, the test helpers `DCounter` and `Counter` have inefficient lock usage:

**DCounter** (lines 161-171) acquires the same mutex **3 times** in one `next_status()` call:
```rust
fn next_status(&mut self) -> Option<...> {
    let item_size = self.1.lock().unwrap().len();  // Lock #1
    if item_size == self.0 { return None; }
    self.1.lock().unwrap().push(item_size);        // Lock #2
    Some(TaskStatus::Ready(
        self.1.lock().unwrap().len()               // Lock #3
    ))
}
```

**Counter** (lines 195-205) holds the mutex while calling `self.2.send()` which could block:
```rust
fn next_status(&mut self) -> Option<...> {
    let mut items = self.1.lock().unwrap();         // Lock acquired
    let item_size = items.len();
    if item_size == self.0 {
        self.2.send(()).expect("send signal");      // Send while holding lock!
        return None;
    }
    items.push(item_size);
    Some(TaskStatus::Ready(items.len()))
}
```

This is a prerequisite fix independent of the architecture change.

---

## WHAT: Solution

### DCounter — Single Lock Acquisition

```rust
fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
    let mut items = self.1.lock().unwrap();
    let item_size = items.len();

    if item_size == self.0 {
        return None;
    }

    items.push(item_size);
    let new_len = items.len();

    Some(crate::valtron::TaskStatus::Ready(new_len))
}
```

### Counter — Release Lock Before Send

```rust
fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
    tracing::debug!("Counter Task is running");

    let result = {
        let mut items = self.1.lock().unwrap();
        let item_size = items.len();

        if item_size == self.0 {
            None  // signal "done" — will send after lock release
        } else {
            items.push(item_size);
            Some(crate::valtron::TaskStatus::Ready(items.len()))
        }
    }; // lock released here

    match result {
        None => {
            tracing::debug!("Sending signal with sender");
            self.2.send(()).expect("send signal");
            None
        }
        some => some,
    }
}
```

---

## Tasks

- [ ] Rewrite `DCounter::next_status()` to acquire lock once
- [ ] Rewrite `Counter::next_status()` to release lock before `send()`
- [ ] Run `cargo test --package foundation_core -- multi_threaded_tests` — all pass
- [ ] Run `cargo clippy --package foundation_core -- -D warnings` — clean

## Files Changed

- `backends/foundation_core/src/valtron/executors/multi/mod.rs` (test module only)

## Verification

```bash
cargo test --package foundation_core -- multi_threaded_tests
cargo clippy --package foundation_core -- -D warnings
```
