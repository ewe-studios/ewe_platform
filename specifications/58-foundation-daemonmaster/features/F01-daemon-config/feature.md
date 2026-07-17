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
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# F01 — TOML config system + namespace derivation + dependency graph

## Overview

Config file parser and layering for daemon definitions. Supports recursive directory
search, namespace derivation, dependency graph construction, and topological sort for
startup ordering. Inspired by pitchfork's config system.

[spec](../spec.md).

---

## Part A — Config types and TOML parsing

```rust
// foundation_nativeapis/src/daemon/config.rs

use serde::Deserialize;

/// Top-level config loaded from a TOML file.
#[derive(Debug, Clone, Deserialize)]
pub struct DaemonConfig {
    /// Explicit namespace override. If None, derived from config file path.
    pub namespace: Option<String>,
    /// Daemon definitions keyed by name.
    pub daemons: std::collections::BTreeMap<String, DaemonDef>,
}

/// A single daemon definition.
#[derive(Debug, Clone, Deserialize)]
pub struct DaemonDef {
    /// Command to run (binary + args).
    pub run: Vec<String>,
    /// Working directory for the process. Defaults to config file's directory.
    pub cwd: Option<std::path::PathBuf>,
    /// Startup dependencies — these daemon IDs must be ready first.
    #[serde(default)]
    pub depends: Vec<String>,
    /// Readiness detection strategy.
    #[serde(default)]
    pub readiness: ReadinessConfig,
    /// File patterns to watch for auto-restart (F06).
    #[serde(default)]
    pub watch: Vec<String>,
    /// Cron schedule for periodic restart (F06).
    pub cron_schedule: Option<String>,
    /// Stop timeout in seconds before SIGKILL (default: 10).
    pub stop_timeout: Option<u64>,
    /// Auto-restart on exit (default: true).
    #[serde(default = "default_true")]
    pub restart: bool,
    /// Max restart attempts in a window before giving up (default: 5).
    pub max_restarts: Option<u32>,
    /// Environment variables to inject.
    #[serde(default)]
    pub env: std::collections::BTreeMap<String, String>,
    /// Resource limits (F07).
    pub memory_limit: Option<String>,
    pub cpu_limit: Option<u32>,
    /// Lifecycle hook commands (F02).
    pub hooks: Option<LifecycleHooks>,
}

fn default_true() -> bool { true }

/// Readiness detection configuration (mutually exclusive strategies, pick one).
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessConfig {
    /// Mark ready after a fixed delay in seconds.
    Delay(u64),
    /// Mark ready when stdout/stderr matches this regex.
    Output(String),
    /// Mark ready when this HTTP endpoint returns 2xx.
    Http(String),
    /// Mark ready when this TCP port accepts a connection.
    Port(u16),
    /// Mark ready when this command exits 0.
    Cmd(String),
    /// No readiness check — mark ready immediately on spawn.
    #[default]
    Immediate,
}

/// Lifecycle hook commands.
#[derive(Debug, Clone, Deserialize)]
pub struct LifecycleHooks {
    pub on_ready: Option<String>,
    pub on_fail: Option<String>,
    pub on_retry: Option<String>,
    pub on_stop: Option<String>,
    pub on_exit: Option<String>,
}
```

### A.2 — DaemonId with namespace derivation

```rust
/// Qualified daemon identifier: `namespace/name`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DaemonId {
    pub namespace: String,
    pub name: String,
}

impl DaemonId {
    /// Derive namespace from a config file's parent directory name.
    /// E.g. `/home/user/my-project/daemon.toml` → namespace = `"my-project"`.
    pub fn derive(config_path: &std::path::Path, name: &str) -> Self {
        let namespace = config_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("global")
            .to_string();
        DaemonId { namespace, name: name.to_string() }
    }
}

impl std::fmt::Display for DaemonId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.namespace, self.name)
    }
}
```

---

## Part B — Config file layering

Config files are loaded recursively from cwd to filesystem root, merging at each level:

```
cwd/daemon.local.toml     (highest priority — nearest, gitignored)
cwd/daemon.toml           (project config)
~/daemon.toml             (user-global)
~/.local/state/daemon/    (state directory, not config)
```

The `root = true` field in a config stops upward recursion.

```rust
impl DaemonConfig {
    /// Load and merge config files from cwd upward.
    ///
    /// Returns the merged config and the directory that defined `root = true`
    /// (if any), which stops the search.
    pub fn load_from_path(cwd: &std::path::Path) -> Result<Self, ConfigError> {
        let mut configs = Vec::new();
        for dir in cwd.ancestors() {
            if let Some(path) = Self::find_config(dir) {
                let cfg = Self::load_file(&path)?;
                configs.push(cfg);
                if cfg.root == Some(true) {
                    break;
                }
            }
        }
        // Merge: first loaded = lowest priority, last = highest.
        configs.into_iter().rev().reduce(|a, b| a.merge(b)).unwrap_or_default()
    }

    fn find_config(dir: &std::path::Path) -> Option<std::path::PathBuf> {
        // Priority: daemon.local.toml > daemon.toml
        let local = dir.join("daemon.local.toml");
        if local.is_file() { return Some(local); }
        let main = dir.join("daemon.toml");
        if main.is_file() { return Some(main); }
        None
    }
}
```

---

## Part C — Dependency graph and topological sort

```rust
// foundation_nativeapis/src/daemon/deps.rs

use std::collections::{BTreeMap, VecDeque};

/// Compute the startup order via topological sort (Kahn's algorithm).
/// Returns levels — daemons in the same level can start concurrently.
///
/// # Errors
/// Returns the cycle members if a dependency cycle is detected.
pub fn startup_order(
    daemons: &BTreeMap<DaemonId, DaemonDef>,
) -> Result<Vec<Vec<DaemonId>>, CycleError> {
    // Build adjacency: daemon → set of dependencies.
    let mut in_degree: BTreeMap<DaemonId, usize> = BTreeMap::new();
    let mut dependents: BTreeMap<DaemonId, Vec<DaemonId>> = BTreeMap::new();

    for (id, def) in daemons {
        in_degree.entry(id.clone()).or_insert(0);
        for dep_name in &def.depends {
            let dep_id = resolve_id(id, dep_name)?;
            dependents.entry(dep_id.clone()).or_default().push(id.clone());
            *in_degree.entry(id.clone()).or_insert(0) += 1;
        }
    }

    // Kahn's: start with zero in-degree nodes.
    let mut queue: VecDeque<DaemonId> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(id, _)| id.clone())
        .collect();

    let mut levels: Vec<Vec<DaemonId>> = Vec::new();
    let mut processed = 0;

    while !queue.is_empty() {
        let level: Vec<DaemonId> = queue.drain(..).collect();
        processed += level.len();
        for id in &level {
            if let Some(deps) = dependents.remove(id) {
                for dep in deps {
                    let deg = in_degree.get_mut(&dep).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(dep);
                    }
                }
            }
        }
        levels.push(level);
    }

    if processed < daemons.len() {
        // Cycle: remaining nodes with in_degree > 0.
        let cycle: Vec<DaemonId> = in_degree
            .into_iter()
            .filter(|(_, deg)| *deg > 0)
            .map(|(id, _)| id)
            .collect();
        Err(CycleError { cycle })
    } else {
        Ok(levels)
    }
}

/// Compute the reverse order for shutdown (dependents first).
pub fn shutdown_order(
    levels: &[Vec<DaemonId>],
) -> Vec<Vec<DaemonId>> {
    levels.iter().rev().cloned().collect()
}

#[derive(Debug)]
pub struct CycleError {
    pub cycle: Vec<DaemonId>,
}
```

### C.2 — ID resolution

Dependencies can be short names (resolved within the same namespace) or qualified:

```rust
fn resolve_id(self_id: &DaemonId, ref_name: &str) -> Result<DaemonId, ConfigError> {
    if ref_name.contains('/') {
        // Already qualified: "other-ns/daemon"
        let (ns, name) = ref_name.split_once('/').unwrap();
        Ok(DaemonId { namespace: ns.into(), name: name.into() })
    } else {
        // Unqualified: resolve in self's namespace.
        Ok(DaemonId { namespace: self_id.namespace.clone(), name: ref_name.into() })
    }
}
```

---

## Verification

```bash
cargo test --package foundation_nativeapis --features daemon -- daemon::config
cargo test --package foundation_nativeapis --features daemon -- daemon::deps
```

Tests cover:
- Config loading from nested directories with merge semantics
- Namespace derivation from directory names
- `root = true` stopping recursion
- Topological sort with concurrent levels
- Cycle detection with informative error
- Short-name vs qualified dependency resolution
