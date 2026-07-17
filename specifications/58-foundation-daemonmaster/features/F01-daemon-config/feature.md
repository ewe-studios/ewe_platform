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

    /// Require that `key` is already set in the process environment.
    /// Fails at boot time if the variable is not present.
    /// Useful for secrets/credentials that must be injected externally
    /// (CI secrets, vault, env files) rather than hardcoded.
    pub fn must_env(mut self, key: &str) -> Self {
        let value = std::env::var(key)
            .unwrap_or_else(|_| panic!("required env var {key} is not set"));
        self.env.insert(key.into(), value);
        self
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
        valtron::spawn(async move {
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
| **Proc macro** | `#[daemon_process({ ... }, { ... })]` on `fn main()` or test | Binary always owns these daemons, starts at boot |
| **Builder API** | `DaemonGroup::boot(vec![...])` | Tests, dynamic config, programmatic startup |
| **TOML config** | `daemon.toml` file | Project-level config, ops deployment |

### C.1 — Proc macro attribute (binary always-owns pattern)

A single `#[daemon_process]` invocation declares every daemon via `{...}` blocks,
comma-separated. One invocation = one `DaemonGroup`. Stacking is rejected at
compile time. The function MUST take `&DaemonGroup` or `DaemonGroup` as its
first parameter (same constraint as `#[docker_container]`).

```rust
// In foundation_macros/src/daemon_process.rs (proc_macro_attribute)
//
// Pattern matches #[docker_container]: single invocation, {...} per daemon,
// hand-rolled parser (syn::Meta rejects `as` keyword), duplicate key = compile error,
// stacking = compile error, fn must take DaemonGroup.

/// # Example
/// ```rust
/// #[daemon_process(
///     { name = "db", run = ["postgres", "-D", "/var/lib/postgres/data"],
///       readiness_port = 5432,
///       must_env = ["POSTGRES_PASSWORD", "POSTGRES_DB"] },
///     { name = "redis", run = ["redis-server"],
///       readiness_port = 6379 },
///     { name = "api", run = ["./target/release/api-server"],
///       depends = ["db", "redis"],
///       readiness_http = "http://localhost:3000/health",
///       env = [("LOG_LEVEL", "info")] },
/// )]
/// fn main(group: &DaemonGroup) {
///     // POSTGRES_PASSWORD and POSTGRES_DB were validated from the environment
///     // before any daemon was started. LOG_LEVEL was set explicitly to "info".
///     let db = group.daemon("db").unwrap();
///     println!("DB PID: {:?}", db.pid());
/// }
/// ```
///
/// The macro expands to: build DaemonDef[] → DaemonGroup::boot() → call user fn.
```

**Macro expansion** (async fn — executor-agnostic):

```rust
// What #[daemon_process] expands to on an `async fn` (simplified):
//
// The macro does NOT introduce an executor — it generates an async fn that
// awaits DaemonGroup::boot(), and the caller's executor drives it.
// This means the user can run it under valtron, tokio, async-std, or any other.

async fn main() {
    // 1. Build daemon definitions from the {...} blocks (in declaration order).
    let definitions = vec![
        DaemonDef::new("db", vec!["postgres".into(), "-D".into(), "/var/lib/postgres/data".into()])
            .readiness(ReadinessConfig::Port(5432)),
        DaemonDef::new("redis", vec!["redis-server".into()])
            .readiness(ReadinessConfig::Port(6379)),
        DaemonDef::new("api", vec!["./target/release/api-server".into()])
            .depends(&["db", "redis"])
            .readiness(ReadinessConfig::Http("http://localhost:3000/health".into())),
    ];

    // 2. Boot all daemons (dependency-ordered) — this is async, caller's executor drives it.
    let group = DaemonGroup::boot(definitions).await.unwrap();

    // 3. Bind the group to the name the body declared.
    // 4. Call the original function body.
    original_main(&group);

    // 5. Group drops on function exit → all daemons stopped (reverse order).
}
```

**Stacking rules** (matches `#[docker_container]` exactly):
- One invocation declares every daemon via `{...}` blocks
- Stacking `#[daemon_process(...)]` on the same fn = compile error:
  `"#[daemon_process] cannot be stacked: declare every daemon in one invocation,
  one `{ ... }` block each"`
- Fn **must** take exactly one parameter (`group: &DaemonGroup` or `_: DaemonGroup`) —
  zero-arg fn = compile error
- Lookup by `name` key (`name = "..."` in the block), NOT by fn name
- Duplicate `name` within one invocation = compile error

**Parser** — hand-rolled (not `syn::Meta`) because `as`, `run`, `depends` etc. are
not all valid Meta paths. Peeking for keyword tokens (`Token![as]`, `Token![run]`)
gives better error spans.

```rust
// foundation_macros/src/daemon_process.rs

struct Attr {
    defs: Vec<DaemonDef>,
}

impl Parse for Attr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut defs = Vec::new();

        if input.peek(syn::token::Brace) {
            while !input.is_empty() {
                let content;
                syn::braced!(content in input);
                defs.push(content.parse::<DaemonDefAttr>()?);
                if input.is_empty() { break; }
                input.parse::<Token![,]>()?;
            }
        } else {
            // Shorthand: bare key=value for a single daemon.
            defs.push(input.parse::<DaemonDefAttr>()?);
        }

        if defs.is_empty() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[daemon_process] needs at least one daemon definition",
            ));
        }

        // Duplicate name detection at compile time.
        let mut seen: HashMap<&str, ()> = HashMap::new();
        for def in &defs {
            if seen.insert(def.name.as_str(), ()).is_some() {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!(
                        "duplicate daemon name {:?}: each `name = \"...\"` must be unique \
                         within one #[daemon_process]",
                        def.name
                    ),
                ));
            }
        }

        Ok(Attr { defs })
    }
}
```

**Macro expansion** (sync fn — wraps in async, caller provides executor):

```rust
// For sync fns applied with #[daemon_process], the macro generates an inner
// async function that bootstraps the daemons, then wraps it so a sync caller
// can use their executor of choice (valtron::block_on_future, etc.).

#[daemon_process({ name = "db", run = ["postgres"] })]
fn my_app(group: &DaemonGroup) {
    // body
}

// Expands to:
fn my_app() {
    async fn __daemon_boot_my_app(group: &DaemonGroup) {
        // original body
    }

    // The caller uses their own executor to drive this:
    //   valtron::block_on_future(my_app())
    //   or any other executor.
    //
    // The macro itself does NOT spawn — it just restructures the fn to be
    // async and boot daemons before the body. The caller's executor runs it.
}
```

**The `daemons!{}` macro is also executor-agnostic** — it's a `macro_rules!` that
generates a `Vec<DaemonDef>`. No async, no executor, no runtime dependency.

### C.1b — `daemons!{}` declarative macro (inline anywhere)

A declarative macro that builds a `Vec<DaemonDef>` inline — usable inside any
function, not just as an attribute on `fn main()`:

```rust
// foundation_macros/src/lib.rs — declarative macro (macro_rules!)

/// Build a Vec<DaemonDef> inline. Same {...} block syntax as #[daemon_process].
///
/// # Example
/// ```rust
/// async fn boot_my_services() {
///     let definitions = daemons! {
///         { name = "db", run = ["postgres", "-D", "/var/lib/pg"],
///           readiness_port = 5432 },
///         { name = "redis", run = ["redis-server"],
///           readiness_port = 6379 },
///         { name = "api", run = ["./api-server"],
///           depends = ["db", "redis"],
///           readiness_http = "http://localhost:3000/health" },
///     };
///
///     let group = DaemonGroup::boot(definitions).await.unwrap();
///     // ...
/// }
/// ```
///
/// Also usable from TOML-loaded or programmatically-built configs:
/// ```rust
/// let defs = daemons! {
///     { name = "worker", run = ["./worker"],
///       readiness_port = 8080,
///       memory_limit = "1GB" },
/// };
/// // Can merge with other DaemonDef sources:
/// let mut all = load_from_toml()?;
/// all.extend(defs);
/// ```
```

Expansion:

```rust
// daemons!{ { name = "db", run = ["postgres"], readiness_port = 5432 } }
// expands to:
vec![
    DaemonDef::new("db", vec!["postgres".into()])
        .readiness(ReadinessConfig::Port(5432)),
]
```

The same `{...}` block syntax as `#[daemon_process]` — each block maps to a
`DaemonDef::new(...)` builder chain. Supports all `DaemonDef` fields:
- `name = "..."` — required
- `run = ["binary", "arg1", ...]` — required
- `depends = ["dep1", "dep2"]`
- `readiness_port = N` / `readiness_http = "url"` / `readiness_delay = N`
  / `readiness_output = "regex"` / `readiness_cmd = ["cmd", "args"]`
- `env = [("KEY", "VALUE")]` — explicit key-value pairs
- `must_env = ["KEY1", "KEY2"]` — read from process env, fail if missing
- `cwd = "/path"`
- `watch = ["src/**/*.rs"]`
- `memory_limit = "500MB"` / `cpu_limit = 80`
- `restart = true/false` / `max_restarts = 5`
- `stop_timeout = 10`
- `boot_start = true`

### C.2 — Builder API (programmatic)

```rust
#[valtron_test]
async fn test_app_with_db_and_cache() {
    let group = DaemonGroup::boot(vec![
        DaemonDef::new("db", vec!["postgres".into()])
            .must_env("POSTGRES_PASSWORD")     // must exist in process env
            .must_env("POSTGRES_DB")           // must exist in process env
            .readiness(ReadinessConfig::Port(5432)),
        DaemonDef::new("cache", vec!["redis-server".into()])
            .readiness(ReadinessConfig::Port(6379)),
        DaemonDef::new("api", vec!["./target/release/api-server".into()])
            .depends(&["db", "cache"])
            .env("LOG_LEVEL", "info")          // explicit value
            .must_env("API_SECRET_KEY")        // from vault/CI
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
