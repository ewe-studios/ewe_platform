---
feature: "Native Watching"
description: "Replace DirectoryWatcher (notify-based) with NativeWatcher from foundation_nativeapis — FileChange enum + thin valtron FileWatcherTask adapter"
status: "pending"
priority: "high"
depends_on: ["02-task-operators", "specifications/34-native-file-watchers/01-native-apis"]
estimated_effort: "medium"
created: 2026-06-01
last_updated: 2026-06-01
---

# Feature: Native Watching

## Problem

Current `DirectoryWatcher` in `crates/devserver/src/watchers.rs`:
- Uses `ewe_watch_utils::watch_path` which wraps `notify` + `notify-debouncer-full`
- Runs in `tokio::spawn_blocking` with a blocking `watcher_handler.0.join()`
- Sends events via `tokio::sync::broadcast::Sender<FileChange>`
- `FileChange` enum categorizes by extension: `Rust`, `Javascript`, `Typescript`, `Ruby`, `Any`

## Solution

Replace with `FileWatcherTask` that wraps `foundation_nativeapis::NativeWatcher`:

```rust
pub struct FileWatcherTask {
    watcher: Box<dyn NativeWatcher>,
    change_queue: Arc<ConcurrentQueue<FileChange>>,
    watched_paths: Vec<PathBuf>,
    poll_interval: Duration,
}

impl TaskIterator for FileWatcherTask {
    type Ready = ();
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // Poll the native watcher (sync, blocks up to 50ms)
        match self.watcher.poll(Duration::from_millis(50)) {
            Ok(events) => {
                for event in events {
                    let change = FileChange::from(&event.path);
                    let _ = self.change_queue.push(change);
                }
            }
            Err(e) => tracing::warn!("watcher poll error: {}", e),
        }

        // Yield back to valtron
        Some(TaskStatus::Wait(self.poll_interval))
    }
}
```

The valtron task is a thin adapter. `NativeWatcher::poll()` does all the work.

### FileChange Enum (preserve current API)

Keep the existing `FileChange` enum for API compatibility — it categorizes changes by file extension, which the CargoBuilderTask uses to decide whether to rebuild (only on `FileChange::Rust`).

### Two Watcher Instances

The current devserver uses **two** separate watchers:
1. **Builder watcher** — watches `build_directories` for Rust file changes → triggers cargo build
2. **Reloader watcher** — watches `reload_directories` for any file changes → triggers browser SSE reload

Both become separate `FileWatcherTask` instances with separate `ConcurrentQueue`s.

### Task Breakdown

1. [ ] Define `FileChange` enum (copy from devserver, preserve API)
2. [ ] Implement `FileWatcherTask` struct wrapping `NativeWatcher`
3. [ ] Implement `TaskIterator` for `FileWatcherTask`
4. [ ] Implement `From<PathBuf> for FileChange` (extension-based categorization)
5. [ ] Write tests: touch file, verify FileChange event delivered to queue
6. [ ] Verify wasm32 compilation (stub/no-op watcher)

## Dependencies

- **Feature 01** (scaffolding) must be complete
- **Spec 34 Feature 01** (native-apis) must be complete — `NativeWatcher` trait must exist
- Requires `foundation_nativeapis` with `watcher` feature enabled

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/src/watcher/mod.rs` | Create — FileChange + FileWatcherTask |

---

_Created: 2026-06-01_
