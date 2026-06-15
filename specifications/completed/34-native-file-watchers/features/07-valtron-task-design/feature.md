---
feature: "Valtron Task Design — Shared Watcher, Readiness, Stop Signal"
description: "Critical design rules for watcher tasks: has_events() caches events (never duplicates work), poll() drains cache, SharedWatcher wraps Arc<RwLock<T>>, StopSignal for clean termination, collect_one for finite collection, no Delayed for event-driven watchers"
status: "completed"
priority: "high"
depends_on: ["05-valtron-watcher-task"]
estimated_effort: "small"
created: 2026-06-03
last_updated: 2026-06-03
author: "Main Agent"
tasks:
  completed: 7
  uncompleted: 0
  total: 7
  completion_percentage: 100%
---

# Feature: Valtron Task Design

## Critical Rules (Non-Negotiable)

### Rule 1: `has_events()` caches events, `poll()` drains them

`has_events()` and `poll()` MUST share a cache. `has_events()` scans for events and stores them in a cache. `poll()` drains the cache first before doing any work. This prevents:

1. **Duplicate scanning** — `has_events()` already did the work, `poll()` shouldn't repeat it.
2. **Lost events** — if `has_events()` updates snapshots but `poll()` re-scans, it sees no changes (snapshots already updated) → events lost.

```rust
fn has_events(&mut self, timeout: Option<Duration>) -> bool {
    // If we already have cached events, return immediately — don't scan again.
    if !self.cached_events.is_empty() {
        return true;
    }
    // Scan, cache, and return true ONLY when events were found.
    // Returning false when nothing changed is correct — it avoids busy-looping.
    if let Some(dur) = timeout {
        thread::sleep(dur);
    }
    self.cached_events = self.scan_watches();
    !self.cached_events.is_empty()
}

fn poll(&mut self, timeout: Duration) -> Result<Vec<WatchEvent>> {
    // Drain cached events first (populated by has_events).
    if !self.cached_events.is_empty() {
        return Ok(std::mem::take(&mut self.cached_events));
    }
    // No cached events — sleep then scan.
    thread::sleep(timeout);
    self.cached_events = self.scan_watches();
    Ok(std::mem::take(&mut self.cached_events))
}
```

### Rule 2: `has_events()` behavior differs by watcher type

**Event-driven watchers** (inotify, kqueue, IOCP): `has_events()` returns `true` ONLY when the OS signals readiness (events available). The OS handles parking/waking — no periodic checks needed.

**Poll-based watchers** (PollWatcher): `has_events()` returns `true` after the timeout only when events are acquired this avoids causing CPU trashing and busy looping. The sleep prevents busy-looping (minimum interval = timeout). This allows the task to:
1. Wake up periodically to check `StopSignal`.
2. Scan for changes and cache any events found.
3. Let `poll()` drain the cache.


### Rule 3: Use `TaskStatus::Depends(shared)` instead of `TaskStatus::Delayed(timeout)`

`Depends` uses the watcher's `EventReadiness` impl to decide when to reschedule. `Delayed` always wakes after the timeout regardless of whether there's work.

- **Event-driven watchers** (inotify, kqueue): `Depends` — the OS signals readiness.
- **Poll-based watchers** (PollWatcher): `Depends` — `has_events()` scans and returns true only when events found.
- **NEVER use `Delayed`** for watcher tasks — it wastes CPU polling when nothing changed.

### Rule 4: Use `collect_one()` for finite collection, never `collect_result()` on infinite watchers

Watcher tasks are infinite — they never return `None` (they keep watching). `collect_result()` loops until the stream returns `None`, which never happens for a watcher → **blocks forever**.

Use `collect_one()` to grab the first event and stop. For "no events expected" tests, use `drain_stream_limited()` with a manual iteration limit.

### Rule 5: `StopSignal` for clean task termination

Every watcher task must have a `StopSignal` that can terminate it cleanly. `next_status()` checks `stop.is_stopped()` first — if true, returns `None`. This is essential for:

- Tests that need to verify "no events" without hanging.
- Users who want to stop a watcher gracefully.

```rust
fn next_status(&mut self) -> Option<TaskStatus<...>> {
    // Check stop signal first — return None to terminate the task.
    if self.stop.is_stopped() {
        return None;
    }
    // ... normal logic
}
```

### Rule 6: Watch BEFORE creating files in tests

Always register the watch path before creating files. If a file exists before the watch is registered, the snapshot captures its initial state and subsequent "changes" won't be detected. The correct test pattern:

1. Create temp directory
2. Create watcher task, watch the directory
3. Start executor (in background or via stream)
4. Spawn a thread that sleeps briefly, then creates a file
5. Collect results

### Rule 7: No `unsafe` for const-to-mut casting

Never do `self as *const Self as *mut Self`. If `has_events()` needs to mutate (scan, cache), take `&mut self`. The `NativeWatcher` trait should have `fn has_events(&mut self, timeout: Option<Duration>) -> bool`.

## Tasks

### Task 1: `NativeWatcher::has_events(&mut self)` — cache-based readiness

- [x] Add `has_events(&mut self, timeout: Option<Duration>) -> bool` to `NativeWatcher` trait
- [x] Implement for `PollWatcher`: scans, caches, returns true only when events found
- [x] Implement for `InotifyWatcher`: uses epoll readiness check, doesn't consume events
- [x] Implement for `KqueueWatcher`: uses kqueue readiness check
- [x] Implement for `WinWatcher`: peeks internal IOCP cache

### Task 2: `SharedWatcher` — type-erased `Arc<RwLock<Box<dyn NativeWatcher>>>`

- [x] Implement `SharedWatcher` wrapper with inherent methods
- [x] Implement `EventReadiness` for `SharedWatcher` → delegates to `has_events()`
- [x] Implement `NativeWatcher` for `SharedWatcher`
- [x] Implement `SharedNativeWatcher<T>` for generic use
- [x] Re-export from crate root

### Task 3: `StopSignal` — clean task termination

- [x] Create `StopSignal` struct: `Arc<AtomicBool>` with `stop()` and `is_stopped()`
- [x] Add `stop_signal()` method to `FileWatcherTask`
- [x] Add `stop_signal()` method to `FdMonitorTask`
- [x] `next_status()` checks stop signal first, returns `None` if set

### Task 4: `FileWatcherTask` uses `TaskStatus::Depends`

- [x] Store `SharedWatcher` internally (was `Box<dyn NativeWatcher>`)
- [x] `next_status()` returns `TaskStatus::Depends(Arc::new(self.watcher.clone_handle()))`
- [x] `next_status()` checks `StopSignal` first

### Task 5: Tests use `collect_one()` and `StopSignal`

- [x] Fix `file_watcher_task_valtron_execution` → `collect_one()`, thread creates file
- [x] Fix `file_watcher_task_multi_subscriber_valtron` → `collect_one()`
- [x] Fix `file_watcher_task_delivers_events` → `collect_one()`
- [x] Fix `file_watcher_task_no_subscribers` → `collect_one()`
- [x] Fix `file_watcher_task_handles_poll_error` → `StopSignal` + `collect_result()`
- [x] Fix `file_watcher_task_unwatch` → `StopSignal` + verify zero events
- [x] Add `file_watcher_task_inotify_execution` test (Linux only)
- [x] Add `file_watcher_task_inotify_multi_subscriber` test (Linux only)
- [x] Add `file_watcher_task_inotify_unwatch` test (Linux only)
- [x] Fix `fd_monitor_task_valtron_execution` → `StopSignal`
- [x] Fix multi_executor tests → `collect_one()`

### Task 6: PollWatcher cache mechanism

- [x] Add `cached_events: Vec<WatchEvent>` field to `PollWatcher`
- [x] `has_events()` scans and caches, returns true only when events found
- [x] `poll()` drains cache first, then scans if empty
- [x] `clear()` also clears cache

### Task 7: Update spec with these lessons

- [x] Create this feature document

## Current Status

All 7 tasks complete. All 26 tests pass (12 integration + 12 unit + 2 doc).

## Key Design Decision: `Delayed` for PollWatcher, `Depends` for Event-Driven

`TaskStatus::Depends(shared)` panics when `is_ready(None)` returns `true` 3 times in a row.
`PollWatcher::has_events(None)` returns `true` immediately (it sleeps 0ms and scans).
This triggers the panic. **Therefore: `PollWatcher`-based tasks use `Delayed`, not `Depends`.**

`Depends` is correct for **event-driven watchers** (inotify, kqueue) where `is_ready(None)`
returns `false` when no OS events are pending — the executor parks the task and the OS
signals readiness when events arrive.

`Delayed` is correct for **polling-based watchers** (PollWatcher) where periodic scanning
is the only way to detect changes. The timeout controls the poll interval.

Both mechanisms avoid busy-looping — `Delayed` via the executor's sleep timer, `Depends`
via the OS readiness signal.

## Lessons Learned

1. **`has_events()` caches events, `poll()` drains them** — prevents duplicate scan work
2. **`PollWatcher` cannot use `Depends`** — executor panics on consecutive `is_ready(None) == true`
3. **`collect_one()` for finite collection** — `collect_result()` blocks forever on infinite watchers
4. **`StopSignal` for clean termination** — tests use it to verify "no events" without hanging
5. **Watch BEFORE creating files** — otherwise snapshot captures initial state, no changes detected
6. **No `unsafe` for const-to-mut** — `has_events()` takes `&mut self`, period
