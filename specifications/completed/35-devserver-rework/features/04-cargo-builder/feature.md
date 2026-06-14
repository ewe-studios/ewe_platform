---
feature: "Project Builder"
description: "Pluggable ProjectBuilder trait — multiple builders per task, each decides via should_build(FileChange) whether to run, submitted as Arc<dyn ProjectBuilder> to BackgroundJobRegistry for natural backpressure"
status: "complete"
priority: "high"
depends_on: ["02-task-operators", "03-native-watching"]
estimated_effort: "medium"
created: "2026-06-01"
last_updated: "2026-06-15"
---

# Feature: Project Builder

## Problem

Current `CargoShellBuilder` in `crates/devserver/src/builders.rs`:
- Hardcoded to Rust/cargo only
- Uses `tokio::process::Command` for `cargo check` and `cargo build`
- Single builder — can't handle WASM, JS, TypeScript, or other build targets
- Listens on `tokio::sync::broadcast::Receiver` for file change events
- `tokio::spawn` loops indefinitely, spawning unbounded threads

## Solution

Replace with a **pluggable builder system** — any number of `ProjectBuilder` implementations, each deciding whether a file change is relevant to them:

```rust
/// A build target that decides whether it should run for a given file change.
pub trait ProjectBuilder: Send + Sync + std::fmt::Debug {
    /// Returns the builder's human-readable name (e.g. "cargo-wasm", "esbuild").
    fn name(&self) -> &str;

    /// Decide whether this file change should trigger a build for this builder.
    ///
    /// Examples:
    /// - CargoBuilder: returns true for FileChange::Rust(path)
    /// - WasmBuilder:  returns true for FileChange::Rust in a crate with
    ///                 `crate-type = ["cdylib"]`
    /// - Esbuild:      returns true for FileChange::Javascript/Typescript
    fn should_build(&self, change: &FileChange) -> bool;

    /// Run the build. Called by background job — blocking is fine.
    fn build(&self, change: &FileChange) -> BuildResult;
}
```

### ProjectBuilderTask — manages multiple builders

```rust
pub struct ProjectBuilderTask {
    /// All registered builders — iterated on each file change.
    builders: Vec<Arc<dyn ProjectBuilder>>,
    /// File change events from FileWatcherTask.
    change_rx: mpp::Receiver<FileChange>,
    change_ready: QueueReadiness<FileChange>,
    /// Build results from background jobs.
    result_queue: Arc<ConcurrentQueue<BuildResult>>,
    result_ready: QueueReadiness<BuildResult>,
    /// Signal BinaryRunnerTask when any build completes.
    build_complete_queue: Arc<ConcurrentQueue<()>>,
}

impl ProjectBuilderTask {
    pub fn builder(mut self, b: impl ProjectBuilder + 'static) -> Self {
        self.builders.push(Arc::new(b));
        self
    }

    pub fn build(self) -> GenericResult<DrivenStreamIterator<Self>> {
        execute(self, None)
    }
}

impl TaskIterator for ProjectBuilderTask {
    type Ready = BuildResult;
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // 1. Drain file changes — dispatch to matching builders
        while let Ok(change) = self.change_rx.try_recv() {
            for builder in &self.builders {
                if builder.should_build(&change) {
                    self.submit_build(builder.clone(), change);
                }
            }
        }

        // 2. Drain build results (fire-and-forget: log, signal runner)
        while let Ok(result) = self.result_queue.try_pop() {
            match &result {
                Ok(output) => tracing::info!("build complete: {output}"),
                Err(e) => {
                    tracing::error!("build failed: {e}");
                    // stop_on_failure handled at DevService level
                }
            }
            let _ = self.build_complete_queue.push(());
        }

        // 3. Park until file changes or build results arrive
        Some(TaskStatus::Depends(Arc::new(CompositeReadiness::new(
            Arc::new(self.change_ready.clone()),
            Arc::new(self.result_ready.clone()),
        ))))
    }
}

impl ProjectBuilderTask {
    /// Submit a build to BackgroundJobRegistry.
    ///
    /// The builder itself (Arc<dyn ProjectBuilder>) is sent to the background
    /// thread. The job calls builder.build() and pushes the result.
    ///
    /// Backpressure: if the background queue is bounded and all workers are
    /// busy, push blocks — preventing unbounded build queue growth.
    fn submit_build(&self, builder: Arc<dyn ProjectBuilder>, change: FileChange) {
        let result_queue = self.result_queue.clone();

        valtron::run_background_job(move || {
            tracing::info!("{}: building for {:?}", builder.name(), change);
            let result = builder.build(&change);
            let _ = result_queue.push(result);
            // Queue no longer empty → QueueReadiness wakes the TaskIterator
        }).expect("background job pool available");
    }
}
```

### Example builders

```rust
/// Standard Rust cargo build.
pub struct CargoBuilder {
    workspace_root: String,
    crate_name: String,
    build_args: Vec<String>,
    skip_check: bool,
}

impl ProjectBuilder for CargoBuilder {
    fn name(&self) -> &str { "cargo" }

    fn should_build(&self, change: &FileChange) -> bool {
        matches!(change, FileChange::Rust(_))
    }

    fn build(&self, _change: &FileChange) -> BuildResult {
        if !self.skip_check {
            run_cargo_check(&self.workspace_root)?;
        }
        run_cargo_build(&self.workspace_root, &self.crate_name, &self.build_args)
    }
}

/// WASM build via `cargo build --target wasm32-unknown-unknown`.
/// No wasm-pack, no wasm-bindgen — just a raw `.wasm` binary.
/// Use this when you manage .wasm loading yourself (e.g. embedded Deno runtime,
/// foundation_testbed, or custom WASM host).
pub struct WasmBuilder {
    crate_path: String,
    target: String,           // "wasm32-unknown-unknown" or "wasm32-wasip1"
    release: bool,
    extra_args: Vec<String>,
}

impl ProjectBuilder for WasmBuilder {
    fn name(&self) -> &str { "wasm" }

    fn should_build(&self, change: &FileChange) -> bool {
        // Only trigger on Rust files in the WASM crate's path
        matches!(change, FileChange::Rust(path) if path.starts_with(&self.crate_path))
    }

    fn build(&self, _change: &FileChange) -> BuildResult {
        let mut args = vec!["build".into(), "--target".into(), self.target.clone()];
        if self.release {
            args.push("--release".into());
        }
        args.extend_from_slice(&self.extra_args);
        run_cargo_cmd(&self.crate_path, &args)
    }
}

/// WASM build via `wasm-pack build`.
/// Produces .wasm + JS bindings + pkg directory.
/// Use this when targeting web/browser with wasm-bindgen output.
pub struct WasmPackBuilder {
    crate_path: String,
    target: String,    // "bundler", "web", "no-modules", "nodejs"
    release: bool,
    extra_args: Vec<String>,
}

impl ProjectBuilder for WasmPackBuilder {
    fn name(&self) -> &str { "wasm-pack" }

    fn should_build(&self, change: &FileChange) -> bool {
        // Only trigger on Rust files in the WASM crate's path
        matches!(change, FileChange::Rust(path) if path.starts_with(&self.crate_path))
    }

    fn build(&self, _change: &FileChange) -> BuildResult {
        let mut args = vec!["build".into(), "--target".into(), self.target.clone()];
        if self.release {
            args.push("--release".into());
        }
        args.extend_from_slice(&self.extra_args);
        run_wasm_pack(&self.crate_path, &args)
    }
}

/// JS/TS bundler (esbuild, webpack, etc.).
pub struct BundlerBuilder {
    entry_dir: String,
    bundler_cmd: Vec<String>,
}

impl ProjectBuilder for BundlerBuilder {
    fn name(&self) -> &str { "esbuild" }

    fn should_build(&self, change: &FileChange) -> bool {
        matches!(change, FileChange::Javascript(path) | FileChange::Typescript(path)
            if path.starts_with(&self.entry_dir))
    }

    fn build(&self, _change: &FileChange) -> BuildResult {
        run_bundler(&self.entry_dir, &self.bundler_cmd)
    }
}
```

### Usage in DevService

```rust
let builder_task = ProjectBuilderTask::new(
    change_rx, build_complete_queue,
)
.builder(CargoBuilder {
    workspace_root: project.workspace_root.clone(),
    crate_name: project.crate_name.clone(),
    build_args: project.build_arguments.clone(),
    skip_check: project.skip_rust_checks,
})
.builder(WasmBuilder {
    crate_path: "crates/wasm-lib".into(),
    wasm_args: vec!["--release".into()],
})
.builder(BundlerBuilder {
    entry_dir: "frontend/src".into(),
    bundler_cmd: vec!["npx", "esbuild", "frontend/src/index.ts", "--bundle"],
});

engine.schedule(Box::new(builder_task))?;
```

### Why this design

| Benefit | How |
|---------|-----|
| **Pluggable** | Any `ProjectBuilder` impl — cargo, wasm-pack, esbuild, custom scripts |
| **Scoped** | Each builder decides via `should_build` — path matching, file type, project config |
| **Natural backpressure** | `Arc<dyn ProjectBuilder>` submitted to `BackgroundJobRegistry` — if all workers busy and queue is bounded, `push` blocks. No unbounded thread spawning. |
| **Zero-copy dispatch** | Builder is already `Arc` — cloned into background job, no serialization |
| **Fire-and-forget** | Task doesn't wait for results — drains them when available, parks via `QueueReadiness` |

### Key Differences from Current Implementation

| Current | New |
|---------|-----|
| Hardcoded cargo-only | Pluggable `ProjectBuilder` trait |
| Single builder | `Vec<Arc<dyn ProjectBuilder>>` — multiple builders per task |
| `tokio::process::Command.output().await` | `run_background_job` → `builder.build()` |
| `tokio::select!` for event waiting | `Depends(QueueReadiness)` — executor parks, wakes on queue message |
| Unbounded `tokio::spawn` | `BackgroundJobRegistry` fixed pool — natural backpressure |
| `broadcast::Sender<()>` for build complete | `ConcurrentQueue<()>` pushed by background job |

### Task Breakdown

1. [ ] Define `ProjectBuilder` trait (`name`, `should_build`, `build`)
2. [ ] Define `BuildResult` type (success with artifact info, or error)
3. [ ] Implement `CargoBuilder` (replaces current CargoShellBuilder logic)
4. [ ] Implement `ProjectBuilderTask` (manages multiple builders, dispatches to background)
5. [ ] Implement `WasmBuilder` (raw `cargo build --target wasm32-unknown-unknown`)
6. [ ] Implement `WasmPackBuilder` (`wasm-pack build` with target/release flags)
7. [ ] Wire into `DevService` with builder registration
8. [ ] Write tests: verify should_build filtering, verify background dispatch

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/src/builder/mod.rs` | Create — ProjectBuilder trait, ProjectBuilderTask |
| `backends/foundation_toolings/src/builder/cargo.rs` | Create — CargoBuilder impl |
| `backends/foundation_toolings/src/builder/wasm.rs` | Create — WasmBuilder (raw cargo wasm) |
| `backends/foundation_toolings/src/builder/wasm_pack.rs` | Create — WasmPackBuilder (wasm-pack CLI) |

---

_Created: 2026-06-01_
