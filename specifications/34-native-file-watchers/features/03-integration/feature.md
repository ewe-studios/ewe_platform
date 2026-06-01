---
feature: "Integration Tests & Cross-Platform Verification"
description: "Cross-platform compilation checks, integration tests, documentation, and example usage for foundation_nativeapis"
status: "pending"
priority: "medium"
depends_on: ["01-native-apis", "02-fd-management", "04-ipc-bus", "05-valtron-watcher-task"]
estimated_effort: "small"
created: 2026-06-01
last_updated: 2026-06-01
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# Feature: Integration Tests & Cross-Platform Verification

## Problem

After implementing the native APIs and valtron tasks, we need to verify:

1. The crate compiles on all target platforms
2. The watcher actually detects file changes on the current platform
3. The FD readiness tracking works correctly with edge-triggered pollers
4. The valtron integration delivers events correctly
5. Users have working examples to reference
6. The crate has clear documentation for its public API

## Solution

Integration tests, cross-platform compilation checks, examples, and a README.

---

## Cross-Platform Compilation

### What to Verify

```bash
# Native target — full compilation (fast, 5-6 min with cranelift)
cargo check -p foundation_nativeapis

# All feature combinations on native target
cargo check -p foundation_nativeapis --features "poll"
cargo check -p foundation_nativeapis --features "watcher"
cargo check -p foundation_nativeapis --features "native"
cargo check -p foundation_nativeapis --features "uring"
cargo check -p foundation_nativeapis --features "ipc"
cargo check -p foundation_nativeapis --features "native-linux"    # Linux full stack
cargo check -p foundation_nativeapis --features "native-macos"    # macOS full stack
cargo check -p foundation_nativeapis --features "native-windows"  # Windows full stack

# wasm32 target — must compile (stubs/no-op)
cargo check -p foundation_nativeapis --target wasm32-unknown-unknown

# Cross-target compilation (if cross-compilation toolchains are installed)
cargo check -p foundation_nativeapis --target x86_64-apple-darwin
cargo check -p foundation_nativeapis --target x86_64-pc-windows-gnu
```

### Edge Cases

- **No features enabled**: `cargo check -p foundation_nativeapis --no-default-features` — only exports `WatchEvent`, `WatchEventKind`, `WatchError`, `NativeAPI` enum (no poll, no watcher, no ipc)
- **Conflicting features**: `--features "watcher-linux,watcher-macos"` on Linux — should compile both, only Linux backend used at runtime
- **IOUring on non-Linux**: Should produce a compile error, not a runtime panic

---

## Integration Tests

### 1. Watcher Integration Test (`tests/watcher_integration.rs`)

**What it tests**: End-to-end file watching on the current platform.

**How it works**:

```rust
// 1. Create temp directory
let dir = TempDir::new()?;

// 2. Create native_watcher with default settings
let mut watcher = native_watcher()?;

// 3. Watch the temp directory
watcher.watch(dir.path(), false)?;

// 4. Create a file and verify Created event
File::create(dir.path().join("test.txt"))?;
let events = watcher.poll(Duration::from_millis(500))?;
assert!(events.iter().any(|e| e.path.ends_with("test.txt") && matches!(e.kind, WatchEventKind::Created)));

// 5. Modify the file and verify Modified event
fs::write(dir.path().join("test.txt"), "hello")?;
let events = watcher.poll(Duration::from_millis(500))?;
assert!(events.iter().any(|e| e.path.ends_with("test.txt") && matches!(e.kind, WatchEventKind::Modified)));

// 6. Delete the file and verify Removed event
fs::remove_file(dir.path().join("test.txt"))?;
let events = watcher.poll(Duration::from_millis(500))?;
assert!(events.iter().any(|e| e.path.ends_with("test.txt") && matches!(e.kind, WatchEventKind::Removed)));

// 7. unwatch and verify no events
watcher.unwatch(dir.path())?;
File::create(dir.path().join("new.txt"))?;
let events = watcher.poll(Duration::from_millis(500))?;
assert!(events.is_empty());

// 8. clear and verify no events
watcher.clear()?;
File::create(dir.path().join("another.txt"))?;
let events = watcher.poll(Duration::from_millis(500))?;
assert!(events.is_empty());
```

**Edge cases**:
- **Rename detection**: Rename `a.txt` → `b.txt` → verify `Renamed { from, to }` (Linux only; other platforms best-effort)
- **Unicode filenames**: Create file with emoji/CJK characters → path correctly decoded
- **Nested directories**: Watch parent, create file in child directory → event received (Linux: need recursive walk; macOS/Windows: recursive flag)
- **Rapid changes**: Create → modify → delete within 10ms → at least one event received
- **Symlink**: Watch a symlink → events on the target
- **Empty timeout**: `poll(Duration::ZERO)` returns immediately

### 2. FD Registration Integration Test (`tests/fd_registration.rs`)

**What it tests**: Readiness tracking with edge-triggered semantics.

**How it works**:

```rust
// 1. Create a pipe (one-way communication)
let (mut reader, mut writer) = pipe()?;

// 2. Wrap reader in RegisteredFd
let registered = RegisteredFd::with_interest(reader, Interest::READABLE)?;

// 3. Before writing: NotReady
assert!(matches!(registered.poll_readable(), PollResult::NotReady));

// 4. Write data to pipe
writer.write_all(b"hello")?;

// 5. After writing: Ready
let mut guard = match registered.poll_readable() {
    PollResult::Ready(g) => g,
    other => panic!("expected Ready, got {:?}", other),
};

// 6. Read data via try_io
let result = guard.try_io(|fd| {
    let mut buf = [0u8; 5];
    fd.get_ref().read(&mut buf)
})?;
assert_eq!(result?, 5);

// 7. After reading all data: NotReady (readiness was auto-cleared by WouldBlock)
assert!(matches!(registered.poll_readable(), PollResult::NotReady));
```

**Edge cases**:
- **Spurious readiness**: fd appears ready but read returns WouldBlock → readiness cleared
- **Drop deregisters**: drop(registered) → subsequent operations on the raw fd still work (we don't close it), but no more poll events
- **EOF handling**: Close write end → poll_readable returns Ready, read returns 0 (EOF)

### 3. Poll Selector Integration Test (`tests/poll_integration.rs`)

**What it tests**: The extracted poll layer (epoll/kqueue/IOCP).

**How it works**:

```rust
// 1. Create Poll instance
let poll = Poll::new()?;

// 2. Create a pipe, register read end
let (mut reader, mut writer) = pipe()?;
poll.registry().register(&mut SourceFd(reader.as_raw_fd()), Token(7), Interest::READABLE)?;

// 3. Poll with short timeout — no events
let mut events = Events::with_capacity(16);
poll.poll(&mut events, Some(Duration::from_millis(10)))?;
assert!(events.is_empty());

// 4. Write to pipe
writer.write_all(b"hello")?;

// 5. Poll again — event returned
poll.poll(&mut events, Some(Duration::from_millis(100)))?;
assert_eq!(events.len(), 1);
let event = events.iter().next().unwrap();
assert_eq!(event.token(), Token(7));
assert!(event.is_readable());

// 6. Deregister
poll.registry().deregister(&mut SourceFd(reader.as_raw_fd()))?;
let mut events = Events::with_capacity(16);
poll.poll(&mut events, Some(Duration::from_millis(10)))?;
assert!(events.is_empty());
```

**Edge cases**:
- **Waker wake**: Create Waker, call wake() from another thread → poll unblocks
- **Multiple tokens**: Register 3 fds with different tokens → poll returns correct token for each
- **Token reuse**: Deregister fd, register new fd with same token → token now refers to new fd

### 4. Valtron Integration Test (`tests/valtron_integration.rs`)

**What it tests**: FileWatcherTask and FdMonitorTask in valtron execution engine.

**How it works**:

```rust
// 1. Create FileWatcherTask
let mut watcher = FileWatcherTask::new()?
    .watch(temp_dir.path(), false)?;

// 2. Subscribe to events
let mut rx = watcher.subscribe();

// 3. Spawn into valtron engine
let stream = execute(watcher, None)?;

// 4. Touch a file
File::create(temp_dir.path().join("trigger.txt"))?;

// 5. Collect results
let events = collect_result(stream);

// 6. Verify event received
assert!(events.iter().any(|e| e.path.ends_with("trigger.txt")));
```

---

## Examples

### 1. `examples/file_watcher.rs` — Simple file watcher

```rust
//! Watch a directory and print file changes.
//!
//! Usage: cargo run --example file_watcher -- path/to/watch

use foundation_nativeapis::{native_watcher, WatchEventKind};
use std::time::Duration;

fn main() {
    let path = std::env::args().nth(1).expect("usage: file_watcher <path>");
    let mut watcher = native_watcher().expect("failed to create watcher");
    watcher.watch(path.as_ref(), true).expect("failed to watch path");

    println!("Watching {} for changes...", path);

    loop {
        let events = watcher.poll(Duration::from_millis(100)).expect("poll failed");
        for event in events {
            match event.kind {
                WatchEventKind::Created => println!("  + {:?}", event.path),
                WatchEventKind::Modified => println!("  ~ {:?}", event.path),
                WatchEventKind::Removed => println!("  - {:?}", event.path),
                WatchEventKind::Renamed { from, to } => println!("  {} -> {}", from.display(), to.display()),
            }
        }
    }
}
```

### 2. `examples/fd_readiness.rs` — FD readiness tracking

```rust
//! Demonstrate RegisteredFd readiness tracking with a pipe.

use foundation_nativeapis::fd::{RegisteredFd, PollResult};
use foundation_nativeapis::poll::{Interest, Token};

fn main() -> std::io::Result<()> {
    // Create a pipe
    let (mut reader, mut writer) = make_pipe()?;

    // Register read end
    let registered = RegisteredFd::with_interest(reader, Interest::READABLE)?;

    // Before writing: NotReady
    println!("Before write: {:?}", registered.poll_readable());

    // Write data
    writer.write_all(b"hello")?;

    // After writing: Ready
    let mut guard = match registered.poll_readable() {
        PollResult::Ready(g) => g,
        other => panic!("expected Ready, got {:?}", other),
    };

    // Read via try_io
    guard.try_io(|fd| {
        let mut buf = [0u8; 5];
        let n = fd.get_ref().read(&mut buf)?;
        println!("Read {} bytes: {:?}", n, &buf[..n]);
        Ok(())
    })?;

    // After reading: NotReady (readiness auto-cleared)
    println!("After read: {:?}", registered.poll_readable());

    Ok(())
}
```

---

## Documentation

### README.md

```markdown
# foundation_nativeapis

Cross-platform native APIs for I/O readiness polling, file watching, and interprocess messaging.

## Features

- **poll**: I/O readiness selector (epoll/kqueue/IOCP) — extracted from mio
- **watcher**: File system event monitoring using native OS primitives
  - `watcher-linux`: inotify + epoll
  - `watcher-macos`: kqueue + EVFILT_VNODE
  - `watcher-windows`: ReadDirectoryChangesW + IOCP
- **fd**: File descriptor readiness tracking (like tokio's AsyncFd)
- **uring**: io_uring abstractions (Linux only)
- **ipc**: Interprocess message bus with typed messaging

## Quick Start

```rust
use foundation_nativeapis::{native_watcher, WatchEventKind};
use std::time::Duration;

let mut watcher = native_watcher()?;
watcher.watch("src/", true)?;

loop {
    let events = watcher.poll(Duration::from_millis(100))?;
    for event in events {
        println!("{:?}: {:?}", event.kind, event.path);
    }
}
```

## Platform Support

| Feature | Linux | macOS | Windows |
|---------|-------|-------|---------|
| poll (epoll/kqueue/IOCP) | ✓ | ✓ | ✓ |
| watcher (inotify/kqueue) | ✓ | ✓ | ✓ |
| fd (RegisteredFd) | ✓ | ✓ | ✓ |
| uring (io_uring) | ✓ | — | — |
| ipc (Unix sockets) | ✓ | ✓ (Mach ports) | ✓ (Named pipes) |
```

---

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_nativeapis/tests/watcher_integration.rs` | Create — file watcher end-to-end tests |
| `backends/foundation_nativeapis/tests/fd_registration.rs` | Create — FD readiness tracking tests |
| `backends/foundation_nativeapis/tests/poll_integration.rs` | Create — poll selector tests |
| `backends/foundation_nativeapis/tests/valtron_integration.rs` | Create — valtron task integration tests |
| `backends/foundation_nativeapis/examples/file_watcher.rs` | Create — simple file watcher example |
| `backends/foundation_nativeapis/examples/fd_readiness.rs` | Create — FD readiness example |
| `backends/foundation_nativeapis/README.md` | Create — crate documentation |

---

_Created: 2026-06-01_
