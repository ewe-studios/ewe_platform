---
feature: "Native File Watching APIs"
description: "foundation_nativeapis crate with NativeWatcher trait and platform-specific backends (inotify, kqueue, ReadDirectoryChangesW, poll fallback) + extracted poll/mio layer"
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "large"
created: 2026-06-01
last_updated: 2026-06-01
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 27
  total: 27
  completion_percentage: 0%
---

# Feature: Native File Watching APIs

## Problem

`crates/watchers` was a thin wrapper around `notify` + `notify-debouncer-full` with:
- Config parsing (TOML/JSON) mixed with watching logic
- Command execution bolted on
- Thread-per-watcher model with blocking mpsc channels
- No reusable abstraction — just delegated to notify
- Heavy transitive deps (crossbeam, filetime, walkdir, etc.)

Need a minimal, fast, pluggable file watcher that uses native OS primitives directly.

## Solution

A new crate `backends/foundation_nativeapis/` with:

1. **`NativeWatcher` trait** — the pluggable interface
2. **Platform backends** — one per OS, gated by `cfg`
3. **Unified `WatchEvent` type** — cross-platform event representation
4. **Poll watcher** — fallback for platforms without native support or network filesystems
5. **Extracted poll/mio layer** — epoll/kqueue/IOCP selector extracted from mio (we control it)

### Design Philosophy

- **No debouncing** — consumer decides how to coalesce events
- **No config parsing** — just watch paths and emit events
- **No internal threads** — consumer calls `poll()` when ready
- **Minimal deps** — one thin crate per platform
- **Pluggable** — new providers implement `NativeWatcher`, done

---

## Component 1: `poll` Layer — I/O Readiness Selector (extracted from mio)

### What It Does

Provides a cross-platform `Poll` type that wraps the OS readiness mechanism (epoll on Linux, kqueue on macOS/BSD, IOCP on Windows). Users register file descriptors with a `Registry`, then call `poll.poll(&mut events, timeout)` to wait for readiness. When a registered fd becomes readable or writable, the selector returns events with the associated `Token`.

This layer is extracted from mio because we need full control over:
- Token allocation and interest management
- Platform-specific extensions (EVFILT_VNODE for kqueue file watching)
- No version churn from a library we're tightly coupled to
- We can expose APIs tuned to our use cases (file watching + networking + FD registration)

### How It Works — Core Flow

```
1. Poll::new() creates a platform-specific Selector:
   Linux:   epoll_create1(EPOLL_CLOEXEC) → internal fd
   macOS:   kqueue() → internal fd
   Windows: CreateIoCompletionPort → HANDLE

2. user.registry().register(source, Token(0), Interest::READABLE)
   Linux:   epoll_ctl(EPOLL_CTL_ADD, fd, &epoll_event { events=EPOLLIN, data.u64=0 })
   macOS:   kevent(internal_fd, &EV_SET(fd, EVFILT_READ, EV_ADD, 0, 0, 0), ...)
   Windows: CreateIoCompletionPort(fd, handle, 0 as CompletionKey, 0, 0)

3. poll.poll(&mut events, Some(Duration::from_millis(100)))
   Linux:   epoll_wait(internal_fd, &mut epoll_events, timeout_ms)
   macOS:   kevent(internal_fd, &[], &mut kevents, timeout)
   Windows: GetQueuedCompletionStatus(handle, &key, &bytes, &overlapped, timeout_ms)

4. Returns Vec<Event> — each with Token + readiness flags
```

### Key Types

```rust
/// The I/O readiness poller.
pub struct Poll {
    selector: Selector,  // platform-specific: epoll_fd / kqueue_fd / iocp_handle
}

/// Registration handle — used to add/remove fd interests.
pub struct Registry {
    selector: Arc<Selector>,
}

/// Opaque token associated with a registered fd. User-chosen.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Token(pub usize);

/// What readiness to track for a registered fd.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Interest: READ | WRITE {
    // bitwise: READ = 0b01, WRITE = 0b10
}

/// Batch container for readiness events returned by poll().
pub struct Events {
    // platform-specific internal buffer
}

/// A single readiness event from poll().
pub struct Event {
    token: Token,          // which fd
    // platform-specific internal buffer
}

impl Event {
    pub fn token(&self) -> Token;

    // Readiness flags
    pub fn is_readable(&self) -> bool;
    pub fn is_writable(&self) -> bool;

    // Lifecycle flags — critical for broken pipe / EOF handling
    pub fn is_read_closed(&self) -> bool;   // EPOLLHUP / EPOLLRDHUP / EV_EOF on read filter
    pub fn is_write_closed(&self) -> bool;  // EPOLLHUP / EV_EOF on write filter
    pub fn is_error(&self) -> bool;         // EPOLLERR / EV_EOF + fflags!=0

    // Platform-specific extras
    pub fn priority(&self) -> bool;         // EPOLLPRI / SIGIO
}

impl Events {
    pub fn iter(&self) -> impl Iterator<Item = Event>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

impl Poll {
    pub fn new() -> io::Result<Self>;
    pub fn registry(&self) -> &Registry;
    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()>;
}

impl Registry {
    pub fn register(&self, source: &impl event::Source, token: Token, interest: Interest) -> io::Result<()>;
    pub fn reregister(&self, source: &impl event::Source, token: Token, interest: Interest) -> io::Result<()>;
    pub fn deregister(&self, source: &impl event::Source) -> io::Result<()>;
}
```

### `event::Source` Trait

Any type that can register/deregister itself with the selector:

```rust
pub trait Source {
    fn register(&self, registry: &Registry, token: Token, interest: Interest) -> io::Result<()>;
    fn reregister(&self, registry: &Registry, token: Token, interest: Interest) -> io::Result<()>;
    fn deregister(&self, registry: &Registry) -> io::Result<()>;
}
```

Implemented for: raw fds, TcpStream, TcpListener, UdpSocket, UnixStream, UnixListener, and any custom type.

### `SourceFd` — Register Any Raw FD

```rust
/// A wrapper that lets you register any raw file descriptor.
///
/// The fd must already be set to nonblocking mode.
/// The fd is NOT closed on drop — ownership is not transferred.
pub struct SourceFd(pub c_int);  // unix
pub struct SourceFd(pub SOCKET); // windows

impl event::Source for SourceFd { ... }
```

Usage:
```rust
let inotify_fd = inotify_init1(IN_CLOEXEC)?;
registry.register(&mut SourceFd(inotify_fd), Token(0), Interest::READABLE)?;
// Now poll() will return Token(0) events when inotify has data to read.
```

### Platform Selectors — How Each Works

#### epoll (Linux)

- `epoll_create1(EPOLL_CLOEXEC)` creates the epoll instance
- `epoll_ctl(EPOLL_CTL_ADD/MOD/DEL)` manages registrations
- `epoll_wait()` blocks until events arrive or timeout
- Uses `EPOLLET` (edge-triggered) mode — only notifies on state transitions
- Token stored in `epoll_event.data.u64` field
- `EPOLLIN` → `is_readable()`, `EPOLLOUT` → `is_writable()`
- `EPOLLHUP` → `is_read_closed()` and `is_write_closed()` (both halves closed)
- `EPOLLRDHUP` + `EPOLLIN` → `is_read_closed()` (peer shut down read side)
- `EPOLLERR` → `is_error()` (socket error condition)
- `EPOLLPRI` → `priority()` (out-of-band data)
- waker: `eventfd(EFD_CLOEXEC | EFD_NONBLOCK)` → registered with `EPOLLIN` → write to eventfd to wake the poll

#### kqueue (macOS/BSD/iOS)

- `kqueue()` creates the kernel event queue
- `kevent()` does both registration AND polling (single syscall)
- `EV_SET` macros build the `struct kevent` entries
- `EVFILT_READ` → `is_readable()`, `EVFILT_WRITE` → `is_writable()`
- `EVFILT_READ` + `EV_EOF` → `is_read_closed()` (peer closed read side)
- `EVFILT_WRITE` + `EV_EOF` → `is_write_closed()` (peer closed write side)
- `EV_EOF` + `fflags != 0` → `is_error()` (error condition)
- Also supports `EVFILT_VNODE` for file watching (see watcher section)
- Token stored in `udata` field (cast from usize to *const c_void and back)
- waker: `EVFILT_USER` with `NOTE_TRIGGER` flag

#### IOCP (Windows)

- `CreateIoCompletionPort()` associates handles with the completion port
- `GetQueuedCompletionStatus()` blocks until completions arrive or timeout
- Uses overlapped I/O — operations are submitted with `OVERLAPPED` structs
- Token stored in `lpCompletionKey` parameter
- waker: `PostQueuedCompletionStatus()` posts a fake completion

### Wakers

Each platform provides a waker to unblock `poll()` from another thread:

```rust
pub struct Waker {
    registry: Registry,
    token: Token,
}

impl Waker {
    pub fn new(registry: &Registry, token: Token) -> io::Result<Self>;
    pub fn wake(&self) -> io::Result<()>;
}
```

- **Linux**: writes 1 byte to eventfd → epoll returns readable event
- **macOS**: triggers EVFILT_USER kevent → kqueue returns event
- **Windows**: PostQueuedCompletionStatus → IOCP returns completion

### Module Structure

```
src/poll/
├── mod.rs              # Poll, Registry, Token, Interest, Events, Waker
├── event/
│   ├── mod.rs          # Event re-export per platform
│   ├── event.rs        # Event type definition
│   ├── events.rs       # Events batch container
│   └── source.rs       # event::Source trait
└── sys/
    ├── mod.rs          # cfg selection of platform module
    ├── unix/
    │   ├── mod.rs
    │   ├── selector/
    │   │   ├── epoll.rs      # Linux: epoll_create1, epoll_ctl, epoll_wait
    │   │   └── kqueue.rs     # macOS: kqueue, kevent, EV_SET
    │   ├── sourcefd.rs       # SourceFd for raw fd registration
    │   └── waker/
    │       ├── mod.rs        # cfg selection
    │       ├── eventfd.rs    # Linux: eventfd(EFD_CLOEXEC|EFD_NONBLOCK)
    │       ├── kqueue.rs     # macOS: EVFILT_USER + NOTE_TRIGGER
    │       └── pipe.rs       # BSD fallback: pipe + write
    └── windows/
        ├── mod.rs
        ├── selector.rs       # IOCP: CreateIoCompletionPort, GetQueuedCompletionStatus
        └── waker.rs          # PostQueuedCompletionStatus
```

---

## Component 2: `NativeWatcher` Trait + Platform Backends

### What It Does

Provides a unified, pluggable interface for file system event monitoring. Each platform backend uses the native OS mechanism directly — no `notify` crate, no debouncing, no config parsing. Just: register paths, poll for events, get `WatchEvent` structs.

### The Trait

```rust
/// A pluggable native file watcher for a specific platform.
pub trait NativeWatcher: Send + Sync {
    /// Add a path to watch (file or directory).
    ///
    /// For directories, inotify watches only the directory itself (not recursive).
    /// kqueue openss a fd per path and tracks that fd.
    /// ReadDirectoryChangesW supports recursive watching via a flag.
    fn watch(&mut self, path: &Path, recursive: bool) -> Result<()>;

    /// Remove a previously watched path.
    fn unwatch(&mut self, path: &Path) -> Result<()>;

    /// Poll for events with an optional timeout.
    ///
    /// Returns immediately if events are available, or blocks up to `timeout`.
    /// Returns an empty Vec on timeout (not an error).
    fn poll(&mut self, timeout: Duration) -> Result<Vec<WatchEvent>>;

    /// Remove all watches and release resources.
    fn clear(&mut self) -> Result<()>;
}
```

### `WatchEvent` and `WatchEventKind`

```rust
#[derive(Debug, Clone)]
pub struct WatchEvent {
    pub kind: WatchEventKind,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEventKind {
    Created,
    Modified,
    Removed,
    Renamed { from: PathBuf, to: PathBuf },
}
```

### Platform Backend: Linux — InotifyWatcher

#### How It Works

```
1. inotify_init1(IN_CLOEXEC) → creates inotify fd
2. Register inotify fd with our epoll Selector:
   registry.register(&mut SourceFd(inotify_fd), WATCHER_TOKEN, Interest::READABLE)
3. For each watch(path, recursive):
   a. inotify_add_watch(inotify_fd, path, IN_ALL_EVENTS) → watch_descriptor (wd)
   b. Store mapping: wd → PathBuf (needed for event resolution)
   c. If recursive: walkdir(path) and call inotify_add_watch for each subdirectory
4. poll():
   a. Call poll.poll(&mut events, Some(timeout)) on our Selector
   b. When WATCHER_TOKEN event returns (inotify fd is readable):
      read(inotify_fd, buffer) → raw bytes
      Decode buffer as sequence of inotify_event structs:
        struct inotify_event {
            int32  wd;        // watch descriptor → look up path
            uint32 mask;      // event flags
            uint32 cookie;    // rename pairing
            uint32 name_len;  // filename length
            char   name[];    // filename (relative to watched dir)
        }
      For each decoded event:
        IN_CREATE → WatchEvent { Created, dir_path + "/" + name }
        IN_MODIFY → WatchEvent { Modified, dir_path + "/" + name }
        IN_DELETE → WatchEvent { Removed, dir_path + "/" + name }
        IN_MOVED_FROM + cookie=X → remember (from_path, cookie)
        IN_MOVED_TO + cookie=X → WatchEvent { Renamed { from: remembered_path, to: current_path } }
        IN_MOVE_SELF / IN_DELETE_SELF → handle path removal
      Return Vec<WatchEvent>
   c. If timeout: return empty Vec
5. unwatch(path):
   a. Look up wd from path mapping
   b. inotify_rm_watch(inotify_fd, wd)
   c. Remove from mapping
6. clear():
   a. deregister inotify fd from Selector
   b. close(inotify_fd) → kernel removes all watches automatically
   c. Clear mapping
```

#### Important Details

- **Rename cookie matching**: `IN_MOVED_FROM` and `IN_MOVED_TO` share the same `cookie` value. We must buffer the `MOVED_FROM` event and match it when `MOVED_TO` arrives. If no matching `MOVED_TO` arrives within the same `poll()` call, emit it as `Removed`.
- **inotify watch limit**: `/proc/sys/fs/inotify/max_user_watches` defaults to 8192 on most systems. Hitting this returns `ENOSPC` from `inotify_add_watch`. Our `watch()` should return a clear error.
- **Network filesystems**: inotify doesn't work on NFS, CIFS, FUSE. `poll()` will return empty even if files change. Document this clearly — users should fall back to `PollWatcher`.
- **Nonblocking read**: inotify fd should be set to `O_NONBLOCK` so `read()` returns immediately even if no data.

#### Dependencies

- `inotify = "0.11"` — for `inotify_event` struct definitions and decoding helpers
- Our `poll::SourceFd` — to register the inotify fd with the epoll selector

### Platform Backend: macOS/BSD — KqueueWatcher

#### How It Works

```
1. KqueueWatcher owns a set of open fds — one per watched path
2. For each watch(path, recursive):
   a. open(path, O_EVTONLY | O_CLOEXEC) → file_fd
   b. Register with our kqueue Selector using EVFILT_VNODE (NOT EVFILT_READ):
      EV_SET(&kevent, file_fd, EVFILT_VNODE, EV_ADD | EV_CLEAR,
             NOTE_WRITE | NOTE_DELETE | NOTE_EXTEND | NOTE_RENAME | NOTE_REVOKE,
             0, user_data as *const c_void)
      Note: This bypasses the normal socket registration and adds a VNODE filter directly
   c. Store mapping: file_fd → PathBuf (kqueue tracks by fd, not path)
3. poll():
   a. Call poll.poll() → kevent() internally, which also picks up EVFILT_VNODE
   b. For each returned EVFILT_VNODE event:
      fflags contain:
        NOTE_WRITE   → Modified
        NOTE_DELETE  → Removed
        NOTE_EXTEND  → Modified
        NOTE_RENAME  → Removed (target path unknown — kqueue limitation)
        NOTE_REVOKE  → Removed
      Look up path from fd mapping
   c. Return Vec<WatchEvent>
4. unwatch(path):
   a. Find fd from path mapping
   b. EV_SET(DELETE) to remove the vnode filter
   c. close(fd)
   d. Remove from mapping
```

#### Important Details

- **kqueue tracks by fd, not path**: You must keep the fd open for the lifetime of the watch. If the file is deleted and recreated, the fd still points to the old inode — you won't see events on the new file.
- **EVFILT_VNODE is separate from EVFILT_READ/WRITE**: Our selector normally uses EVFILT_READ/EVFILT_WRITE for socket readiness. For file watching, we add EVFILT_VNODE filters directly. The selector's kevent() loop must handle both filter types and route them correctly (VNODE events go to the watcher, READ/WRITE events go to normal socket users).
- **Rename detection**: kqueue gives `NOTE_RENAME` but NOT the target path. We can only emit `Removed` — the consumer must handle the rename by detecting a new `Created` elsewhere.
- **Recursive watching**: kqueue can't watch recursively in one call. Must open+register each subdirectory's fd individually.
- **FSEvents not used**: macOS also has FSEvents API — it's directory-level, coalesced, and lazy. kqueue is precise and fast for our use case.

### Platform Backend: Windows — WinWatcher

#### How It Works

```
1. For each watch(path, recursive):
   a. CreateFile(path, FILE_LIST_DIRECTORY, FILE_SHARE_READ|WRITE|DELETE,
                 OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANICS | FILE_FLAG_OVERLAPPED,
                 NULL) → directory handle
   b. Allocate a buffer for ReadDirectoryChangesW output (typically 64KB)
   c. Set up OVERLAPPED struct with the buffer
   d. ReadDirectoryChangesW(handle, buffer, recursive,
        FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_DIR_NAME
        | FILE_NOTIFY_CHANGE_SIZE | FILE_NOTIFY_CHANGE_LAST_WRITE,
        &bytes_returned, &overlapped)
   e. Register handle with our IOCP selector
2. poll():
   a. GetQueuedCompletionStatus(iocp_handle, &key, &bytes, &overlapped, timeout)
   b. When completion returns for a watch handle:
      Decode FILE_NOTIFY_INFORMATION structs from the buffer:
        struct FILE_NOTIFY_INFORMATION {
            DWORD NextEntryOffset;
            DWORD Action;           // FILE_ACTION_*
            DWORD FileNameLength;   // in bytes
            WCHAR FileName[1];      // relative to watched dir
        }
      Actions:
        FILE_ACTION_ADDED     → Created
        FILE_ACTION_MODIFIED  → Modified
        FILE_ACTION_REMOVED   → Removed
        FILE_ACTION_RENAMED_OLD_NAME + next NEW_NAME → Renamed { from, to }
        FILE_ACTION_RENAMED_NEW_NAME → (handled as part of pair)
   c. Re-arm ReadDirectoryChangesW for next poll
   d. Return Vec<WatchEvent>
3. unwatch(path):
   a. CancelOverlappedIO or close handle
   b. Deregister from IOCP
   c. CloseHandle
```

#### Important Details

- **Overlapped I/O required**: ReadDirectoryChangesW must be called with an OVERLAPPED struct for IOCP integration. The buffer must outlive the overlapped operation.
- **Buffer management**: We need a per-watch buffer (64KB typical) to receive FILE_NOTIFY_INFORMATION structs. These must be re-allocated on each poll.
- **Handle ownership**: Directory handles must be kept open. Closing the handle cancels all pending notifications.
- **UTF-16 filenames**: Windows filenames come as WCHAR — must convert to Rust String.

### Platform Backend: PollWatcher (Fallback)

#### How It Works

```rust
struct PathSnapshot {
    exists: bool,
    mtime: SystemTime,
    size: u64,
    is_dir: bool,
}

struct PollWatcher {
    watches: HashMap<PathBuf, PathSnapshot>,
    recursive: HashMap<PathBuf, bool>,  // which paths to walk recursively
}

impl PollWatcher {
    fn watch(&mut self, path: &Path, recursive: bool) -> Result<()> {
        let snapshot = snapshot_path(path)?;
        self.watches.insert(path.to_path_buf(), snapshot);
        self.recursive.insert(path.to_path_buf(), recursive);
        Ok(())
    }

    fn poll(&mut self, timeout: Duration) -> Result<Vec<WatchEvent>> {
        thread::sleep(timeout);
        let mut events = Vec::new();
        let mut stale = Vec::new();

        for (path, old) in &self.watches {
            match snapshot_path(path) {
                Ok(new) => {
                    if old.mtime != new.mtime || old.size != new.size {
                        events.push(WatchEvent { kind: Modified, path: path.clone() });
                    }
                    self.watches.insert(path.clone(), new);
                }
                Err(_) if old.exists => {
                    events.push(WatchEvent { kind: Removed, path: path.clone() });
                    stale.push(path.clone());
                }
                Ok(new) if !old.exists => {
                    events.push(WatchEvent { kind: Created, path: path.clone() });
                    self.watches.insert(path.clone(), new);
                }
                _ => {}
            }
        }

        // Clean up removed paths
        for path in stale {
            self.watches.remove(&path);
            self.recursive.remove(&path);
        }

        Ok(events)
    }
}
```

#### Important Details

- **Not real-time**: Events are only detected between poll() calls. A file created and deleted between polls is invisible.
- **Directory walking**: For recursive mode, walk the entire tree on each poll — O(n) per poll where n = total files. Only use for small trees.
- **Metadata syscalls**: Each stat() is a syscall. Large trees with many files = many syscalls per poll.
- **mtime resolution**: On some filesystems (FAT32, some network fs), mtime has 2-second granularity. Rapid changes within that window are coalesced.

---

## Component 3: `WatcherBuilder` and Factory

### What It Does

Users select their preferred backend via a builder with explicit fallback chain. The builder tries each backend in order until one succeeds.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAPI {
    IOUring,    // Linux only — io_uring with IORING_OP_POLL_ADD
    EPoll,      // Linux: epoll+inotify, macOS: kqueue+EVFILT_VNODE, Windows: IOCP
    Poll,       // Stdlib metadata polling — works everywhere, slow
}
```

Platform mapping at build time:

| `NativeAPI` | Linux | macOS/BSD | Windows |
|-------------|-------|-----------|---------|
| `IOUring` | `io-uring` crate | compile error | compile error |
| `EPoll` | epoll + inotify | kqueue + EVFILT_VNODE | IOCP + ReadDirectoryChangesW |
| `Poll` | stdlib metadata | stdlib metadata | stdlib metadata |

```rust
pub struct WatcherBuilder {
    preferred: Option<NativeAPI>,
    fallbacks: Vec<NativeAPI>,
    poll_timeout: Duration,
}

impl WatcherBuilder {
    pub fn preferred(mut self, api: NativeAPI) -> Self;
    pub fn fallback(mut self, api: NativeAPI) -> Self;
    pub fn poll_timeout(mut self, timeout: Duration) -> Self;
    pub fn build(self) -> Result<Box<dyn NativeWatcher>>;
}

impl Default for WatcherBuilder {
    fn default() -> Self {
        Self {
            preferred: Some(native_default_api()),  // IOUring on Linux 5.1+, EPoll elsewhere
            fallbacks: vec![NativeAPI::Poll],
            poll_timeout: Duration::from_millis(100),
        }
    }
}

/// Create the best native watcher for the current platform.
pub fn native_watcher() -> Result<Box<dyn NativeWatcher>> {
    WatcherBuilder::default().build()
}
```

### How `build()` Works

```rust
fn build(self) -> Result<Box<dyn NativeWatcher>> {
    let mut apis = Vec::new();
    if let Some(pref) = self.preferred { apis.push(pref); }
    apis.extend(self.fallbacks);

    for api in apis {
        match try_create_watcher(api, self.poll_timeout) {
            Ok(watcher) => return Ok(watcher),
            Err(e) => tracing::warn!("Failed to create {:?} watcher: {}", api, e),
        }
    }
    Err(WatchError::AllBackendsFailed)
}
```

### Compile-Time Safety

If a user explicitly requests `NativeAPI::IOUring` on macOS, the build fails at the `#[cfg(not(target_os = "linux"))]` gate — not at runtime. The `IOUring` variant's constructor is only compiled on Linux.

---

## Testing Strategy

### What to Test

#### Unit Tests (in `src/`)

1. **WatchEvent serialization**: `WatchEvent` can be cloned, debug-printed, compared
2. **WatchEventKind matching**: `Renamed { from, to }` correctly carries both paths
3. **NativeAPI enum**: platform mapping correct per cfg, `native_default_api()` returns expected value
4. **WatcherBuilder**: fallback chain ordering, default values
5. **PollWatcher**: snapshot creation, diff detection, stale path cleanup

#### Integration Tests (in `tests/`)

6. **watcher_integration.rs**: Full end-to-end test on the current platform:
   - Create temp dir, write file → verify `Created` event
   - Append to file → verify `Modified` event
   - Delete file → verify `Removed` event
   - Rename file → verify `Renamed { from, to }` event (Linux only; other platforms best-effort)
   - unwatch() → verify no more events for that path
   - clear() → verify no events for any path

7. **poll_integration.rs**: Selector tests (replicated from mio):
   - Register fd → poll returns event when fd readable
   - Deregister fd → poll returns no events
   - Waker → wake() from another thread unblocks poll()
   - Multiple tokens → poll returns correct tokens for correct fds
   - Timeout → poll returns empty Vec after timeout

8. **net_integration.rs**: Networking tests (replicated from mio):
   - TcpListener accept → TcpStream connect
   - UdpSocket send/recv
   - UnixStream connect/accept

### Edge Cases to Test

- **inotify watch limit**: Fill up to limit → verify clear error (not panic)
- **Rename without partner**: `IN_MOVED_FROM` with no matching `IN_MOVED_TO` → emits as `Removed`
- **Rapid file churn**: create → modify → delete within one poll() → at least one event received
- **Unicode filenames**: Files with emoji, CJK, RTL characters in names → paths correctly decoded
- **Symlinks**: Watching a symlink → events on the target, not the link itself
- **Directory removal with children**: Delete a watched directory containing files → inotify sends events for children, then directory → verify all events received
- **Network filesystem**: PollWatcher used when inotify fails on NFS mount
- **Empty poll timeout**: `poll(Duration::ZERO)` returns immediately with whatever is available
- **Concurrent watches**: Multiple `NativeWatcher` instances running simultaneously → no interference
- **wasm32 target**: `cargo check --target wasm32-unknown-unknown` compiles (stubs/no-op)

### How to Test

```bash
# Unit tests (fast, no filesystem)
cargo test -p foundation_nativeapis --lib

# Integration tests (filesystem, platform-specific)
cargo test -p foundation_nativeapis --test watcher_integration
cargo test -p foundation_nativeapis --test poll_integration

# Cross-platform compilation checks
cargo check -p foundation_nativeapis                                    # native target
cargo check -p foundation_nativeapis --target x86_64-apple-darwin      # macOS (if toolchain available)
cargo check -p foundation_nativeapis --target wasm32-unknown-unknown   # wasm stubs

# Feature-gated compilation
cargo check -p foundation_nativeapis --features "poll"
cargo check -p foundation_nativeapis --features "watcher"
cargo check -p foundation_nativeapis --features "watcher-linux"
cargo check -p foundation_nativeapis --features "native"
cargo check -p foundation_nativeapis --features "native-linux"
cargo check -p foundation_nativeapis --features "uring"
```

---

## Implementation Plans

### Task Breakdown

#### 1. Extract I/O Readiness Layer (from mio)
1. [ ] Create `src/poll/mod.rs` — `Poll`, `Registry`, `Token`, `Interest`, `Events`, `Waker`
2. [ ] Create `src/poll/event/` — `Event` type alias per platform, `event::Source` trait
3. [ ] Create `src/poll/sys/unix/selector/epoll.rs` — Linux epoll selector
4. [ ] Create `src/poll/sys/unix/selector/kqueue.rs` — macOS/BSD kqueue selector
5. [ ] Create `src/poll/sys/unix/sourcefd.rs` — `SourceFd` for registering any raw fd
6. [ ] Create `src/poll/sys/unix/waker/` — eventfd (Linux), kqueue EVFILT_USER (macOS), pipe fallback
7. [ ] Create `src/poll/sys/windows/selector.rs` — Windows IOCP selector
8. [ ] Create `src/poll/sys/shell/` — fallback stubs when os-poll disabled
9. [ ] Create `src/net/` — re-export networking from sys layer (TcpStream, TcpListener, UdpSocket, Unix sockets)
10. [ ] Replicate mio's tests for selector, poll, networking

#### 2. Crate Scaffolding
11. [ ] Create `backends/foundation_nativeapis/` directory
12. [ ] Write `Cargo.toml` with platform-gated dependencies
13. [ ] Write `src/lib.rs` with module declarations, feature flags, re-exports
14. [ ] Write `src/error.rs` with `WatchError` enum
15. [ ] Write `src/event.rs` with `WatchEvent` and `WatchEventKind`
16. [ ] Write `src/api.rs` with `NativeAPI` enum and platform mapping
17. [ ] Write `src/builder.rs` with `WatcherBuilder` — preferred/fallback chain, `build()` tries each in order
18. [ ] Write `native_watcher()` factory — defaults to `WatcherBuilder::default().build()`

#### 3. Linux Backend (inotify + our epoll selector)
19. [ ] Write `src/watcher/linux/mod.rs` with `InotifyWatcher`
20. [ ] Integrate inotify fd registration with our `SourceFd` + `Interest::READABLE`
21. [ ] Decode `inotify_event` → `WatchEvent` (handle rename cookies)

#### 4. macOS/BSD Backend (kqueue + EVFILT_VNODE)
22. [ ] Write `src/watcher/unix/mod.rs` with `KqueueWatcher`
23. [ ] Register `EVFILT_VNODE` into the same kqueue fd our selector owns
24. [ ] Decode `EVFILT_VNODE` fflags → `WatchEventKind`

#### 5. Windows Backend (ReadDirectoryChangesW + our IOCP selector)
25. [ ] Write `src/watcher/windows/mod.rs` with `WinWatcher`
26. [ ] Register file notification handles with our IOCP selector
27. [ ] Decode `FILE_NOTIFY_INFORMATION` → `WatchEventKind`

#### 6. Poll Fallback
28. [ ] Write `src/watcher/poll/mod.rs` with `PollWatcher`
29. [ ] Track `HashMap<PathBuf, PathSnapshot>` — diff on each poll
30. [ ] Export as fallback in `native_watcher()` factory

## Trade-offs

| Decision | Choice | Rationale |
|----------|--------|-----------|
| mio extraction | Direct copy into our crate | Full control, no dependency churn, expose APIs for our own use cases |
| Debouncing | None | Consumer decides. Keep the library fast and simple. |
| Recursive watching | Per-directory for inotify, recursive flag for kqueue/Win | inotify doesn't support recursive natively. Consumer can walk dirs. |
| Rename detection | Cookie matching (inotify), best-effort (kqueue) | inotify gives us the target path via cookie. kqueue doesn't. |
| Async support | None initially | valtron task provides the async integration layer. |
| Fallback | PollWatcher in stdlib only | Works everywhere, no extra deps. Slow but reliable. |
| FSEvents on macOS | Not used | Directory-level, coalesced, lazy. kqueue is precise and fast. |
| Network filesystems | PollWatcher fallback | inotify/kqueue don't work on NFS/FUSE. Poll via metadata stat works everywhere. |

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_nativeapis/Cargo.toml` | Create |
| `backends/foundation_nativeapis/src/lib.rs` | Create |
| `backends/foundation_nativeapis/src/error.rs` | Create |
| `backends/foundation_nativeapis/src/event.rs` | Create |
| `backends/foundation_nativeapis/src/api.rs` | Create — `NativeAPI` enum, platform mapping, `native_default_api()` |
| `backends/foundation_nativeapis/src/builder.rs` | Create — `WatcherBuilder` with preferred/fallback chain |
| `backends/foundation_nativeapis/src/poll/` | Create — extracted from mio (Poll, Registry, Selector, Events, Token, Interest, Source) |
| `backends/foundation_nativeapis/src/poll/sys/` | Create — platform selectors (epoll, kqueue, IOCP) + shell stubs |
| `backends/foundation_nativeapis/src/poll/sys/unix/sourcefd.rs` | Create |
| `backends/foundation_nativeapis/src/poll/sys/unix/waker/` | Create — eventfd, kqueue-user, pipe |
| `backends/foundation_nativeapis/src/poll/sys/windows/` | Create — IOCP selector |
| `backends/foundation_nativeapis/src/net/` | Create — networking types (TCP, UDP, Unix sockets) |
| `backends/foundation_nativeapis/src/watcher/mod.rs` | Create — NativeWatcher trait + factory |
| `backends/foundation_nativeapis/src/watcher/linux/mod.rs` | Create — InotifyWatcher |
| `backends/foundation_nativeapis/src/watcher/unix/mod.rs` | Create — KqueueWatcher |
| `backends/foundation_nativeapis/src/watcher/windows/mod.rs` | Create — WinWatcher |
| `backends/foundation_nativeapis/src/watcher/poll/mod.rs` | Create — PollWatcher fallback |
| `backends/foundation_nativeapis/tests/poll_integration.rs` | Create — selector tests |
| `backends/foundation_nativeapis/tests/net_integration.rs` | Create — networking tests |
| `backends/foundation_nativeapis/tests/watcher_integration.rs` | Create — watcher end-to-end tests |
| Root `Cargo.toml` | Edit — add `foundation_nativeapis` to workspace members (already done) |

---

_Created: 2026-06-01_
