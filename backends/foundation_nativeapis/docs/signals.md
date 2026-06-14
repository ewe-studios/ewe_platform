# Signal Handling

Cross-platform OS signal reception (SIGINT, SIGTERM, SIGHUP, SIGQUIT) as a
valtron `TaskIterator`. Zero CPU spinning — parks on epoll (Linux), kqueue
(macOS), or WaitForSingleObject (Windows) and wakes only when a signal arrives.

## Quick start

```rust
use foundation_nativeapis::signal::signal_task;
use foundation_core::valtron::{valtron, execute, TaskIterator};

#[valtron]
fn main() {
    let (signal_task, bus) = signal_task().unwrap();

    // Schedule the signal task into valtron — the engine drives it,
    // you never call next_status() yourself.
    let mut stream = execute(signal_task, None).unwrap();

    // Collect signals until one arrives (blocking at the surface point).
    // In a real app you'd compose this with other tasks, not block.
    for signal in &mut stream {
        if let foundation_core::valtron::Stream::Next(event) = signal {
            println!("Received: {}", event.kind);
            break;
        }
    }
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  signal_task()                                               │
│  ├─ SignalBus — fan-out to subscribers (ConcurrentQueue)     │
│  ├─ SignalHandle — epoll/kqueue/WaitForSingleObject          │
│  └─ SignalTask — TaskIterator with Depends(EventReadiness)   │
│                                                              │
│  OS signal ──► handler writes eventfd/set event              │
│           ──► epoll/kqueue wakes                             │
│           ──► SignalTask yields Ready(SignalEvent)           │
│           ──► SignalBus delivers to all subscribers           │
└─────────────────────────────────────────────────────────────┘
```

## Types

### `SignalKind`

```rust
pub enum SignalKind {
    Interrupt,   // Ctrl+C (SIGINT / CTRL_C_EVENT)
    Terminate,   // SIGTERM / CTRL_CLOSE_EVENT
    Hangup,      // SIGHUP (Unix only) — typically means "reload config"
    Quit,        // SIGQUIT (Unix only) — core dump
}
```

### `SignalEvent`

```rust
pub struct SignalEvent {
    pub kind: SignalKind,
    pub timestamp: Instant,
}
```

### `SignalBus`

Fan-out hub. Each `subscribe()` call returns an `Arc<ConcurrentQueue<SignalEvent>>`
that receives every signal event. Multiple subscribers are supported.

```rust
let (task, bus) = signal_task().unwrap();
let subscriber = bus.subscribe();

// In another task (or the same one via execute):
if let Ok(event) = subscriber.pop() {
    match event.kind {
        SignalKind::Hangup => reload_config(),
        _ => {},
    }
}
```

### `SignalTask`

Implements `TaskIterator`. Returns `TaskStatus::Depends(handle)` when idle
(parked on the OS wait primitive) and `TaskStatus::Ready(SignalEvent)` when
a signal arrives.

**You never drive this manually.** Use `execute()`, `send()`, or `sync_one()`
through the valtron unified API.

## Usage patterns

### Graceful shutdown on Ctrl+C

```rust
use foundation_nativeapis::signal::{signal_task, SignalKind};
use foundation_core::valtron::{valtron, execute, sync_one, TaskIterator, Stream};

#[valtron]
fn main() {
    let (sig_task, _bus) = signal_task().unwrap();
    let mut stream = execute(sig_task, None).unwrap();

    for item in &mut stream {
        if let Stream::Next(event) = item {
            match event.kind {
                SignalKind::Interrupt | SignalKind::Terminate => {
                    println!("Shutting down...");
                    break;
                }
                SignalKind::Hangup => {
                    println!("Reloading config...");
                }
                _ => {}
            }
        }
    }
}
```

### Signal + other tasks (composed)

```rust
use foundation_core::valtron::{valtron, execute, execute_collect_all};

#[valtron]
fn main() {
    let (sig_task, bus) = signal_task().unwrap();

    // Your other tasks...
    let watcher_task = FileWatcherTask::new().unwrap();
    let builder_task = MyBuilderTask::new();

    // Execute all concurrently — valtron interleaves them.
    // The signal task parks on epoll; the others do their work.
    // When a signal arrives, valtron wakes the signal task and
    // delivers the event through the stream.
    let sig_stream = execute(sig_task, None).unwrap();
    // ... compose with other streams
}
```

### Multiple subscribers (signal bus)

```rust
let (sig_task, bus) = signal_task().unwrap();

// Subscriber 1: shutdown handler
let shutdown_sub = bus.subscribe();

// Subscriber 2: config reload handler
let reload_sub = bus.subscribe();

// Each subscriber gets its own queue — no contention.
// The signal handler (OS-level) writes once, the bus fans out to all.
```

## Platform details

### Linux

Uses `eventfd` + `sigaction` + `epoll`:
1. `eventfd(0, EFD_NONBLOCK)` creates a non-blocking event fd
2. `sigaction` installs handlers for SIGINT/SIGTERM/SIGHUP/SIGQUIT
3. Handler writes `1` to eventfd (single syscall, async-signal-safe)
4. `epoll_wait` blocks until eventfd is readable → wakes the valtron task

### macOS

Uses `kqueue` with `EVFILT_SIGNAL`:
1. `kqueue()` creates a kernel event queue
2. `kevent` registers each signal with `EVFILT_SIGNAL` filter
3. `kevent` blocks until any registered signal fires

### Windows

Uses `SetConsoleCtrlHandler` + event object:
1. `CreateEventW` creates an auto-reset event
2. `SetConsoleCtrlHandler` installs a console handler
3. Handler calls `SetEvent` on Ctrl+C, close, logoff, shutdown
4. `WaitForSingleObject` blocks until the event is signaled

## Feature flags

```toml
# Default: signal handling is included
foundation_nativeapis = { workspace = true }

# Only signal handling, no watcher/ipc/vfs:
foundation_nativeapis = { workspace = true, default-features = false, features = ["signal"] }
```

The `signal` feature depends on `poll` (for the epoll/kqueue infrastructure).

## Important notes

1. **Never call `next_status()` directly.** The valtron engine drives tasks.
   Use `execute()`, `send()`, `sync_one()`, or the stream iterators returned
   by `execute()`.

2. **Signal handlers are async-signal-safe.** The OS handler only writes to
   eventfd / sets an event. All fan-out to subscribers happens on the valtron
   thread, not in the signal handler.

3. **Only one `signal_task()` per process.** The global bus is set once.
   Calling `signal_task()` multiple times will re-register handlers (which
   is safe on most platforms but wastes the previous registration).

4. **SIGHUP/SIGQUIT are Unix-only.** On Windows, only Interrupt (Ctrl+C)
   and Terminate (close/logoff/shutdown) are available.
