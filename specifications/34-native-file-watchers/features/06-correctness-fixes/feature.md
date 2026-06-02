---
feature: "Correctness Fixes — Remove Stubs, Fix Error Handling, Implement Missing Backends"
description: "Fix all places where we silently drop errors, use placeholder fallbacks, or leave stub implementations. Specifically: implement Windows IOCP readiness polling via WSAPoll, fix InotifyWatcher to return errors for unknown wd and queue overflow, fix KqueueWatcher to log unknown fd events, remove dead unimplemented!() code, and add feature flag for Windows watcher."
status: "pending"
priority: "high"
depends_on: ["01-native-apis", "02-fd-management"]
estimated_effort: "medium"
created: 2026-06-02
last_updated: 2026-06-02
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---

# Feature: Correctness Fixes — Remove Stubs, Fix Error Handling, Implement Missing Backends

## Problem

Across the codebase there are several places where we silently drop errors, use placeholder fallbacks, or leave stub implementations that will cause runtime failures or silent data loss:

| # | Location | Issue | Impact |
|---|----------|-------|--------|
| 1 | `poll/sys/windows/mod.rs` | IOCP `GetQueuedCompletionStatus` blocks forever when no overlapped I/O initiated. Returns completions only for actual I/O operations, never for readiness. | **Critical** — Windows poll() never returns |
| 2 | `watcher/linux.rs:121` | `PathBuf::from("<unknown>")` fallback when wd not in map — silently produces bogus paths instead of returning an error | **High** — caller receives events with fake paths |
| 3 | `watcher/linux.rs:138` | Queue overflow silently ignored via `continue` — some events were lost but caller never knows | **Medium** — silent data loss |
| 4 | `watcher/unix.rs:169` | `continue` when vnode event fd not in map — silently drops events without logging | Medium — silent data loss |
| 5 | `poll/sys/unix/selector/epoll.rs:48` | `unimplemented!()` dead code path — panics if reached | Medium — potential panic |
| 6 | `api.rs:97` | Non-Linux `NativeAPI::EPoll` returns `UnsupportedPlatform` instead of building kqueue watcher | Medium — macOS can't use native watcher |
| 7 | `watcher/windows.rs` | Missing entirely — no Windows file watcher backend | Medium |

## Solution

### Issue 1: Windows IOCP Selector — Use WSAPoll for Readiness

`CreateIoCompletionPort` only posts completions for overlapped I/O operations. It doesn't generate readiness events on its own. The correct approach:

- **For sockets**: Use `WSAPoll` to check readiness (POLLIN/POLLOUT/POLLERR/POLLHUP)
- **For non-socket handles**: Keep the IOCP association for overlapped I/O completions, but use WSAPoll as the primary readiness mechanism for sockets (which is the main use case)

```rust
// In Selector::poll():
// 1. Collect all registered socket handles
// 2. Build WSAPOLLFD array with desired events
// 3. Call WSAPoll(pollfds, timeout_ms)
// 4. For each fd with revents != 0, build an Event with correct flags
// 5. Return the events
```

**What WSAPoll maps to:**
- `POLLIN` → `is_readable()`
- `POLLOUT` → `is_writable()`
- `POLLERR` → `is_error()`
- `POLLHUP` → `is_read_closed()` / `is_write_closed()`

### Issue 2: InotifyWatcher — Unknown Watch Descriptor

When `decode_events()` encounters an event for a watch descriptor that's not in our `wd_to_path` map, this is a real error condition. Instead of falling back to `"<unknown>"`, return an error to the caller so they can log it and take action.

```rust
fn decode_events(...) -> (Vec<WatchEvent>, Option<WatchError>) {
    // ...
    let dir_path = match wd_to_path.get(&event.wd).cloned() {
        Some(p) => p,
        None => {
            decode_error = Some(WatchError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("inotify event for unknown watch descriptor {}", event.wd),
            )));
            continue;
        }
    };
}
```

The caller in `poll()` checks the optional error and logs it:
```rust
let (events, error) = Self::decode_events(...);
if let Some(e) = error {
    tracing::error!("InotifyWatcher decode error: {}", e);
}
Ok(events)
```

### Issue 3: InotifyWatcher — Queue Overflow

When `IN_Q_OVERFLOW` is set, inotify's buffer was full and some events were lost. This cannot be recovered — return an error to the caller.

```rust
if mask & libc::IN_Q_OVERFLOW != 0 {
    decode_error = Some(WatchError::Io(io::Error::new(
        io::ErrorKind::Other,
        "inotify queue overflow — some events may have been lost",
    )));
    continue;
}
```

### Issue 4: KqueueWatcher — Unknown FD in VNODE Events

When a vnode event arrives for an fd we don't know about, log a warning instead of silently dropping it. This shouldn't happen in normal operation — if it does, it indicates a bug or fd lifecycle issue.

```rust
let path = match self.fd_to_path.get(&fd) {
    Some(p) => p.clone(),
    None => {
        tracing::warn!("KqueueWatcher: vnode event for unknown fd {}", fd);
        continue;
    }
};
```

### Issue 5: Remove Dead `unimplemented!()` Code

The `registry()` method on the epoll `Selector` contains `unimplemented!()`. It's never called — `Poll::new()` uses `new_with_registry()` directly. Remove the dead method entirely.

### Issue 6: Non-Linux EPoll → kqueue Watcher

On macOS/BSD, `NativeAPI::EPoll` should build the `KqueueWatcher`, not return `UnsupportedPlatform`. Wire up the existing `watcher/unix.rs` module in `api.rs`:

```rust
#[cfg(all(target_os = "macos", feature = "watcher-macos"))]
NativeAPI::EPoll => {
    let w = crate::watcher::unix::KqueueWatcher::new()?;
    Ok(Box::new(w))
}
```

### Issue 7: Windows File Watcher (ReadDirectoryChangesW)

Not part of this fix — requires a separate feature. The Windows IOCP selector fix (Issue 1) is the prerequisite. A proper `WinWatcher` using `ReadDirectoryChangesW` will be implemented as a follow-up.

## Implementation Plans

### Task Breakdown

1. [ ] **Windows IOCP selector**: Replace broken `GetQueuedCompletionStatus` with `WSAPoll` for socket readiness. Map POLLIN/POLLOUT/POLLERR/POLLHUP to our Event flags. Initialize Winsock in `new_with_registry()`, call `WSACleanup()` in `Drop`.

2. [ ] **InotifyWatcher unknown wd**: Change `decode_events()` to return `(Vec<WatchEvent>, Option<WatchError>)` instead of just `Vec<WatchEvent>`. When wd not found in map, record error and skip event. Caller logs error but still returns decoded events.

3. [ ] **InotifyWatcher queue overflow**: Same pattern as #2 — record error, return to caller.

4. [ ] **KqueueWatcher unknown fd**: Replace `None => continue` with `None => { tracing::warn!(...); continue }`.

5. [ ] **Remove dead unimplemented!() code**: Delete the `registry()` method from epoll Selector.

6. [ ] **Wire up macOS kqueue watcher**: Add `NativeAPI::EPoll → KqueueWatcher::new()` in `api.rs` for macOS target.

7. [ ] **Add Windows watcher feature flag**: Add `watcher-windows` feature gate to Cargo.toml and `api.rs` placeholder.

## Trade-offs

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Windows readiness | WSAPoll instead of IOCP | WSAPoll works for readiness without overlapped I/O. IOCP only posts completions for actual I/O ops. |
| Inotify errors | Return error alongside events, don't abort | Some events were still decoded successfully — the caller should see both the events and know about the error. |
| Unknown wd/fd | Error + continue vs return immediately | Continue processing remaining events in the buffer — don't let one bad event hide the rest. |
| Winsock init/cleanup | In Selector::new() / Drop | Scoped to the selector's lifetime. Multiple selectors can coexist. |

## File Changes Summary

| File | Action |
|------|--------|
| `src/poll/sys/windows/mod.rs` | Rewrite — use WSAPoll for socket readiness polling |
| `src/watcher/linux.rs` | Change `decode_events` to return `(events, error)`, handle unknown wd and queue overflow |
| `src/watcher/unix.rs` | Log warning for unknown fd vnode events |
| `src/poll/sys/unix/selector/epoll.rs` | Remove dead `unimplemented!()` method |
| `src/api.rs` | Wire up KqueueWatcher for macOS EPoll, add Windows watcher placeholder |
| `Cargo.toml` | Add `watcher-windows` feature flag |
| `src/poll/event/windows.rs` | Add `READ_CLOSED` flag constant for POLLHUP mapping |

---

_Created: 2026-06-02_
