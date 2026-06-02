---
feature: "Correctness Fixes — Remove Stubs, Fix Error Handling, Implement Missing Backends, Fix File Structure"
description: "Fix all places where we silently drop errors, use placeholder fallbacks, leave stub implementations, or have broken file structure. Also implement missing valtron TaskIterator impls and create the net module."
status: "pending"
priority: "high"
depends_on: ["01-native-apis", "02-fd-management"]
estimated_effort: "large"
created: 2026-06-02
last_updated: 2026-06-02
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 14
  total: 14
  completion_percentage: 0%
---

# Feature: Correctness Fixes — Remove Stubs, Fix Error Handling, Implement Missing Backends

## Problem

Across the codebase there are several categories of issues:

### Category A: Silent Error Dropping

| # | Location | Issue | Impact |
|---|----------|-------|--------|
| 1 | `watcher/linux.rs:121` | `PathBuf::from("<unknown>")` fallback when wd not in map — silently produces bogus paths | **High** — caller receives events with fake paths |
| 2 | `watcher/linux.rs:138` | Queue overflow (`IN_Q_OVERFLOW`) silently ignored via `continue` | **Medium** — silent data loss |
| 3 | `watcher/unix.rs:169` | `continue` when vnode event fd not in map — silently drops events | Medium — silent data loss |

### Category B: Dead/Broken Code

| # | Location | Issue | Impact |
|---|----------|-------|--------|
| 4 | `poll/sys/unix/selector/epoll.rs:48` | `unimplemented!()` dead code path | Medium — panics if reached |
| 5 | `api.rs:97` | Non-Linux `NativeAPI::EPoll` returns `UnsupportedPlatform` | Medium — macOS can't use native watcher |
| 6 | `task/` directory | Empty directory, stale | Low — confusion |
| 7 | `task_fd.rs` (2 lines) wrapping `task_fd/` dir with one file | Pointless indirection | Low — confusing structure |

### Category C: Missing Implementations

| # | Location | Issue | Impact |
|---|----------|-------|--------|
| 8 | `src/net/` | Entire networking module missing — spec says TcpStream, TcpListener, UdpSocket, Unix sockets | **High** — can't use poll layer for networking |
| 9 | `src/watcher/windows.rs` | WinWatcher using ReadDirectoryChangesW not implemented | Medium — no Windows file watcher |
| 10 | `task.rs` | `FileWatcherTask` has `tick()` but no `impl TaskIterator` | **High** — not a valtron task, just a struct |
| 11 | `task_fd/fd_monitor.rs` | `FdMonitorTask` has `tick()` but no `impl TaskIterator` | **High** — not a valtron task, just a struct |
| 12 | `examples/file_watcher.rs` | Example deleted or never committed | Low — no demo |

### Category D: Missing Tests

| # | Location | Issue | Impact |
|---|----------|-------|--------|
| 13 | mio tests replicated | Spec says "Replicate mio's tests for selector, poll, networking" | Medium — no selector tests |
| 14 | `net_integration.rs` | Missing because net module doesn't exist | Medium — no networking tests |

## Solution

### A1-A3: Fix Silent Error Dropping

**InotifyWatcher unknown wd** — return `(events, Option<error>)` instead of `Vec<WatchEvent>`. When wd not in map, record error and skip:
```rust
let dir_path = match wd_to_path.get(&event.wd).cloned() {
    Some(p) => p,
    None => {
        decode_error = Some(WatchError::Io(io::Error::new(...)));
        continue;
    }
};
```

**Queue overflow** — same pattern, record error:
```rust
if mask & libc::IN_Q_OVERFLOW != 0 {
    decode_error = Some(WatchError::Io(io::Error::new(...)));
    continue;
}
```

**KqueueWatcher unknown fd** — log warning:
```rust
None => {
    tracing::warn!("KqueueWatcher: vnode event for unknown fd {}", fd);
    continue;
}
```

### A4-A7: Clean Up Dead/Broken Code

- Delete `unimplemented!()` method from epoll Selector
- Wire up `KqueueWatcher::new()` for macOS/BSD `NativeAPI::EPoll`
- Remove empty `task/` directory
- Flatten `task_fd.rs` + `task_fd/` — either put everything in `task.rs` or `task/fd_monitor.rs`

### C8: Implement net Module

Extract networking types from mio's sys layer:
```
src/net/
├── mod.rs          # TcpStream, TcpListener, UdpSocket re-exports
├── tcp.rs          # TcpStream + TcpListener using poll layer
├── udp.rs          # UdpSocket
└── unix.rs         # UnixStream, UnixListener, UnixDatagram
```

### C9: Implement Windows Watcher

`src/watcher/windows.rs` — ReadDirectoryChangesW + IOCP:
```rust
pub struct WinWatcher {
    iocp: Arc<Selector>,
    watches: HashMap<PathBuf, WatchState>,
}
// watch() → CreateFile + ReadDirectoryChangesW
// poll() → GetQueuedCompletionStatus + decode FILE_NOTIFY_INFORMATION
```

### C10-C11: Implement TaskIterator for FileWatcherTask and FdMonitorTask

Both need `impl TaskIterator` from foundation_core:
```rust
use foundation_core::valtron::{TaskIterator, TaskStatus, BoxedSendExecutionAction};

impl TaskIterator for FileWatcherTask {
    type Ready = WatchEvent;
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let events = self.tick();
        if !events.is_empty() {
            // Return first event as Ready, push rest to subscribers
            // ...
        }
        Some(TaskStatus::Wait(self.poll_timeout))
    }
}
```

## Implementation Plans

### Task Breakdown

1. [ ] **Fix InotifyWatcher error handling** — `decode_events` returns `(events, error)`, unknown wd → error, queue overflow → error
2. [ ] **Fix KqueueWatcher unknown fd** — `tracing::warn!` instead of silent `continue`
3. [ ] **Remove dead `unimplemented!()` code** — delete epoll `registry()` method
4. [ ] **Wire up macOS kqueue watcher** — `NativeAPI::EPoll → KqueueWatcher::new()` in api.rs
5. [ ] **Fix file structure** — remove empty `task/` dir, flatten `task_fd.rs` into `task.rs` or `task/`
6. [ ] **Implement `TaskIterator` for `FileWatcherTask`** — proper `next_status()` with `TaskStatus::Wait`
7. [ ] **Implement `TaskIterator` for `FdMonitorTask`** — proper `next_status()` with `TaskStatus::Wait`
8. [ ] **Add `task` feature flag** to Cargo.toml (depends on foundation_core for valtron types)
9. [ ] **Create `src/net/` module** — TcpStream, TcpListener, UdpSocket using poll layer
10. [ ] **Create `src/watcher/windows.rs`** — ReadDirectoryChangesW watcher
11. [ ] **Create `examples/file_watcher.rs`** — simple demo
12. [ ] **Replicate mio poll/selector tests** — register, deregister, waker, multiple tokens
13. [ ] **Add `AsFd` impl for `RegisteredFd<T>`** (when available)
14. [ ] **Add Windows `watcher-windows` feature flag** to Cargo.toml

## Trade-offs

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Inotify errors | Return error alongside events, don't abort | Some events were still decoded — caller should see both |
| Unknown wd/fd | Error + continue vs return immediately | Continue processing remaining events in the buffer |
| File structure | Flatten `task_fd.rs` into `task.rs` | No point having 2-line re-export files |
| net module | Build on top of poll layer | Reuses epoll/kqueue/IOCP, consistent with mio pattern |
| TaskIterator | Requires foundation_core dependency | `task` feature gated, only enabled when valtron integration needed |

## File Changes Summary

| File | Action |
|------|--------|
| `src/watcher/linux.rs` | Fix decode_events to return (events, error) |
| `src/watcher/unix.rs` | Log warning for unknown fd |
| `src/poll/sys/unix/selector/epoll.rs` | Remove dead unimplemented!() |
| `src/api.rs` | Wire up macOS kqueue watcher |
| `src/task.rs` | Flatten fd_monitor into task, add TaskIterator impls |
| `src/task_fd.rs` | Delete — merged into task.rs |
| `src/task_fd/fd_monitor.rs` | Delete — merged into task.rs |
| `src/task/` | Delete empty directory |
| `src/net/mod.rs` | Create — networking re-exports |
| `src/net/tcp.rs` | Create — TcpStream, TcpListener |
| `src/net/udp.rs` | Create — UdpSocket |
| `src/watcher/windows.rs` | Create — WinWatcher |
| `Cargo.toml` | Add task feature flag, watcher-windows feature |
| `examples/file_watcher.rs` | Create — demo |
| `tests/poll_integration.rs` | Expand — add mio-style tests |

---

_Created: 2026-06-02_
