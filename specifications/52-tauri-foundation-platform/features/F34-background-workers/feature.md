---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F34-background-workers"
this_file: "specifications/52-tauri-foundation-platform/features/F34-background-workers/feature.md"

status: pending
priority: high
created: 2026-07-21

depends_on:
  - "F01-session-backbone"
  - "F25-ipc-registry"

tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# F34 — Background workers: `#[platform_worker]` + `WorkerSender`/`WorkerReceiver`

## Problem

The platform has no background execution model. Mutation queue replay, cache
warming, sync, and push notification handling all need to run when the app
isn't actively rendering a page. Tauri's lifecycle (`RunEvent::ExitRequested`,
`suspend`, `resume`) provides hooks but the platform doesn't use them for
background work.

Decision 11 defines `#[platform_worker]` and `#[platform_service]` but
neither exists as proc macros or runtime infrastructure.

## Solution

`#[platform_worker]` is a proc macro that wraps a Rust async function into
a valtron task with a typed channel. The worker receives commands via
`WorkerReceiver<T>` and sends results via `WorkerSender<T>`.

```rust
// app-worker/src/lib.rs

use foundation_platform::worker::{platform_worker, WorkerReceiver};

#[platform_worker]
async fn sync_worker(
    session: PlatformSession,
    rx: WorkerReceiver<SyncCommand>,
) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            SyncCommand::FullSync => {
                // Fetch delta from server
                let data = session.http_backend()
                    .fetch("https://api.example.com/sync/delta")
                    .unwrap();
                // Apply to local cache
                session.cache().apply_delta(&data.0);
            }
            SyncCommand::PushPending => {
                let queue = session.mutation_queue().pending();
                for mutation in queue {
                    // Replay each pending mutation
                }
            }
        }
    }
}

// In setup_routes():
session.spawn_worker(sync_worker);
```

### Proc macro expansion

```rust
// #[platform_worker] expands to:
pub fn spawn_sync_worker(
    session: Arc<PlatformSession>,
    sender: WorkerSender<SyncCommand>,
) -> JoinHandle<()> {
    let rx = sender.into_receiver();
    foundation_core::valtron::spawn(async move {
        sync_worker((*session).clone(), rx).await;
    })
}
```

### Channel types

```rust
// foundation_platform/src/worker.rs (NEW)

pub struct WorkerSender<T> { tx: valtron::Sender<T> }
pub struct WorkerReceiver<T> { rx: valtron::Receiver<T> }

impl<T> WorkerReceiver<T> {
    pub async fn recv(&mut self) -> Option<T> { self.rx.recv().await }
}

impl<T: Clone> WorkerSender<T> {
    pub fn send(&self, cmd: T) { self.tx.send(cmd).ok(); }
    pub fn sender(&self) -> Self { self.clone() }
}
```

### Background-aware execution (mobile)

On mobile (Android/iOS), the worker hooks into OS background APIs:

- **Android:** `WorkManager` for periodic sync, `ForegroundService` for
  long-running tasks. The platform registers a `WorkRequest` that calls
  back into Rust via JNI.
- **iOS:** `BGAppRefreshTask` for periodic fetch, `BGProcessingTask` for
  longer work. Registered at app launch via `BGTaskScheduler`.

The worker FUNCTION is the same on desktop and mobile. Only the TRIGGER
differs: foreground = manual `send()`, background = OS callback.

### In-process services (`#[platform_service]`)

Longer-running services that live for the app's lifetime. Useful for:
- WebSocket connections (persistent, reconnect on drop)
- SSE listeners (server-sent events → IPC emit)
- Background file watchers

```rust
#[platform_service]
async fn push_service(
    session: PlatformSession,
    rx: ServiceReceiver<PushEvent>,
) {
    // This runs for the app's entire lifetime.
    // Reconnects on disconnect, emits events to the session.
}
```

## Requirements

### R1. `#[platform_worker]` proc macro — foundation_macros
- Wraps an async fn in a spawn-able factory
- Generates `fn spawn_{name}(session, sender) -> JoinHandle`
- The worker function signature: `async fn(Session, WorkerReceiver<T>)`
- Works on all targets (desktop, Android, iOS)

### R2. `WorkerSender<T>` / `WorkerReceiver<T>` — foundation_platform
- Thin wrappers around valtron channels
- `send()` dispatches a command to the worker
- `recv()` awaits the next command (returns None when channel closes)
- Cloneable sender — multiple call sites can send commands

### R3. Foreground execution
- `session.spawn_worker(fn)` creates the channel, spawns the valtron task
- The task runs in the valtron thread pool
- Channel closes when the session drops → worker gets None → exits cleanly

### R4. Mobile background hooks
- Android: `WorkManager` periodic work request registered in Tauri setup
- iOS: `BGTaskScheduler` registered in `didFinishLaunchingWithOptions`
- Worker function is shared — same code, different trigger
- Feature-gated: `#[cfg(target_os = "android")]`, `#[cfg(target_os = "ios")]`

### R5. `#[platform_service]` proc macro — foundation_macros
- Same pattern as `#[platform_worker]` but for app-lifetime services
- Service restarts automatically if it panics (configurable retry policy)
- Service channel is unbounded (can't block the sender)

### R6. No new dependencies
- Uses existing valtron channels for message passing
- No tokio, no async-std — pure valtron
- Mobile: uses platform SDK via Tauri's existing mobile hooks

### R7. Worker registry
- `WorkerRegistry` on `PlatformSession` tracks spawned workers
- `session.workers().send("sync", SyncCommand::FullSync)` dispatches to named worker
- Workers deregister on drop

### R8. Mutation queue integration
- `MutationQueue::replay()` can be called from a worker
- Worker receives `OnlineChanged` events and triggers sync
- No UI thread blocking — queue replay runs entirely in background

## Verification

```bash
# Proc macro expansion
cargo test -p foundation_macros -- platform_worker

# Worker channel round-trip
cargo test -p foundation_platform -- worker

# Background sync: enqueue mutations, spawn worker, verify replay
cargo test -p foundation_platform --test walking_skeleton -- worker_replay

# Mobile: verify WorkManager registration compiles
cargo check -p foundation_platform --target aarch64-linux-android
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_macros/src/platform_worker.rs` | **NEW** — `#[platform_worker]` proc macro |
| `backends/foundation_platform/src/worker.rs` | **NEW** — WorkerSender, WorkerReceiver, WorkerRegistry |
| `backends/foundation_platform/src/session.rs` | Add `spawn_worker()` + `workers()` |
| `backends/foundation_platform/src/builder.rs` | Register mobile background hooks in Tauri setup |
