# 22 — Background sync: options, constraints, and phased approach

**Date:** 2026-07-04
**Status:** Resolved

### Decision

Background sync is designed in three tiers matching what the OS actually allows.
Foreground sync is v1 (trivial). Background fetch and push-triggered refresh are
v1 with platform caveats. Full background services are v2, documented as
platform-constrained.

### The OS reality: what mobile platforms actually allow

**iOS (BGTaskScheduler):**

| Task type | Max duration | When it runs | Use case |
|---|---|---|---|
| `BGAppRefreshTask` | ~30 seconds | System-decided schedule (opportunistic) | Fetch latest content, sync small deltas |
| `BGProcessingTask` | ~minutes (when charging, connected to WiFi) | System-decided, rare | Large sync, database cleanup, index rebuild |
| Push notification (silent) | ~30 seconds | When push received | React to server event |
| Background URLSession | Hours (discretionary) | System-managed | Large downloads (deferred to system daemon) |
| App active (foreground) | Unlimited | App is in foreground | Full sync, full processing |

**Android (WorkManager / Foreground Services):**

| Task type | Max duration | When it runs | Use case |
|---|---|---|---|
| `WorkManager` (expedited) | ~minutes | As soon as constraints met | Time-sensitive sync |
| `WorkManager` (regular) | ~10 minutes | Opportunistic (battery, network, idle) | Periodic sync, cleanup |
| `Foreground Service` | Unlimited (with notification) | Immediately, user-visible | Ongoing sync, media playback, location tracking |
| `JobScheduler` | ~10 minutes | System-decided | Legacy (WorkManager wraps this) |
| App active (foreground) | Unlimited | App is in foreground | Full sync, full processing |

**Common constraints across both platforms:**

1. The OS decides WHEN background work runs. Not the app.
2. Background execution time is limited and not guaranteed.
3. Network may be unavailable during background windows.
4. Battery and thermal state affect scheduling.
5. The app can be killed at any time during background execution.
6. Foreground services (Android) require a persistent notification.
7. iOS silent pushes can be throttled by the system.

### Tier 1: Foreground sync (v1, trivial)

The app is active and in the foreground. The shell runs a Rust async task (or
`#[platform_worker]`). No OS constraints. Full sync, full processing.

```rust
#[platform_worker]
async fn sync_worker(session: PlatformSession, mut rx: WorkerReceiver<SyncCommand>) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            SyncCommand::FullSync => {
                let server_data = session.remote().fetch("/api/sync/delta").await?;
                session.db().apply_delta(&server_data).await?;
                session.cache().invalidate_stale(server_data.stale_routes);
                session.emit("sync_complete", &SyncStatus::done());
            }
            SyncCommand::PushPending => {
                let queue = session.db().pending_mutations().await?;
                for mutation in queue {
                    session.remote().post("/api/mutations", &mutation).await?;
                }
            }
        }
    }
}
```

This covers 90% of use cases. The user is actively using the app; sync runs
concurrently. No OS background API needed.

### Tier 2: Background-aware sync (v1, platform-conditional)

The app is suspended or in the background. The shell hooks into platform-specific
background execution APIs. The sync logic is the SAME as Tier 1 — the only
difference is how it's triggered and how long it can run.

**Option A: Periodic background fetch**

```rust
// The platform registers with the OS for periodic background execution.
// This is a best-effort request — the OS decides if/when to run it.

#[platform_background(fetch, interval = "minimum")]  // iOS: BGAppRefreshTask
#[platform_background(periodic, interval_hours = 1)]  // Android: WorkManager periodic
async fn background_sync(session: PlatformSession) -> Result<()> {
    // Same logic as foreground sync, but time-limited.
    // The shell provides a deadline: session.background_deadline() returns
    // how much time is left before the OS kills this task.
    let deadline = session.background_deadline();
    let data = tokio::time::timeout(
        deadline,
        session.remote().fetch("/api/sync/delta"),
    ).await??;

    session.db().apply_delta(&data).await?;

    // If time runs out, the partial sync is committed.
    // Next background window continues from where we left off.
    Ok(())
}
```

**Option B: Push-triggered refresh**

```rust
// The server sends a silent push notification.
// The OS wakes the app briefly. The shell invokes the handler.

#[platform_background(push)]
async fn handle_push(payload: PushPayload, session: PlatformSession) -> Result<()> {
    match payload.event {
        "content_updated" => {
            session.cache().invalidate(payload.routes);
            session.db().fetch_delta(payload.sync_token).await?;
            // Update badge, schedule local notification if needed.
            session.native().set_badge(session.db().unread_count().await?);
        }
        "new_message" => {
            // Show local notification with the message content.
            session.native().show_notification(
                &payload.title,
                &payload.body,
            );
        }
        _ => log::warn!("unknown push event: {}", payload.event),
    }
    Ok(())
}
```

**Option C: Deferred downloads (iOS Background URLSession)**

For large assets (WASM updates, media, datasets), the platform hands the
download to the OS's system daemon. The OS manages the download even if the
app is killed. On completion, the app is woken briefly to process the result.

```rust
#[platform_background(download)]
async fn handle_wasm_update(session: PlatformSession) -> Result<()> {
    // Shell checks for updates. If a new WASM version is available,
    // hands the download URL to the system daemon.
    let update = session.update_service().check().await?;
    if let Some(new_version) = update.wasm_version {
        session.deferred_download(&new_version.url, |result| {
            // Called when download completes (app may be restarted).
            session.update_service().apply_wasm_update(&result.path);
        });
    }
    Ok(())
}
```

### Tier 3: Full background services (v2, platform-constrained)

Long-running background work that continues indefinitely. This requires
foreground services (Android) or is simply not possible (iOS). Designed in
detail now, implemented when needed.

**Android: Foreground Service**

The platform starts a foreground service with a persistent notification:

```rust
#[platform_service(foreground)]
async fn sync_service(session: PlatformSession) -> Result<()> {
    // Shows a persistent notification: "Syncing your data..."
    // Runs until explicitly stopped or the user dismisses the notification.
    loop {
        session.remote().sync_continuous().await?;
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}
```

On iOS, this tier is not available. The closest equivalent is a combination
of periodic fetch + push triggers + background URLSession.

### Offline mutation queue

A dedicated cross-tier concern. When the user performs an action offline:

```rust
struct MutationQueue {
    db: foundation_db::Database,
}

impl MutationQueue {
    /// Enqueue a mutation while offline. Validated locally before enqueue.
    async fn enqueue(&self, mutation: Mutation) -> Result<()> {
        mutation.validate_local()?;
        self.db.insert("mutations", &mutation).await
    }

    /// Replay all pending mutations. Called on connectivity restore.
    /// Order-preserving, idempotent (each mutation has a UUID).
    async fn replay(&self, session: &PlatformSession) -> Result<ReplayResult> {
        let pending = self.db.query("SELECT * FROM mutations ORDER BY created_at").await?;
        let mut results = Vec::new();
        for mutation in pending {
            let result = session.remote().post("/api/mutations", &mutation).await;
            match result {
                Ok(_) => {
                    self.db.delete("mutations", &mutation.id).await?;
                    results.push(ReplayStatus::Applied(mutation.id));
                }
                Err(e) if e.is_conflict() => {
                    results.push(ReplayStatus::Conflict(mutation.id, e.detail));
                }
                Err(e) => {
                    results.push(ReplayStatus::Failed(mutation.id, e.to_string()));
                    break; // stop replaying on non-retryable error
                }
            }
        }
        Ok(ReplayResult { results })
    }
}
```

The platform provides: queue storage (SQLite), replay triggers (on connectivity
change), conflict detection (version vectors or timestamps). The user defines:
what goes in the queue, how conflicts resolve, what mutations are valid offline.

### Connectivity lifecycle

The platform manages connectivity state transitions:

```
Online (active)
  │
  ├── Foreground sync runs (Tier 1)
  ├── Mutation queue drains
  ├── Cache updates from server
  │
  ▼ App goes to background
Background (suspended)
  │
  ├── Periodic fetch may run (Tier 2, OS-decided)
  ├── Push-triggered refresh may run (Tier 2)
  ├── Mutation queue: nothing (can't reach server? Might be online but backgrounded)
  │
  ▼ App goes offline
Offline
  │
  ├── No sync possible
  ├── Mutations enqueued locally
  ├── Cache serves stale content
  │
  ▼ Connectivity returns + app foregrounded
Online (reconnected)
  │
  ├── Mutation queue replays
  ├── Cache invalidates stale entries
  ├── Server streams latest state (surgical update if possible)
  └── App reflects current state
```

### What the platform provides vs what the user provides

| Layer | Platform provides | User provides |
|---|---|---|
| **Storage** | `foundation_db` (SQLite, Turso, D1). Queue table schema. | What goes in the queue. Conflict resolution logic. |
| **Sync triggers** | Connectivity change events. App lifecycle events (foreground/background). OS background execution hooks. | Sync logic (what to sync, when, how). |
| **Conflict model** | Version vector primitives. Timestamp comparison. "Last-write-wins" default. | Custom conflict resolution per mutation type. |
| **Background execution** | Registration with OS APIs (`BGTaskScheduler`, `WorkManager`). Deadline tracking. | Handler logic (same code as foreground). |
| **Push notifications** | Push registration, payload parsing, handler dispatch. | Handler logic. Notification content. |
| **Deferred downloads** | Registration with `BGURLSession` (iOS) or `DownloadManager` (Android). | Download URL. Completion handler. |
| **Cache invalidation** | Invalidation events. Stale content detection. | Which routes to invalidate. Fresh content to cache. |

### What `#[platform_worker]` provides for sync

The platform worker mode (decision 16) is the native-side execution context:

```rust
// This runs on a background thread in the native shell.
// In foreground: full Rust std::thread or tokio task.
// In background (Tier 2): limited by OS deadline.
// Same code, different execution guarantees.

#[platform_worker(sync)]
async fn main(session: PlatformSession, rx: WorkerReceiver<SyncEvent>) {
    let queue = MutationQueue::new(session.db());

    while let Ok(event) = rx.recv().await {
        match event {
            SyncEvent::Foreground => {
                // Full sync — unlimited time.
                full_sync(&session, &queue).await;
            }
            SyncEvent::BackgroundFetch(deadline) => {
                // Quick delta sync — limited by deadline.
                tokio::time::timeout(deadline, delta_sync(&session)).await;
            }
            SyncEvent::PushReceived(payload) => {
                handle_push(payload, &session).await;
            }
            SyncEvent::Online => {
                // Connectivity restored — replay queue.
                queue.replay(&session).await;
            }
        }
    }
}
```

### Why not v1 for full background services

1. iOS fundamentally limits what's possible. No long-running background processes.
2. Android foreground services require user-visible notifications — acceptable
   for music players, not for data sync in most apps.
3. The 90% use case is foreground sync + mutation queue replay. This works
   without any OS background API.
4. Periodic background fetch and push-triggered refresh cover the "update while
   not using the app" use case adequately for v1.
5. Full background services can be added without changing the sync API — the
   same `#[platform_worker]` code runs, just with different execution constraints.

### When to upgrade each tier

| Trigger | Upgrade to |
|---|---|
| Users report stale content when opening the app | Tier 2: background fetch on a periodic interval |
| Server needs to push urgent updates (messages, alerts) | Tier 2: push-triggered refresh |
| Users want continuous sync even when using other apps | Tier 3: Android foreground service |
| Large assets (WASM updates, media) need background download | Tier 2: deferred download |
| Offline mutations pile up and conflict frequently | Improve conflict resolution; Tier 2 fetch to sync more often |
