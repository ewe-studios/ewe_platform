# foundation_nativeapis

Cross-platform native APIs for I/O readiness, file watching, and interprocess messaging.

## Features

| Feature | What it provides |
|---------|-----------------|
| `poll` | I/O readiness selector (epoll on Linux, kqueue on macOS/BSD, IOCP on Windows) |
| `fd` | File descriptor readiness tracking (`RegisteredFd`, `ReadyGuard`) |
| `watcher-linux` | Linux inotify file watcher |
| `watcher-macos` | macOS/BSD kqueue file watcher (EVFILT_VNODE) |
| `watcher-windows` | Windows IOCP file watcher (ReadDirectoryChangesW) |
| `uring` | io_uring layer (Linux only) |
| `task` | Valtron executor integration (`FileWatcherTask`, `FdMonitorTask`, `EventBroadcaster`) |
| `ipc` | Interprocess message bus (bincode serialization) |

## Module Structure

```
src/
├── shared/          # Always compiled — cross-platform types
│   ├── error.rs     # WatchError, Result<T>
│   ├── event.rs     # WatchEvent, WatchEventKind
│   ├── watcher/     # NativeWatcher trait, PollWatcher, SharedWatcher
│   └── api.rs       # WatcherBuilder, native_watcher()
├── native/          # Feature-gated — platform-specific APIs
│   ├── poll/        # epoll / kqueue / IOCP selector
│   ├── fd/          # RegisteredFd, ReadyGuard, edge-triggered readiness
│   ├── net/         # TcpStream, UdpSocket, UnixStream wrappers
│   └── watcher/     # InotifyWatcher, KqueueWatcher, WinWatcher
└── valtron/         # Feature-gated — executor integration
    ├── broadcaster.rs   # EventBroadcaster<T>
    ├── file_watcher.rs  # FileWatcherTask
    ├── stop_signal.rs   # StopSignal, CompositeReadiness
    └── native/          # FdMonitorTask (behind fd feature)
```

## Quick Start

### File Watching

```rust
use foundation_nativeapis::{FileWatcherTask, native_watcher};

let mut task = FileWatcherTask::new()?
    .watch("/path/to/dir", true)?;

let (tx, rx) = task.subscribe();
```

### FD Readiness

```rust
use foundation_nativeapis::native::fd::{RegisteredFd, PollResult};
use foundation_nativeapis::{Poll, Token, Interest};

let poll = Poll::new()?;
let registered = RegisteredFd::new(my_fd, poll.registry(), Token(0))?;

match registered.poll_readable() {
    PollResult::Ready(mut guard) => { /* fd is readable */ }
    PollResult::NotReady => { /* wait and retry */ }
    PollResult::Error(e) => { /* fd closed or errored */ }
}
```

## Feature Flags

```toml
[dependencies]
foundation_nativeapis = { version = "0.0.1", features = ["native-linux"] }
# Or pick individual features:
# features = ["task", "poll", "watcher-linux"]
```

Default features: `["task", "native"]` — pulls in poll, fd, and valtron tasks.

For the full platform-specific stack, use `native-linux`, `native-macos`, or `native-windows`.

## Testing

```bash
# All tests (Linux with inotify)
cargo test -p foundation_nativeapis --features task,watcher-linux

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
