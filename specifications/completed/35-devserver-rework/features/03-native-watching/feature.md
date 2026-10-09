---
feature: "Native Watching"
description: "Reuse foundation_nativeapis::valtron::FileWatcherTask — subscribe to its mpp broadcast, map WatchEvent → FileChange"
status: "complete"
priority: "high"
depends_on: ["02-task-operators"]
estimated_effort: "small"
created: "2026-06-01"
last_updated: "2026-06-15"
---

# Feature: Native Watching

## Problem

Current `DirectoryWatcher` in `crates/devserver/src/watchers.rs`:
- Uses `ewe_watch_utils::watch_path` which wraps `notify` + `notify-debouncer-full`
- Runs in `tokio::spawn_blocking` with a blocking `watcher_handler.0.join()`
- Sends events via `tokio::sync::broadcast::Sender<FileChange>`
- `FileChange` enum categorizes by extension: `Rust`, `Javascript`, `Typescript`, `Ruby`, `Any`

## Solution

**Reuse `foundation_nativeapis::valtron::FileWatcherTask` as-is.** It already:
- Wraps `NativeWatcher` (inotify on Linux, kqueue on macOS, ReadDirectoryChangesW on Windows)
- Uses `TaskStatus::Depends(CompositeReadiness)` — parks the task, zero CPU spinning
- Broadcasts `WatchEvent` to subscribers via `mpp::Receiver<WatchEvent>`
- Handles stop signals via `StopSignal`

We only need a thin adapter that subscribes to its broadcast and maps `WatchEvent → FileChange`:

```rust
// FileWatcherTask already exists in foundation_nativeapis — we just subscribe:
let mut builder_task = FileWatcherTask::new()?;
builder_task.watch(Path::new("src/"), true)?;
let mut event_rx = builder_task.subscribe();  // mpp::Receiver<WatchEvent>

// Spawn into valtron (already a TaskIterator):
engine.schedule(Box::new(builder_task))?;

// In ProjectBuilderTask (subscriber side):
fn next_status(&mut self) -> Option<TaskStatus<...>> {
    while let Ok(watch_event) = self.event_rx.try_recv() {
        let change = FileChange::from(&watch_event.path);
        if matches!(change, FileChange::Rust(_)) {
            self.pending_rebuild = true;
        }
    }
    if self.pending_rebuild {
        // do the build...
    }
    // No events → Depends(QueueReadiness) — parks until file change arrives
    Some(TaskStatus::Depends(Arc::new(self.ready_signal.clone())))
}
```

### Why Depends matters here

`FileWatcherTask` itself uses `TaskStatus::Depends(CompositeReadiness(watcher, stop_signal))` — on Linux this parks on inotify epoll, zero ticks. **Subscribers** (ProjectBuilderTask, SseReloadHandler) use `Depends(QueueReadiness)` — the queue itself is the readiness signal, no separate bool to flip.

### FileChange Enum (preserve current API)

Keep the existing `FileChange` enum for API compatibility — it categorizes changes by file extension, which each `ProjectBuilder` uses via `should_build()` to decide whether to trigger (e.g. `CargoBuilder` only on `FileChange::Rust`).

```rust
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum FileChange {
    Rust(PathBuf),
    Javascript(PathBuf),
    Typescript(PathBuf),
    Ruby(PathBuf),
    Any(PathBuf),
}

impl From<&PathBuf> for FileChange { ... }
```

### Two Watcher Instances

The current devserver uses **two** separate watchers:
1. **Builder watcher** — watches `build_directories` for Rust file changes → triggers cargo build
2. **Reloader watcher** — watches `reload_directories` for any file changes → triggers browser SSE reload

Both become separate `FileWatcherTask` instances with separate subscriptions.

### Task Breakdown

1. [ ] Define `FileChange` enum (copy from devserver, preserve API)
2. [ ] Wire `FileWatcherTask` from `foundation_nativeapis::valtron` — no implementation needed
3. [ ] Create `QueueReadiness<WatchEvent>` wrapping the broadcaster's queue — subscribers use `Depends(QueueReadiness)`
4. [ ] ProjectBuilderTask and SseReloadHandler subscribe to their own `QueueReadiness` for zero-spinning waits
5. [ ] Write tests: touch file, verify FileChange delivered to subscriber queue

### File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/src/watcher/mod.rs` | Create — FileChange enum + event pump helper |

---

_Created: 2026-06-01_
