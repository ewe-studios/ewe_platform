---
feature: "Dev Service"
description: "Replace HttpDevService with valtron-coordinated DevService — spawn all components as child tasks through ExecutionEngine"
status: "pending"
priority: "high"
depends_on: ["02-task-operators", "03-native-watching", "04-cargo-builder", "05-binary-runner", "06-native-proxy", "07-sse-reload"]
estimated_effort: "medium"
created: 2026-06-01
last_updated: 2026-06-01
---

# Feature: Dev Service

## Problem

Current `HttpDevService` in `crates/devserver/src/builders.rs`:
- `async fn start(&mut self, canceller: broadcast::Receiver<()>) -> Result<JoinHandle<()>>`
- Sets up 5 `tokio::spawn`ed components via `ParrellelOps`
- Uses 5 `broadcast::channel`s for inter-component communication
- Returns a `JoinHandle` that the caller `.await`s

## Solution

Replace with `DevService` — a valtron-coordinated service:

```rust
pub struct DevService {
    pub project: ProjectDefinition,
}

impl DevService {
    pub fn new(project: ProjectDefinition) -> Self {
        Self { project }
    }

    /// Start the dev service by spawning all components into valtron.
    /// Returns the task entry IDs for monitoring.
    pub fn start(
        &self,
        engine: &dyn ExecutionEngine,
    ) -> Result<DevServiceHandles> {
        // 1. Create shared channels/queues
        let (build_changes_tx, build_changes_rx) = ...;
        let (reload_changes_tx, reload_changes_rx) = ...;
        let (build_complete_tx, build_complete_rx) = ...;
        let (package_started_tx, package_started_rx) = ...;

        // 2. Register SSE routes
        let proxy_type = self.configure_proxy_routes(&reload_changes_tx);

        // 3. Spawn all component tasks into valtron
        let builder_watcher = engine.schedule(Box::new(
            FileWatcherTask::new(self.project.build_directories.clone(), build_changes_tx)
                .into_execution_iterator()
        ))?;

        let reloader_watcher = engine.schedule(Box::new(
            FileWatcherTask::new(self.project.reload_directories.clone(), reload_changes_tx)
                .into_execution_iterator()
        ))?;

        let cargo_builder = engine.schedule(Box::new(
            CargoBuilderTask::new(..., build_changes_rx, build_complete_tx)
                .into_execution_iterator()
        ))?;

        let binary_runner = engine.schedule(Box::new(
            BinaryRunnerTask::new(..., build_complete_rx, package_started_tx)
                .into_execution_iterator()
        ))?;

        let proxy = engine.schedule(Box::new(
            ProxyTask::new(proxy_type)
                .into_execution_iterator()
        ))?;

        Ok(DevServiceHandles {
            builder_watcher,
            reloader_watcher,
            cargo_builder,
            binary_runner,
            proxy,
        })
    }
}
```

### Communication Flow

```
FileWatcherTask (build dirs)
    ↓ pushes FileChange to queue
CargoBuilderTask
    ↓ sets build_complete_flag
BinaryRunnerTask
    ↓ pushes to started_queue
ProxyTask (reads started_queue for SSE reload signals)
    ↓ serves HTTP to browser
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
let handles = dev_service.start(&engine)?;
// engine runs; handles provide entry IDs for monitoring
```

For the CLI consumer (`bin/platform/src/local/mod.rs`), we need a convenience wrapper:

```rust
pub fn run_dev_server(project: ProjectDefinition) -> Result<()> {
    let dev_service = DevService::new(project);
    // Set up valtron engine, spawn tasks, run until cancel
    let mut engine = SingleThreadedEngine::new(Config::default())?;
    let _handles = dev_service.start(&engine)?;
    engine.run()?;
    Ok(())
}
```

### Task Breakdown

1. [ ] Define `DevService` struct with `start()` method
2. [ ] Implement component wiring (queues, flags, channels)
3. [ ] Implement proxy route configuration (SSE routes)
4. [ ] Implement `DevServiceHandles` for task monitoring
5. [ ] Write convenience `run_dev_server()` function for CLI consumers
6. [ ] Write tests: verify all 5 components are spawned, verify queue wiring

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/src/service/mod.rs` | Create — DevService |

---

_Created: 2026-06-01_
