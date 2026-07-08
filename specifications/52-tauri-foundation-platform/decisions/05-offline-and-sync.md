# 05 — Offline model and background sync

**Date:** 2026-07-04
**Status:** Resolved

## Decision

Two-tier offline model plus a three-tier background sync strategy. Offline is
the default for local WASM; caching makes remote content available offline.
Background sync covers foreground (trivial), OS-mediated background fetch/push
(v1), and full background services (v2, platform-constrained).

## Table of Contents

1. [Offline model: two tiers](#offline-model-two-tiers)
2. [Cache policies: per-route control](#cache-policies-per-route-control)
3. [Offline mutation queue](#offline-mutation-queue)
4. [Background sync: three tiers](#background-sync-three-tiers)
5. [Connectivity lifecycle](#connectivity-lifecycle)
6. [What the platform provides vs what the user provides](#what-the-platform-provides-vs-what-the-user-provides)

---

## Offline model: two tiers

### Tier 1 — Local WASM execution

Because `foundation_wasm_ui` compiles to WASM, it can run on-device in
multiple configurations:

- **WebView** — WASM bundled with the app, runs locally in the WebView.
- **Native shell** — WASM loaded by the native shell, runs in-process with
  zero-copy Arrow access.
- **IPC process** — WASM in a local Rust process on the device.
- **Mobile backend service** — WASM running as an on-device background process.

In all cases, the WASM is local. There is no network round-trip. The WASM
generates responses, renders pages, handles state, processes actions — entirely
on-device. Offline is not a fallback; it's the default. This covers the full
application without any additional effort beyond deploying the WASM to the
device.

### Tier 2 — Rendered page caching (remote backends)

For when the backend runs remotely (WASM or server elsewhere):

1. The platform caches rendered pages in a local SQLite database, indexed by
   route.
2. When offline, the platform serves the cached page instantly (fast perceived
   response).
3. Once connectivity returns, the backend does whatever it needs — full page
   replacement or incremental diffing/updating of changed elements.

The platform provides APIs to store and retrieve cached rendered content by
route. The backend owns the update strategy. The platform makes both paths
possible.

Two update strategies, selected per route:

- **Full replace** — the snapshot provides instant responsiveness; when the
  server sends the latest page, the platform replaces the whole view. Best for
  content-driven screens where surgical patching adds no value.
- **Surgical update** — the client sends cached state to the backend ("user
  was on step 3 of this form with these values"), and the backend surgically
  patches only what changed. Best for preserving in-progress work.

---

## Cache policies: per-route control

The `CachePolicy` in `RouteDecision` (defined in [decision
02](02-route-policy-model.md)) controls cache behavior per route:

| Policy | Behavior | Offline behavior | Best for |
|---|---|---|---|
| `CacheFirst` | Serve from cache. Fetch on miss. | Works offline (cached content). | Static content, offline-first apps. |
| `NetworkFirst` | Try network. Fall back to cache. | Serves stale cache. | Dynamic content that should be fresh. |
| `OnlineOnly` | Never cache. Always network. | Fails with offline error. | Real-time data, auth-gated content. |
| `LocalOnly` | Always cache. Never network. | Always works (no network needed). | Bundled content. |
| `StaleWhileRevalidate` | Serve cache, refresh in background. | Serves stale cache. | Content that should feel instant. |

### Cache storage

The cache database stores responses as-encoded: if the server returned
`application/primal-columnar`, the cache stores the columnar bytes. On replay,
the cache returns the exact same bytes with the same `Content-Type`. The
runtime renders them identically — the cache is protocol-transparent.

### Backend owns state, platform owns transport

The platform provides the cache infrastructure (store, retrieve, invalidate).
The backend decides what to cache, when to invalidate, and what update
strategy to use. The platform's job is transports and caching — not state
machines, conflict resolution, or sync protocols.

---

## Offline mutation queue

When the user performs an action offline, mutations are enqueued locally in
SQLite. Each mutation has a UUID for idempotency. On connectivity restore, the
queue replays in order.

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
        let pending = self.db.query(
            "SELECT * FROM mutations ORDER BY created_at"
        ).await?;
        let mut results = Vec::new();
        for mutation in pending {
            let result = session.remote()
                .post("/api/mutations", &mutation).await;
            match result {
                Ok(_) => {
                    self.db.delete("mutations", &mutation.id).await?;
                    results.push(ReplayStatus::Applied(mutation.id));
                }
                Err(e) if e.is_conflict() => {
                    results.push(ReplayStatus::Conflict(
                        mutation.id, e.detail
                    ));
                }
                Err(e) => {
                    results.push(ReplayStatus::Failed(
                        mutation.id, e.to_string()
                    ));
                    break; // stop replay on non-retryable error
                }
            }
        }
        Ok(ReplayResult { results })
    }
}
```

The platform provides: queue storage (SQLite), replay triggers (on
connectivity change), conflict detection primitives (version vectors or
timestamps). The user defines: what goes in the queue, how conflicts resolve,
what mutations are valid offline.

---

## Background sync: three tiers

### The OS reality: what mobile platforms actually allow

**iOS (BGTaskScheduler):**

| Task type | Max duration | When it runs | Use case |
|---|---|---|---|
| `BGAppRefreshTask` | ~30 seconds | System-decided schedule (opportunistic) | Fetch latest content, sync small deltas |
| `BGProcessingTask` | ~minutes (when charging, WiFi) | System-decided, rare | Large sync, database cleanup |
| Push notification (silent) | ~30 seconds | When push received | React to server event |
| Background URLSession | Hours (discretionary) | System-managed | Large downloads (deferred to system daemon) |
| App active (foreground) | Unlimited | App is in foreground | Full sync, full processing |

**Android (WorkManager / Foreground Services):**

| Task type | Max duration | When it runs | Use case |
|---|---|---|---|
| `WorkManager` (expedited) | ~minutes | As soon as constraints met | Time-sensitive sync |
| `WorkManager` (regular) | ~10 minutes | Opportunistic (battery, network, idle) | Periodic sync, cleanup |
| `Foreground Service` | Unlimited (with notification) | Immediately, user-visible | Ongoing sync, media playback, location tracking |
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
async fn sync_worker(
    session: PlatformSession,
    mut rx: WorkerReceiver<SyncCommand>,
) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            SyncCommand::FullSync => {
                let server_data = session.remote()
                    .fetch("/api/sync/delta").await?;
                session.db().apply_delta(&server_data).await?;
                session.cache()
                    .invalidate_stale(server_data.stale_routes);
                session.emit("sync_complete", &SyncStatus::done());
            }
            SyncCommand::PushPending => {
                let queue = session.db().pending_mutations().await?;
                for mutation in queue {
                    session.remote()
                        .post("/api/mutations", &mutation).await?;
                }
            }
        }
    }
}
```

This covers 90% of use cases. The user is actively using the app; sync runs
concurrently. No OS background API needed.

### Tier 2: Background-aware sync (v1, platform-conditional)

The app is suspended or in the background. The shell hooks into
platform-specific background execution APIs. The sync logic is the SAME as
Tier 1 — the only difference is how it's triggered and how long it can run.

**Option A: Periodic background fetch**

```rust
// The platform registers with the OS for periodic background execution.
// This is a best-effort request — the OS decides if/when to run it.

#[platform_background(fetch, interval = "minimum")]  // iOS: BGAppRefreshTask
#[platform_background(periodic, interval_hours = 1)]  // Android: WorkManager
async fn background_sync(session: PlatformSession) -> Result<()> {
    let deadline = session.background_deadline();
    let data = tokio::time::timeout(
        deadline,
        session.remote().fetch("/api/sync/delta"),
    ).await??;

    session.db().apply_delta(&data).await?;
    // If time runs out, partial sync is committed.
    // Next background window continues from where we left off.
    Ok(())
}
```

**Option B: Push-triggered refresh**

```rust
#[platform_background(push)]
async fn handle_push(
    payload: PushPayload,
    session: PlatformSession,
) -> Result<()> {
    match payload.event {
        "content_updated" => {
            session.cache().invalidate(payload.routes);
            session.db().fetch_delta(payload.sync_token).await?;
            session.native()
                .set_badge(session.db().unread_count().await?);
        }
        "new_message" => {
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
    let update = session.update_service().check().await?;
    if let Some(new_version) = update.wasm_version {
        session.deferred_download(&new_version.url, |result| {
            session.update_service()
                .apply_wasm_update(&result.path);
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

```rust
#[platform_service(foreground)]
async fn sync_service(session: PlatformSession) -> Result<()> {
    // Shows a persistent notification: "Syncing your data..."
    // Runs until explicitly stopped or user dismisses notification.
    loop {
        session.remote().sync_continuous().await?;
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}
```

On iOS, this tier is not available. The closest equivalent is a combination
of periodic fetch + push triggers + background URLSession.

### Why not v1 for full background services

1. iOS fundamentally limits what's possible. No long-running background
   processes.
2. Android foreground services require user-visible notifications — acceptable
   for music players, not for data sync in most apps.
3. The 90% use case is foreground sync + mutation queue replay. This works
   without any OS background API.
4. Periodic background fetch and push-triggered refresh cover the "update while
   not using the app" use case adequately for v1.
5. Full background services can be added without changing the sync API — the
   same `#[platform_worker]` code runs, just with different execution
   constraints.

### When to upgrade each tier

| Trigger | Upgrade to |
|---|---|
| Users report stale content when opening the app | Tier 2: background fetch on periodic interval |
| Server needs to push urgent updates (messages, alerts) | Tier 2: push-triggered refresh |
| Users want continuous sync even when using other apps | Tier 3: Android foreground service |
| Large assets (WASM updates, media) need background download | Tier 2: deferred download |
| Offline mutations pile up and conflict frequently | Improve conflict resolution; Tier 2 fetch to sync more often |

---

## Connectivity lifecycle

The platform manages connectivity state transitions:

```
Online (active)
  │
  ├── Foreground sync runs (sync tier 1)
  ├── Mutation queue drains
  ├── Cache updates from server
  │
  ▼ App goes to background
Background (suspended)
  │
  ├── Periodic fetch may run (sync tier 2, OS-decided)
  ├── Push-triggered refresh may run (sync tier 2)
  ├── Mutation queue: nothing (can't reach server? Might be online
  │   but backgrounded — depends on platform)
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

### `#[platform_worker]` for sync

The platform worker mode (from [decision 04](04-deployment-surfaces.md)
entrypoints) provides the native-side execution context for sync:

```rust
#[platform_worker(sync)]
async fn main(
    session: PlatformSession,
    rx: WorkerReceiver<SyncEvent>,
) {
    let queue = MutationQueue::new(session.db());

    while let Ok(event) = rx.recv().await {
        match event {
            SyncEvent::Foreground => {
                // Full sync — unlimited time.
                full_sync(&session, &queue).await;
            }
            SyncEvent::BackgroundFetch(deadline) => {
                // Quick delta sync — limited by deadline.
                tokio::time::timeout(
                    deadline, delta_sync(&session)
                ).await;
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

Same code, different execution guarantees depending on whether the app is
foreground, background, or reconnecting.

---

## What the platform provides vs what the user provides

| Layer | Platform provides | User provides |
|---|---|---|
| **Cache storage** | SQLite database, indexed by route, profile-gated. `session.cache().get(route)`, `session.cache().invalidate(routes)`. | What to cache, when to invalidate, which routes are stale. |
| **Cache policies** | `CachePolicy` enum — `CacheFirst`, `NetworkFirst`, `OnlineOnly`, `LocalOnly`, `StaleWhileRevalidate`. Applied per-route in `RouteDecision`. | Which policy per route. |
| **Mutation queue** | Queue schema in SQLite. Replay triggers on connectivity change. Conflict detection primitives (version vectors, timestamps). | What goes in the queue. Conflict resolution logic per mutation type. "Last-write-wins" default available. |
| **Foreground sync** | `#[platform_worker]` with full execution time. Async runtime, session handle, typed channels. | Sync logic — what to sync, when, how. |
| **Background fetch** | Registration with OS APIs (`BGTaskScheduler`, `WorkManager`). Deadline tracking. Handler dispatch. | Handler logic (same code as foreground). |
| **Push notifications** | Push registration, payload parsing, handler dispatch. | Handler logic. Notification content. |
| **Deferred downloads** | Registration with `BGURLSession` (iOS) or `DownloadManager` (Android). | Download URL. Completion handler. |
| **Connectivity** | Connectivity change events. App lifecycle events (foreground/background). Online/offline detection. | React to events — replay queue, invalidate cache, refresh content. |
| **Conflict model** | Version vector primitives. Timestamp comparison. "Last-write-wins" default. | Custom conflict resolution per mutation type. |
