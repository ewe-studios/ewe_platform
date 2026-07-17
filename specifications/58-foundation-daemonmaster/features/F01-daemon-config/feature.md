---
workspace_name: "ewe_platform"
spec_directory: "specifications/58-foundation-daemonmaster"
feature_directory: "specifications/58-foundation-daemonmaster/features/F01-daemon-config"
this_file: "specifications/58-foundation-daemonmaster/features/F01-daemon-config/feature.md"

status: planned
priority: high
created: 2026-07-18

depends_on: []

tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# F01 — Daemon config system: TOML + macro + builder + DaemonGroup

## Overview

Three entry points for defining daemons — all converging on the same `DaemonDef`
struct. A binary can statically declare what it always wants running via proc macro,
and at startup the `DaemonGroup` automatically owns and boots all child processes
connected to itself. Inspired by pitchfork's config system + the `#[docker_container]`
tri-config pattern (macro/builder/static).

[spec](../spec.md).

---

## Part A — Core types (single source of truth)

```rust
// foundation_nativeapis/src/daemon/config.rs

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A single daemon definition — constructed statically, at runtime,
/// or via the `#[daemon_process]` proc macro.
#[derive(Debug, Clone)]
pub struct DaemonDef {
    /// Unique name for lookup within the group.
    pub name: String,
    /// Binary + args to run.
    pub run: Vec<String>,
    /// Working directory. Defaults to current dir.
    pub cwd: Option<PathBuf>,
    /// Startup dependencies — these names must be ready first.
    pub depends: Vec<String>,
    /// Readiness detection strategy.
    pub readiness: ReadinessConfig,
    /// File patterns to watch for auto-restart (F06).
    pub watch: Vec<String>,
    /// Cron schedule for periodic restart (F06).
    pub cron_schedule: Option<String>,
    /// Stop timeout in seconds before SIGKILL (default: 10).
    pub stop_timeout: Option<u64>,
    /// Auto-restart on exit (default: true).
    pub restart: bool,
    /// Max restart attempts before giving up (default: 5).
    pub max_restarts: Option<u32>,
    /// Environment variables to inject.
    pub env: BTreeMap<String, String>,
    /// Resource limits (F07).
    pub memory_limit: Option<String>,
    pub cpu_limit: Option<u32>,
    /// Lifecycle hook commands (F02).
    pub hooks: Option<LifecycleHooks>,
    /// Boot-start: always launch this daemon at startup.
    pub boot_start: bool,
}

impl DaemonDef {
    /// Builder constructor.
    pub fn new(name: impl Into<String>, run: Vec<String>) -> Self {
        Self {
            name: name.into(),
            run,
            cwd: None,
            depends: Vec::new(),
            readiness: ReadinessConfig::default(),
            watch: Vec::new(),
            cron_schedule: None,
            stop_timeout: None,
            restart: true,
            max_restarts: None,
            env: BTreeMap::new(),
            memory_limit: None,
            cpu_limit: None,
            hooks: None,
            boot_start: false,
        }
    }

    pub fn depends(mut self, names: &[&str]) -> Self {
        self.depends = names.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn readiness(mut self, config: ReadinessConfig) -> Self {
        self.readiness = config; self
    }
    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.env.insert(key.into(), value.into()); self
    }
    pub fn cwd(mut self, p: PathBuf) -> Self { self.cwd = Some(p); self }
    pub fn watch(mut self, patterns: &[&str]) -> Self {
        self.watch = patterns.iter().map(|s| s.to_string()).collect(); self
    }
    pub fn memory_limit(mut self, limit: &str) -> Self {
        self.memory_limit = Some(limit.into()); self
    }
    pub fn cpu_limit(mut self, limit: u32) -> Self {
        self.cpu_limit = Some(limit); self
    }
    pub fn restart(mut self, enabled: bool) -> Self { self.restart = enabled; self }
    pub fn max_restarts(mut self, n: u32) -> Self {
        self.max_restarts = Some(n); self
    }
    pub fn stop_timeout(mut self, secs: u64) -> Self {
        self.stop_timeout = Some(secs); self
    }
    pub fn boot_start(mut self) -> Self { self.boot_start = true; self }
}
```

### A.2 — Readiness + hooks (shared with TOML)

```rust
/// Readiness detection — mutually exclusive strategies, pick one.
#[derive(Debug, Clone, Default)]
pub enum ReadinessConfig {
    Delay(std::time::Duration),
    Output(regex::Regex),
    Http(String),
    Port(u16),
    Cmd(Vec<String>),
    #[default]
    Immediate,
}

#[derive(Debug, Clone)]
pub struct LifecycleHooks {
    pub on_ready: Option<String>,
    pub on_fail: Option<String>,
    pub on_retry: Option<String>,
    pub on_stop: Option<String>,
    pub on_exit: Option<String>,
}
```

### A.3 — TOML deserialization

The same `DaemonDef` is `Deserialize`-compatible for config file loading:

```rust
/// Top-level config loaded from a TOML file.
#[derive(Debug, Clone, Deserialize)]
pub struct DaemonConfig {
    pub namespace: Option<String>,
    #[serde(default)]
    pub root: Option<bool>,
    /// Daemon definitions keyed by name.
    pub daemons: BTreeMap<String, DaemonDef>,
}

// DaemonDef derives Deserialize for TOML loading.
// Builder-constructed instances bypass serde entirely.
```

---

## Part B — DaemonGroup (grouped lifecycle handle)

```rust
// foundation_nativeapis/src/daemon/group.rs

/// A handle to a group of running daemon processes. Created by
/// `DaemonGroup::boot(vec![def1, def2, ...])`. On Drop, stops all daemons
/// in reverse dependency order.
///
/// The group handle allows individual daemon access for inspection,
/// status queries, and restart.
pub struct DaemonGroup {
    daemons: BTreeMap<DaemonId, DaemonHandle>,
    supervisor: Arc<Supervisor>,
}

impl DaemonGroup {
    /// Boot all daemons from their definitions. Dependency-ordered start.
    /// Returns a group handle that will clean up all daemons on Drop.
    pub async fn boot(
        definitions: Vec<DaemonDef>,
    ) -> Result<Self, DaemonError> {
        let supervisor = Supervisor::new(definitions).await?;
        supervisor.start_all().await?;
        Ok(Self {
            daemons: supervisor.daemon_handles(),
            supervisor,
        })
    }

    /// Get a handle to an individual daemon by name.
    pub fn daemon(&self, name: &str) -> Option<&DaemonHandle> {
        self.daemons.values().find(|h| h.id.name == name)
    }

    /// Get all daemon handles.
    pub fn daemons(&self) -> &BTreeMap<DaemonId, DaemonHandle> {
        &self.daemons
    }

    /// Restart a specific daemon.
    pub async fn restart(&self, name: &str) -> Result<(), DaemonError> {
        if let Some(handle) = self.daemon(name) {
            self.supervisor.restart_daemon(&handle.id).await
        } else {
            Err(DaemonError::NotFound(name.into()))
        }
    }
}

impl Drop for DaemonGroup {
    fn drop(&mut self) {
        // Best-effort reverse-order teardown.
        // Actual cleanup happens via supervisor.shutdown().
        let supervisor = self.supervisor.clone();
        tokio::spawn(async move {
            let _ = supervisor.shutdown().await;
        });
    }
}

/// Handle to an individual running daemon.
pub struct DaemonHandle {
    pub id: DaemonId,
    pub pid: Option<u32>,
    pub status: DaemonStatus,
    supervisor: Arc<Supervisor>,
}

impl DaemonHandle {
    pub fn pid(&self) -> Option<u32> { self.pid }
    pub fn status(&self) -> DaemonStatus { self.status }
    pub async fn restart(&self) -> Result<(), DaemonError> {
        self.supervisor.restart_daemon(&self.id).await
    }
    pub async fn stop(&self) -> Result<(), DaemonError> {
        self.supervisor.stop_daemon(&self.id).await
    }
}
```

---

## Part C — Three entry points

Users have three ways to define daemons, all backed by `DaemonDef`:

| Path | Mechanism | Use case |
|------|-----------|----------|
| **Proc macro** | `#[daemon_process(...)]` on `fn main()` or test | Binary always owns these daemons, starts at boot |
| **Builder API** | `DaemonGroup::boot(vec![...])` | Tests, dynamic config, programmatic startup |
| **TOML config** | `daemon.toml` file | Project-level config, ops deployment |

### C.1 — Proc macro (binary always-owns pattern)

A binary declares what it always wants running. At startup, the macro expansion
automatically boots all declared daemons and passes the `DaemonGroup` to the
function. The binary owns the daemons for its entire lifetime.

```rust
// In foundation_macros/src/daemon.rs (proc_macro_attribute)

/// Apply to `fn main()` or any entry function. Each attribute defines one daemon.
/// Multiple attributes stack — all daemons start before the function body runs.
/// The function MUST take `&DaemonGroup` or `DaemonGroup` as its first parameter.
///
/// # Example
/// ```rust
/// #[daemon_process(name = "api", run = ["./target/release/api-server"],
///     readiness = "port(3000)", depends = ["db"])]
/// #[daemon_process(name = "db", run = ["postgres", "-D", "/var/lib/postgres/data"],
///     readiness = "port(5432)")]
/// #[daemon_process(name = "redis", run = ["redis-server"],
///     readiness = "port(6379)")]
/// fn main(group: &DaemonGroup) {
///     // All 3 daemons are running.
///     let db = group.daemon("db").unwrap();
///     println!("DB PID: {:?}", db.pid());
///
///     // Run your app...
/// }
/// ```
///
/// The macro expands to wrap the function body in a `DaemonGroup::boot()` call.
```

**Macro expansion:**

```rust
// What #[daemon_process] expands to (simplified):

fn main() {
    // 1. Build daemon definitions from macro attributes.
    let definitions = vec![
        DaemonDef::new("api", vec!["./target/release/api-server".into()])
            .depends(&["db"])
            .readiness(ReadinessConfig::Port(3000)),
        DaemonDef::new("db", vec!["postgres".into(), "-D".into(), "/var/lib/postgres/data".into()])
            .readiness(ReadinessConfig::Port(5432)),
        DaemonDef::new("redis", vec!["redis-server".into()])
            .readiness(ReadinessConfig::Port(6379)),
    ];

    // 2. Boot all daemons (dependency-ordered).
    let group = tokio::runtime::Runtime::new().unwrap()
        .block_on(async {
            DaemonGroup::boot(definitions).await.unwrap()
        });

    // 3. Call the original function with the group.
    original_main(&group);

    // 4. Group drops on function exit → all daemons stopped.
}
```

**Stacking rules** (same as `#[docker_container]`):
- Multiple `#[daemon_process(...)]` attributes on one fn = multi-daemon group
- All daemons defined in ONE set of stacked attributes on the same fn
- Stacking on multiple fns = compile error (daemons must be defined together)
- Fn **must** take `&DaemonGroup` or `DaemonGroup` — zero-arg fn is a compile error
- Lookup by `name` key (the `name = "..."` in the attribute), NOT by fn name

### C.2 — Builder API (programmatic)

```rust
#[valtron_test]
async fn test_app_with_db_and_cache() {
    let group = DaemonGroup::boot(vec![
        DaemonDef::new("db", vec!["postgres".into()])
            .env("POSTGRES_PASSWORD", "test")
            .env("POSTGRES_DB", "app_test")
            .readiness(ReadinessConfig::Port(5432)),
        DaemonDef::new("cache", vec!["redis-server".into()])
            .readiness(ReadinessConfig::Port(6379)),
        DaemonDef::new("api", vec!["./target/release/api-server".into()])
            .depends(&["db", "cache"])
            .readiness(ReadinessConfig::Http("http://localhost:3000/health".into()))
            .memory_limit("500MB"),
    ]).await.expect("Failed to boot daemons");

    let db = group.daemon("db").unwrap();
    let cache = group.daemon("cache").unwrap();

    // Connect to daemons via their ports...

    // Group drops here — all daemons stopped in reverse order.
}
```

### C.3 — Static definitions (reusable)

```rust
const DB_DAEMON: DaemonDef = DaemonDef {
    name: "db".into(),
    run: vec!["postgres".into()],
    readiness: ReadinessConfig::Port(5432),
    env: BTreeMap::from([
        ("POSTGRES_PASSWORD".into(), "test".into()),
    ]),
    ..DaemonDef::defaults()
};

// Reused across tests, binaries, and config exports.
```

### C.4 — TOML config (ops/deployment)

```toml
# daemon.toml
namespace = "myapp"

[daemons.db]
run = ["postgres", "-D", "/var/lib/postgres/data"]
readiness = { port = 5432 }

[daemons.api]
run = ["./target/release/api-server"]
depends = ["db"]
readiness = { http = "http://localhost:3000/health" }
memory_limit = "500MB"
watch = ["src/**/*.rs"]

[daemons.api.hooks]
on_ready = "echo 'API is ready!'"
on_fail = "notify-send 'API crashed'"
```

```rust
// Load from TOML, boot into a DaemonGroup.
let config = DaemonConfig::load_from_path(&std::env::current_dir()?)?;
let definitions = config.into_daemon_defs();
let group = DaemonGroup::boot(definitions).await?;
```

---

## Part D — Config file layering

TOML files are loaded recursively from cwd to filesystem root, merging at each level:

```
cwd/daemon.local.toml     (highest priority — nearest, gitignored)
cwd/daemon.toml           (project config)
~/daemon.toml             (user-global)
```

The `root = true` field in a config stops upward recursion.

```rust
impl DaemonConfig {
    pub fn load_from_path(cwd: &Path) -> Result<Self, ConfigError> {
        let mut configs = Vec::new();
        for dir in cwd.ancestors() {
            if let Some(path) = Self::find_config(dir) {
                let cfg = Self::load_file(&path)?;
                configs.push(cfg);
                if cfg.root == Some(true) { break; }
            }
        }
        configs.into_iter().rev().reduce(|a, b| a.merge(b)).unwrap_or_default()
    }

    fn find_config(dir: &Path) -> Option<PathBuf> {
        let local = dir.join("daemon.local.toml");
        if local.is_file() { return Some(local); }
        let main = dir.join("daemon.toml");
        if main.is_file() { return Some(main); }
        None
    }
}
```

---

## Part E — Dependency graph and topological sort

```rust
// foundation_nativeapis/src/daemon/deps.rs

/// Compute startup order via topological sort (Kahn's algorithm).
/// Returns levels — daemons in the same level start concurrently.
pub fn startup_order(
    daemons: &[DaemonDef],
) -> Result<Vec<Vec<DaemonId>>, CycleError> {
    let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();
    let mut dependents: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for def in daemons {
        in_degree.entry(def.name.clone()).or_insert(0);
        for dep in &def.depends {
            dependents.entry(dep.clone()).or_default().push(def.name.clone());
            *in_degree.entry(def.name.clone()).or_insert(0) += 1;
        }
    }

    let mut queue: VecDeque<String> = in_degree.iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(id, _)| id.clone())
        .collect();

    let mut levels: Vec<Vec<DaemonId>> = Vec::new();
    let mut processed = 0;

    while !queue.is_empty() {
        let level: Vec<String> = queue.drain(..).collect();
        processed += level.len();
        for id in &level {
            if let Some(deps) = dependents.remove(id) {
                for dep in deps {
                    let deg = in_degree.get_mut(&dep).unwrap();
                    *deg -= 1;
                    if *deg == 0 { queue.push_back(dep); }
                }
            }
        }
        levels.push(level.into_iter()
            .map(|name| DaemonId { namespace: "default".into(), name })
            .collect());
    }

    if processed < daemons.len() {
        let cycle: Vec<DaemonId> = in_degree.into_iter()
            .filter(|(_, deg)| *deg > 0)
            .map(|(name, _)| DaemonId { namespace: "default".into(), name })
            .collect();
        Err(CycleError { cycle })
    } else {
        Ok(levels)
    }
}

/// Reverse order for shutdown (dependents first).
pub fn shutdown_order(levels: &[Vec<DaemonId>]) -> Vec<Vec<DaemonId>> {
    levels.iter().rev().cloned().collect()
}
```

### E.2 — ID resolution

Dependencies are resolved by name within the same group/namespace. Cross-group
references use the qualified `namespace/name` format:

```rust
fn resolve_id(self_id: &DaemonId, ref_name: &str) -> Result<DaemonId, ConfigError> {
    if ref_name.contains('/') {
        let (ns, name) = ref_name.split_once('/').unwrap();
        Ok(DaemonId { namespace: ns.into(), name: name.into() })
    } else {
        Ok(DaemonId { namespace: self_id.namespace.clone(), name: ref_name.into() })
    }
}
```

---

## Part F — `#[daemon_main]` entry point macro

For binaries that want the cleanest possible entry point:

```rust
/// Like `#[wireguard_main]` — boots the valtron pool, loads daemon config,
/// starts all daemons, and hands the `DaemonGroup` to the user's function.
///
/// ```rust
/// #[daemon_main]
/// fn my_app(group: &DaemonGroup) {
///     // All daemons from daemon.toml are running.
///     let api = group.daemon("api").unwrap();
///     println!("API PID: {:?}", api.pid());
///
///     // Your app runs here...
/// }
/// ```
///
/// Expansion:
/// 1. Initialize valtron executor pool
/// 2. Load `daemon.toml` from cwd (or `config="path.toml"` override)
/// 3. Boot all daemons (dependency-ordered)
/// 4. Call user function with `&DaemonGroup`
/// 5. On function return → group drops → daemons stop
```

With config override:

```rust
#[daemon_main(config = "configs/production.toml")]
fn prod_app(group: &DaemonGroup) {
    // Daemons loaded from configs/production.toml
}
```

---

## Verification

```bash
cargo test --package foundation_nativeapis --features daemon -- daemon::config
cargo test --package foundation_nativeapis --features daemon -- daemon::group
cargo test --package foundation_macros -- daemon_process
```

Tests cover:
- `DaemonDef` builder: all chain methods produce correct struct
- `DaemonGroup::boot` → dependency-ordered start, reverse-order Drop
- TOML loading: nested directory merge, `root = true` stops recursion
- Topological sort: concurrent levels, cycle detection with member list
- Proc macro: single attribute, stacked attributes, fn must take DaemonGroup
- `#[daemon_main]`: loads TOML, boots daemons, hands group to user fn
- Cross-group ID resolution: `namespace/name` format
