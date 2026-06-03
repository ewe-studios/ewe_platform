# foundation_nativeapis

Cross-platform native APIs for I/O readiness, file watching, and interprocess messaging.

## Features

| Feature | What it provides |
|---------|-----------------|
| `poll` | I/O readiness selector (epoll on Linux, kqueue on macOS/BSD, IOCP on Windows) |
| `fd` | File descriptor readiness tracking (`RegisteredFd`, `ReadyGuard`, edge-triggered) |
| `watcher-linux` | Linux inotify file watcher |
| `watcher-macos` | macOS/BSD kqueue file watcher (EVFILT_VNODE) |
| `watcher-windows` | Windows IOCP file watcher (ReadDirectoryChangesW) |
| `uring` | io_uring layer (Linux only) |
| `ipc` | Interprocess message bus (bincode serialization) |

Note: valtron types (`FileWatcherTask`, `FdMonitorTask`, `EventBroadcaster`, `StopSignal`, `FdState`) are always available — no feature flag needed. Only `FdMonitorTask` requires the `fd` feature since it depends on `native::fd::RegisteredFd`.

## Module Structure

```
src/
├── shared/          # Always compiled — cross-platform types
│   ├── error.rs     # WatchError, Result<T>
│   ├── event.rs     # WatchEvent, WatchEventKind
│   ├── fd_state.rs  # FdState enum (Readable, Writable, Both)
│   ├── watcher/     # NativeWatcher trait, PollWatcher, SharedWatcher
│   └── api.rs       # WatcherBuilder, native_watcher()
├── native/          # Feature-gated — platform-specific APIs
│   ├── poll/        # epoll / kqueue / IOCP selector
│   ├── fd/          # RegisteredFd, ReadyGuard, edge-triggered readiness
│   ├── net/         # TcpStream, UdpSocket, UnixStream wrappers
│   └── watcher/     # InotifyWatcher, KqueueWatcher, WinWatcher
└── valtron/         # Always available — executor integration
    ├── broadcaster.rs   # Broadcaster<T> (re-exported from foundation_core)
    ├── file_watcher.rs  # FileWatcherTask, FileWatcherBuilder, WatchEventStream
    ├── stop_signal.rs   # StopSignal, CompositeReadiness
    └── native/          # Feature-gated (fd)
        └── fd_monitor.rs  # FdMonitorTask
```

## Quick Start

### File Watching (Simple)

The builder pattern creates a task, spawns it into valtron, and returns a stream:

```rust
use foundation_nativeapis::valtron::FileWatcherBuilder;

let stream = FileWatcherBuilder::new()
    .watch("/path/to/dir", true)?
    .build()?;

for event in stream {
    println!("File changed: {:?}", event.path);
}
```

### File Watching (Manual Control)

For fine-grained control, create the task and drive it yourself:

```rust
use foundation_core::valtron::{TaskIterator, execute};
use foundation_nativeapis::{FileWatcherTask, PollWatcher};

let mut task = FileWatcherTask::with_watcher(Box::new({
    let mut w = PollWatcher::new();
    w.watch("src/", false)?;
    w
}));

// Subscribe to receive events
let rx = task.subscribe();

// Drive via valtron executor
let stream = execute(task, None)?;
for item in stream {
    // Handle Stream::Next(event), Stream::Pending, etc.
}
```

### FD Readiness

```rust
use foundation_nativeapis::native::fd::{RegisteredFd, PollResult};
use foundation_nativeapis::{Poll, Token, Interest};

let poll = Poll::new()?;
let registered = RegisteredFd::new(my_fd, &poll.registry(), Token(0))?;

match registered.poll_readable() {
    PollResult::Ready(mut guard) => { /* fd is readable */ }
    PollResult::NotReady => { /* wait and retry */ }
    PollResult::Error(e) => { /* fd closed or errored */ }
}
```

### FD Monitor Task

```rust
use foundation_nativeapis::native::fd::RegisteredFd;
use foundation_nativeapis::FdMonitorTask;
use foundation_nativeapis::{Poll, Token};

let poll = Poll::new()?;
let registered = RegisteredFd::new(my_fd, &poll.registry(), Token(0))?;

let task = FdMonitorTask::new(registered)
    .with_callback(|fd| {
        // Handle readiness
        Ok(())
    });
```

## Feature Flags

```toml
[dependencies]
# Base native layer (poll + fd):
foundation_nativeapis = "0.0.1"

# Add platform-specific watcher:
foundation_nativeapis = { version = "0.0.1", features = ["native-linux"] }

# Individual features:
foundation_nativeapis = { version = "0.0.1", features = ["poll", "watcher-linux"] }
```

Default features: `["native"]` — pulls in poll + fd. Platform-specific watchers are opt-in.

For the full platform-specific stack, use `native-linux`, `native-macos`, or `native-windows`.

## Testing

```bash
# All tests (Linux with inotify)
cargo test -p foundation_nativeapis --features watcher-linux

# Just poll tests
cargo test -p foundation_nativeapis --features poll --test poll_integration

# Just watcher tests
cargo test -p foundation_nativeapis --features watcher-linux --test watcher_integration
```

## Platform Support

| Platform | Poll | FD | File Watcher |
|----------|------|----|-------------|
| Linux | epoll ✓ | ✓ | inotify ✓ |
| macOS | kqueue ✓ | ✓ | kqueue EVFILT_VNODE ✓ |
| BSD | kqueue ✓ | ✓ | kqueue EVFILT_VNODE ✓ |
| Windows | IOCP ✓ | - | ReadDirectoryChangesW ✓ |
| WASM | shell stub | - | PollWatcher ✓ |
