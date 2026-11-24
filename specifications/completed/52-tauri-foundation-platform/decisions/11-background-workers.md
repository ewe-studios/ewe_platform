# 11 — Background workers: foreground and background execution

**Date:** 2026-07-17
**Status:** Resolved

## Decision

The platform provides `#[platform_worker]` for background processing in the
native shell. Workers run in the foreground (unlimited time) or background
(OS-constrained). The sync logic is the same — only the execution guarantees
differ. `#[platform_service]` provides longer-running in-process services where
the platform allows it (Android foreground services, desktop daemons).

This was split from [decision 05](05-offline-and-sync.md) — offline cache
tiers are a separate concern. Workers drive mutation queue replay; see
[decision 12](12-mutation-queue-and-conflict.md) for the queue itself.

## Table of Contents

1. [Foreground workers](#foreground-workers)
2. [Background-aware execution](#background-aware-execution)
3. [The OS reality: what mobile platforms actually allow](#the-os-reality-what-mobile-platforms-actually-allow)
4. [In-process services](#in-process-services)
5. [When to upgrade each tier](#when-to-upgrade-each-tier)
6. [Tauri integration](#tauri-integration)

---

## Foreground workers

The app is active and in the foreground. The shell runs a Rust async task via
`#[platform_worker]`. No OS constraints. Full sync, full processing.

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
concurrently. No OS background API needed. The shell manages the thread
lifecycle, typed channels (`WorkerSender`/`WorkerReceiver`), and shutdown
signaling.

---

## Background-aware execution

The app is suspended or in the background. The shell hooks into
platform-specific background execution APIs. The worker logic is the SAME as
foreground — the only difference is how it's triggered and how long it can run.

### Periodic background fetch

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

### Push-triggered refresh

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

### Deferred downloads (iOS Background URLSession)

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

---

## The OS reality: what mobile platforms actually allow

### iOS (BGTaskScheduler)

| Task type | Max duration | When it runs | Use case |
|---|---|---|---|
| `BGAppRefreshTask` | ~30 seconds | System-decided schedule (opportunistic) | Fetch latest content, sync small deltas |
| `BGProcessingTask` | ~minutes (when charging, WiFi) | System-decided, rare | Large sync, database cleanup |
| Push notification (silent) | ~30 seconds | When push received | React to server event |
| Background URLSession | Hours (discretionary) | System-managed | Large downloads (deferred to system daemon) |
| App active (foreground) | Unlimited | App is in foreground | Full sync, full processing |

### Android (WorkManager / Foreground Services)

| Task type | Max duration | When it runs | Use case |
|---|---|---|---|
| `WorkManager` (expedited) | ~minutes | As soon as constraints met | Time-sensitive sync |
| `WorkManager` (regular) | ~10 minutes | Opportunistic (battery, network, idle) | Periodic sync, cleanup |
| `Foreground Service` | Unlimited (with notification) | Immediately, user-visible | Ongoing sync, media playback, location tracking |
| App active (foreground) | Unlimited | App is in foreground | Full sync, full processing |

### Common constraints across both platforms

1. The OS decides WHEN background work runs. Not the app.
2. Background execution time is limited and not guaranteed.
3. Network may be unavailable during background windows.
4. Battery and thermal state affect scheduling.
5. The app can be killed at any time during background execution.
6. Foreground services (Android) require a persistent notification.
7. iOS silent pushes can be throttled by the system.

---

## In-process services

Long-running work that continues indefinitely. Requires foreground services
(Android) or is simply not possible (iOS). Designed for v2, implemented when
needed.

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
5. Full background services can be added without changing the worker API — the
   same `#[platform_worker]` code runs, just with different execution
   constraints.

---

## When to upgrade each tier

| Trigger | Upgrade to |
|---|---|
| Users report stale content when opening the app | Background fetch on periodic interval |
| Server needs to push urgent updates (messages, alerts) | Push-triggered refresh |
| Users want continuous sync even when using other apps | Android foreground service (v2) |
| Large assets (WASM updates, media) need background download | Deferred download |
| Offline mutations pile up and conflict frequently | Improve conflict resolution; background fetch to sync more often |

---

## Tauri integration

| Concern | Tauri primitive used |
|---|---|
| **Thread spawning** | `std::thread::spawn` or Tauri async task. Shell manages lifecycle. |
| **Typed channels** | `WorkerSender`/`WorkerReceiver` — crossbeam or tokio mpsc behind a platform abstraction. |
| **iOS background fetch** | Register with `BGTaskScheduler` via the shell's iOS integration layer. |
| **Android WorkManager** | Register via JNI. `#[platform_background]` generates the Kotlin glue. |
| **Push notifications** | Tauri plugin or native bridge for push registration. Payload dispatched to the annotated handler. |
| **Foreground service (Android)** | `#[platform_service(foreground)]` generates the `Service` subclass and notification channel via JNI. |
| **Shutdown signaling** | Session emits `shutdown` event → workers drain channels, flush state, exit. |
