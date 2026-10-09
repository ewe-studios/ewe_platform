---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F34-background-workers"
this_file: "specifications/52-tauri-foundation-platform/features/F34-background-workers/feature.md"

status: completed
priority: high
created: 2026-07-21
updated: 2026-07-21

depends_on:
  - "F01-session-backbone"
  - "F25-ipc-registry"

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# F34 — Background workers: session constructors + typed channels

## Problem

The platform has no background execution model. Mutation queue replay, cache
warming, sync, and push notification handling all need to run when the app
isn't actively rendering a page. Tauri's lifecycle (`RunEvent`, `suspend`,
`resume`) provides hooks but the platform doesn't use them.

Decision 11 defines foreground workers (unlimited time) and background-aware
workers (OS-constrained). The original design called for `#[platform_worker]`
and `#[platform_service]` proc macros, but:

1. **Tauri already owns the async runtime** — duplicating it with custom macros
   is wrong. Tauri's `async_runtime::spawn` is the right spawning primitive.
2. **src-tauri/is the right place** — users already import `foundation_platform`
   there. They wire workers in `setup_routes()`. No magic annotations needed.
3. **valtron + Tauri rt coexisting** — valtron drives our internal tasks
   (batch protocol, DOM ops). Tauri's runtime drives the app lifecycle.
   Workers can run on either.

## Solution

The platform provides TWO things: a typed channel and a session spawn method.
No proc macros. Users wire it up in `setup_routes()`:

```rust
// examples/platform_android/src-tauri/src/lib.rs (in setup_routes)

use foundation_platform::worker::{WorkerChannel, ServiceChannel};

// Worker: named, typed, bounded channel. Spawned on valtron.
let sync_channel: WorkerChannel<SyncCommand> = WorkerChannel::new("sync")
    .capacity(64)
    .build();

session.spawn_worker(sync_channel.clone(), |mut rx, session| async move {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            SyncCommand::FullSync => { /* fetch delta, update cache */ }
            SyncCommand::PushPending => { /* replay mutation queue */ }
        }
    }
});

// Dispatch from anywhere — route handlers, IPC, session events:
session.workers().send("sync", SyncCommand::FullSync);

// Service: app-lifetime, restarts on panic. Spawned on Tauri runtime.
session.spawn_service("push_listener", |session| async move {
    // Persistent WebSocket connection. Reconnects on drop. Emits to session.
    loop {
        let msg = connect_and_read().await;
        session.emit("push_received", &msg);
    }
});
```

### Channel types

```rust
// foundation_platform/src/worker.rs (NEW)

use foundation_core::valtron::channel;

/// A typed, bounded, cloneable sender for worker commands.
pub struct WorkerChannel<T: Send + 'static> {
    name: String,
    tx: channel::Sender<T>,
}

impl<T: Send + 'static> Clone for WorkerChannel<T> {
    fn clone(&self) -> Self {
        Self { name: self.name.clone(), tx: self.tx.clone() }
    }
}

impl<T: Send + 'static> WorkerChannel<T> {
    pub fn new(name: &str) -> Self { /* default capacity 64 */ }
    pub fn capacity(mut self, n: usize) -> Self { /* set capacity */ }

    /// Build the channel. Returns (channel, receiver). The receiver is
    /// consumed by `session.spawn_worker()`.
    pub fn build(self) -> (Self, WorkerReceiver<T>) { ... }

    /// Send a command. Non-blocking. Drops if channel is full.
    pub fn send(&self, cmd: T) -> bool { self.tx.try_send(cmd).is_ok() }
}

/// Typed receiver for worker commands. Consumed by the worker closure.
pub struct WorkerReceiver<T: Send + 'static> {
    rx: channel::Receiver<T>,
}

impl<T: Send + 'static> WorkerReceiver<T> {
    pub async fn recv(&mut self) -> Option<T> { self.rx.recv().await }
}
```

### Session API

```rust
impl PlatformSession {
    /// Spawn a worker on valtron's thread pool.
    /// The closure receives the receiver and a session Arc.
    pub fn spawn_worker<F, T>(&self, channel: WorkerChannel<T>, handler: F)
    where
        F: FnOnce(WorkerReceiver<T>, Arc<PlatformSession>) + Send + 'static,
        T: Send + 'static;

    /// Spawn a long-running service on Tauri's async runtime.
    /// Automatically restarts if the closure panics (max 3 restarts per minute).
    pub fn spawn_service<F>(&self, name: &str, handler: F)
    where
        F: Fn(Arc<PlatformSession>) -> Future<()> + Send + 'static;

    /// Access the worker registry to send commands by name.
    pub fn workers(&self) -> &WorkerRegistry;
}

/// Thread-safe registry of named worker channels.
pub struct WorkerRegistry {
    channels: RwLock<HashMap<String, Box<dyn Any + Send + Sync>>>,
}

impl WorkerRegistry {
    /// Send a command to a named worker. No-op if worker doesn't exist.
    pub fn send<T: Send + 'static>(&self, name: &str, cmd: T) -> bool;
}
```

### Mobile background hooks

On mobile (Android/iOS), the worker function is the SAME. Only the trigger
differs: foreground = `workers().send()`, background = OS callback.

The platform provides Tauri lifecycle hooks that bridge OS events to
worker dispatch:

```rust
// In PlatformBuilder::build() → Tauri setup:
app.on_event(|event| {
    match event {
        RunEvent::Suspend => session.workers().send("sync", SyncCommand::Suspend),
        RunEvent::Resume => session.workers().send("sync", SyncCommand::FullSync),
        RunEvent::Background => { /* limit worker throughput */ },
        _ => {}
    }
});
```

Android-specific: the platform doc documents how to register a `WorkManager`
periodic task that calls back into Rust via a Tauri command. The user writes
the Android glue in `src-tauri/src/lib.rs` using our session hooks — no
platform proc macro needed.

## Requirements

### R1. `WorkerChannel<T>` — foundation_platform/src/worker.rs (NEW)
- Builder pattern: `new(name)`, `capacity(n)`, `build() → (channel, receiver)`
- `send(cmd) -> bool` — non-blocking, drops if full
- Clone impl for multi-site dispatch
- Under the hood: valtron `channel::bounded(n)`

### R2. `WorkerReceiver<T>` — foundation_platform/src/worker.rs
- `recv() -> Option<T>` — async, returns None when channel closes
- Owned by the worker closure, not cloneable

### R3. `session.spawn_worker()` — foundation_platform/src/session.rs
- Takes a `WorkerChannel<T>` and a handler closure
- Spawns the closure on valtron's thread pool
- Handler receives `(WorkerReceiver<T>, Arc<PlatformSession>)`
- Channel auto-closes when the session drops (worker gets None → exits)

### R4. `session.spawn_service()` — foundation_platform/src/session.rs
- Takes a name and a closure returning a future
- Spawns on Tauri's async runtime (`tauri::async_runtime::spawn`)
- Auto-restart on panic (max 3/min, then logs and stops)
- Service lifecycle tied to Tauri's RunEvent::Exit

### R5. `WorkerRegistry` — foundation_platform/src/worker.rs
- `HashMap<String, Box<dyn Any>>` keyed by worker name
- `send(name, cmd)` dispatches to the named channel
- Thread-safe via RwLock
- Workers deregister on drop (channel close)

### R6. Tauri lifecycle hooks — foundation_platform/src/builder.rs
- `PlatformBuilder` wires `RunEvent::Suspend`/`Resume`/`Background`
- Dispatches to workers registered for lifecycle events
- No user-facing API change — setup_routes() just calls spawn_worker

## Verification

```bash
# Unit: channel send/recv
cargo test -p foundation_platform -- worker_channel

# Unit: worker registry
cargo test -p foundation_platform -- worker_registry

# Integration: spawn worker, send command, verify handler ran
cargo test -p foundation_platform --test walking_skeleton -- worker_spawn
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_platform/src/worker.rs` | **NEW** — WorkerChannel, WorkerReceiver, WorkerRegistry |
| `backends/foundation_platform/src/session.rs` | Add `spawn_worker()`, `spawn_service()`, `workers()` |
| `backends/foundation_platform/src/builder.rs` | Wire Tauri lifecycle events to worker dispatch |
