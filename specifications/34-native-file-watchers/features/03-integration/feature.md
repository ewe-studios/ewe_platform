---
feature: "Integration Tests & Cross-Platform Verification"
description: "Cross-platform compilation checks, integration tests, documentation, and example usage"
status: "pending"
priority: "medium"
depends_on: ["01-native-apis", "02-valtron-watcher-task"]
estimated_effort: "small"
created: 2026-06-01
last_updated: 2026-06-01
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0%
---

# Feature: Integration Tests & Cross-Platform Verification

## Problem

After implementing the native APIs and valtron task, we need to verify:

1. The crate compiles on all target platforms
2. The watcher actually detects file changes on the current platform
3. The valtron integration delivers events correctly
4. Users have working examples to reference

## Solution

Integration tests and cross-platform compilation checks.

## Implementation Plans

### Task Breakdown

1. [ ] Verify native compilation: `cargo check -p foundation_nativeapis`
2. [ ] Verify wasm32 compilation: `cargo check -p foundation_nativeapis --target wasm32-unknown-unknown`
   - On wasm32, `native_watcher()` should return a no-op stub or `PollWatcher`
3. [ ] Run integration test on Linux:
   ```bash
   cargo test -p foundation_nativeapis --test integration_test
   ```
   - Creates temp dir, writes file, verifies `WatchEvent::Created` and `WatchEvent::Modified` received
4. [ ] Write example `backends/foundation_nativeapis/examples/file_watcher.rs`:
   ```rust
   fn main() {
       let mut watcher = foundation_nativeapis::native_watcher();
       watcher.watch("src/", true).unwrap();
       println!("Watching src/ for changes...");
       loop {
           let events = watcher.poll(Duration::from_millis(100)).unwrap();
           for event in events {
               println!("{:?}: {:?}", event.kind, event.path);
           }
       }
   }
   ```

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_nativeapis/tests/integration_test.rs` | Create |
| `backends/foundation_nativeapis/examples/file_watcher.rs` | Create |
| `backends/foundation_nativeapis/README.md` | Create |

---

_Created: 2026-06-01_
