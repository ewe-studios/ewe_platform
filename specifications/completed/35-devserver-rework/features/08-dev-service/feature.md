---
feature: "Dev Service"
description: "Replace HttpDevService with valtron-coordinated DevService — watchers/builders/runners as TaskIterators, HttpServer runs inline"
status: "complete"
priority: "high"
depends_on: ["02-task-operators", "03-native-watching", "04-cargo-builder", "05-binary-runner", "06-native-proxy", "07-sse-reload"]
estimated_effort: "medium"
created: "2026-06-01"
last_updated: "2026-06-15"
---

# Feature: Dev Service

## Problem

Current `HttpDevService` in `crates/devserver/src/builders.rs`:
- `async fn start(&mut self, canceller: broadcast::Receiver<()>) -> Result<JoinHandle<()>>`
- Sets up 5 `tokio::spawn`ed components via `ParrellelOps`
- Uses 5 `broadcast::channel`s for inter-component communication
- Returns a `JoinHandle` that the caller `.await`s

## Solution

Replace with `DevService` — valtron-coordinated watchers/builders/runners, plus `HttpServer` running inline:

```rust
pub struct DevService {
    pub project: ProjectDefinition,
}

impl DevService {
    pub fn new(project: ProjectDefinition) -> Self {
        Self { project }
    }

    /// Start the dev service. Spawns watchers/builders/runners as valtron
    /// TaskIterators, then starts HttpServer inline (blocking until shutdown).
    pub fn start(
        &self,
        engine: &dyn ExecutionEngine,
        shutdown: &Arc<OnSignal>,
    ) -> Result<()> {
        // 1. Create shared channels
        let (build_changes_tx, _) = synca::broadcast::channel(64);
        let (reload_changes_tx, _) = synca::broadcast::channel(64);
        let (build_complete_tx, _) = synca::broadcast::channel(16);

        // 2. Build HttpApp with routes + TunnelProxy
        let mut app = HttpApp::new_serve();
        app.ctx.store(build_changes_tx.clone());
        app.ctx.store(reload_changes_tx.clone());
        app.ctx.store(build_complete_tx.clone());

        // Static reloader.js
        let reloader = StaticAssetHandler::new(RELOADER_JS, "text/javascript");
        app.router().add_route_any("/static/sse/reloader.js", Arc::new(reloader));

        // SSE reload endpoint
        app.route_any::<SseReloadHandler>("/static/sse/reload");

        // Catch-all proxy forwarder
        app.route_any::<ProxyForwarder>("/");

        // Non-HTTP tunnel
        let tunnel_dest = self.project.upstream_address();
        app = app.tunnel_proxy(TunnelProxy { dest: tunnel_dest });

        // 3. Spawn valtron TaskIterators for watchers, builder, runner
        engine.schedule(Box::new(
            FileWatcherTask::new(self.project.build_directories.clone(), build_changes_tx.clone())
        ))?;

        engine.schedule(Box::new(
            FileWatcherTask::new(self.project.reload_directories.clone(), reload_changes_tx.clone())
        ))?;

        engine.schedule(Box::new(
            ProjectBuilderTask::new(build_changes_tx.subscribe(), build_complete_queue.clone())
                .builder(CargoBuilder {
                    workspace_root: self.project.workspace_root.clone(),
                    crate_name: self.project.crate_name.clone(),
                    build_args: self.project.build_arguments.clone(),
                    skip_check: self.project.skip_rust_checks,
                })
        ))?;

        engine.schedule(Box::new(
            BinaryRunnerTask::new(
                self.project.clone(),
                build_complete_tx.subscribe(),
            )
        ))?;

        // 4. Start HttpServer inline (blocks until shutdown signal)
        let server = app.server(&self.project.proxy_source_address());
        server.serve(shutdown);

        Ok(())
    }
}
```

### Communication Flow

```
FileWatcherTask (build dirs)
    ↓ broadcasts FileChange via synca broadcast
ProjectBuilderTask (subscribes to build changes)
    ↓ dispatches to matching builders → background jobs
    ↓ pushes to build_complete_queue when any build finishes
BinaryRunnerTask (pops from build_complete_queue)
    ↓ kills old binary, spawns new

HttpServer (inline, same thread pool)
    ├─ SseReloadHandler → subscribes to reload_changes broadcast
    ├─ ProxyForwarder → reads build state from ContextBag
    └─ TunnelProxy → raw copy_bidirectional to upstream
```

### Key difference from old HttpDevService

The old service spawned the proxy as a `tokio::spawn`ed task alongside the other components.
Now `HttpServer` runs **inline** after spawning the valtron tasks — its accept loop submits
each connection as a valtron TaskIterator (`ConnectionHandler`), so HTTP connections and
valtron tasks (watchers, builder, runner) share the same executor thread pool.

```
┌─────────────────────────────────────────────────┐
│ valtron executor (thread pool)                  │
│                                                 │
│  TaskIterator  TaskIterator  TaskIterator       │
│  FileWatcher   CargoBuilder  BinaryRunner       │
│                                                 │
│  TaskIterator  TaskIterator  TaskIterator       │
│  Connection 1  Connection 2  Connection 3       │
│  (HTTP)        (HTTP)        (tunnel)            │
└─────────────────────────────────────────────────┘

HttpServer.serve() runs on the calling thread,
accepting connections and submitting them to the
executor via foundation_core::valtron::send()
```

### API Compatibility

The public API should remain similar:

```rust
// Old:
let mut dev_service = HttpDevService::new(definition);
let waiter = dev_service.start(cancel_receiver).await?;
waiter.await??;

// New:
let dev_service = DevService::new(definition);
let shutdown = Arc::new(OnSignal::new());
// In a signal handler: ctrl_c.on_signal(|| shutdown.trigger());

valtron::run(|| {
    let engine = valtron::engine();
    dev_service.start(&engine, &shutdown)?;
});
```

For the CLI consumer (`bin/platform/src/local/mod.rs`), we need a convenience wrapper:

```rust
pub fn run_dev_server(project: ProjectDefinition) -> Result<()> {
    let dev_service = DevService::new(project);

    let shutdown = Arc::new(OnSignal::new());
    setup_ctrlc_handler(shutdown.clone());

    valtron::run(|| {
        let engine = valtron::engine();
        dev_service.start(&engine, &shutdown)
    })
}
```

### Task Breakdown

1. [ ] Define `DevService` struct with `start()` method
2. [ ] Implement component wiring (broadcast channels via synca)
3. [ ] Build `HttpApp` with routes + `TunnelProxy` inside `start()`
4. [ ] Spawn watcher/builder/runner TaskIterators into valtron
5. [ ] Start `HttpServer` inline after spawning
6. [ ] Write convenience `run_dev_server()` function for CLI consumers
7. [ ] Write tests: verify all components are spawned, verify HttpServer binds

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/src/service/mod.rs` | Create — DevService |

---

_Created: 2026-06-01_
