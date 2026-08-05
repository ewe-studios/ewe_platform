//! Daemon configuration: the single `DaemonDef` source of truth plus the TOML
//! surface (`DaemonConfig` / `DaemonSpec`) that deserializes into it.
//!
//! WHY: Three entry points — proc macro, builder API, and TOML config — must all
//! converge on one definition type so the supervisor (F02) reasons about a single
//! shape regardless of where a daemon was declared.
//!
//! WHAT: `DaemonDef` (builder-constructed), `ReadinessConfig` (five mutually
//! exclusive readiness strategies), `LifecycleHooks`, and the TOML-facing
//! `DaemonConfig` + `DaemonSpec` that layer config files from cwd to root.
//!
//! HOW: `DaemonDef` stays serde-free (it carries a compiled `regex::Regex` and a
//! `Duration`, neither cleanly `Deserialize`); TOML instead deserializes into
//! `DaemonSpec` and converts via [`DaemonSpec::into_daemon_def`]. Builder-built
//! defs bypass serde entirely.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use super::error::ConfigError;

/// A single daemon definition — constructed statically, at runtime via the
/// builder, or from the `#[daemon_process]` proc macro / `daemons!` macro.
///
/// WHY: The supervisor needs one uniform description of every managed process.
///
/// WHAT: Binary + args, working dir, dependency names, readiness strategy,
/// restart policy, resource limits, lifecycle hooks, and watch/schedule config.
///
/// HOW: Built through chained builder methods (each returns `Self`) starting
/// from [`DaemonDef::new`]. Fields are public so static `const`/struct-literal
/// construction is also possible via [`DaemonDef::defaults`].
#[derive(Debug, Clone)]
pub struct DaemonDef {
    /// Unique name for lookup within the group.
    pub name: String,
    /// Binary + args to run. `run[0]` is the program; the rest are arguments.
    pub run: Vec<String>,
    /// Working directory. `None` inherits the supervisor's current dir.
    pub cwd: Option<PathBuf>,
    /// Startup dependencies — these names must be ready before this one starts.
    pub depends: Vec<String>,
    /// Readiness detection strategy.
    pub readiness: ReadinessConfig,
    /// File glob patterns to watch for auto-restart (F06).
    pub watch: Vec<String>,
    /// Cron schedule for periodic restart (F06).
    pub cron_schedule: Option<String>,
    /// Stop timeout in seconds before escalating SIGTERM to SIGKILL (default 10).
    pub stop_timeout: Option<u64>,
    /// Auto-restart on unexpected exit (default `true`).
    pub restart: bool,
    /// Max restart attempts before giving up (default: unlimited when `None`).
    pub max_restarts: Option<u32>,
    /// Environment variables injected into the child process.
    pub env: BTreeMap<String, String>,
    /// Memory limit, e.g. `"500MB"` (F07).
    pub memory_limit: Option<String>,
    /// CPU limit as a percentage, e.g. `80` (F07).
    pub cpu_limit: Option<u32>,
    /// Lifecycle hook commands (F02).
    pub hooks: Option<LifecycleHooks>,
    /// Boot-start: always launch this daemon at group startup.
    pub boot_start: bool,
}

impl DaemonDef {
    /// Builder constructor.
    ///
    /// WHY: The common path — name + command line, everything else defaulted.
    ///
    /// WHAT: A `DaemonDef` with the given name and `run` vector and defaults for
    /// all other fields (`restart = true`, immediate readiness, no deps).
    ///
    /// HOW: Delegates to [`DaemonDef::defaults`] and overrides name + run.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(name: impl Into<String>, run: Vec<String>) -> Self {
        Self {
            name: name.into(),
            run,
            ..Self::defaults()
        }
    }

    /// The default field values shared by every construction path.
    ///
    /// WHY: Static `struct`-literal daemons (`DaemonDef { .., ..DaemonDef::defaults() }`)
    /// and [`DaemonDef::new`] must agree on defaults.
    ///
    /// WHAT: An empty-named, empty-command def with `restart = true` and
    /// immediate readiness.
    ///
    /// HOW: Plain struct literal.
    ///
    /// # Panics
    /// Never panics.
    pub fn defaults() -> Self {
        Self {
            name: String::new(),
            run: Vec::new(),
            cwd: None,
            depends: Vec::new(),
            readiness: ReadinessConfig::Immediate,
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

    /// Set startup dependencies by name.
    #[must_use]
    pub fn depends(mut self, names: &[&str]) -> Self {
        self.depends = names.iter().map(|s| (*s).to_string()).collect();
        self
    }

    /// Set the readiness detection strategy.
    #[must_use]
    pub fn readiness(mut self, config: ReadinessConfig) -> Self {
        self.readiness = config;
        self
    }

    /// Inject an explicit environment variable.
    #[must_use]
    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Require that `key` is already present in the supervisor's environment,
    /// copying its value into the child's environment.
    ///
    /// WHY: Secrets/credentials must be injected externally (CI secrets, vault,
    /// env files) rather than hardcoded into a definition. A missing required
    /// variable is a hard configuration error, not something to default around.
    ///
    /// WHAT: Reads `key` from the process environment and records it; aborts if
    /// unset.
    ///
    /// HOW: `std::env::var(key)`, panicking on absence (builder context — there
    /// is no `Result` to thread, and a missing secret must stop startup loudly).
    ///
    /// # Panics
    /// Panics if `key` is not set in the process environment.
    #[must_use]
    pub fn must_env(mut self, key: &str) -> Self {
        let value = std::env::var(key)
            .unwrap_or_else(|_| panic!("required env var {key} is not set"));
        self.env.insert(key.into(), value);
        self
    }

    /// Set the working directory.
    #[must_use]
    pub fn cwd(mut self, p: PathBuf) -> Self {
        self.cwd = Some(p);
        self
    }

    /// Set file glob patterns to watch for auto-restart (F06).
    #[must_use]
    pub fn watch(mut self, patterns: &[&str]) -> Self {
        self.watch = patterns.iter().map(|s| (*s).to_string()).collect();
        self
    }

    /// Set a cron schedule for periodic restart (F06).
    #[must_use]
    pub fn cron_schedule(mut self, expr: impl Into<String>) -> Self {
        self.cron_schedule = Some(expr.into());
        self
    }

    /// Set a memory limit string, e.g. `"500MB"` (F07).
    #[must_use]
    pub fn memory_limit(mut self, limit: &str) -> Self {
        self.memory_limit = Some(limit.into());
        self
    }

    /// Set a CPU limit percentage (F07).
    #[must_use]
    pub fn cpu_limit(mut self, limit: u32) -> Self {
        self.cpu_limit = Some(limit);
        self
    }

    /// Enable or disable auto-restart on exit.
    #[must_use]
    pub fn restart(mut self, enabled: bool) -> Self {
        self.restart = enabled;
        self
    }

    /// Cap the number of restart attempts.
    #[must_use]
    pub fn max_restarts(mut self, n: u32) -> Self {
        self.max_restarts = Some(n);
        self
    }

    /// Set the graceful-stop timeout (seconds) before SIGKILL.
    #[must_use]
    pub fn stop_timeout(mut self, secs: u64) -> Self {
        self.stop_timeout = Some(secs);
        self
    }

    /// Attach lifecycle hook commands.
    #[must_use]
    pub fn hooks(mut self, hooks: LifecycleHooks) -> Self {
        self.hooks = Some(hooks);
        self
    }

    /// Mark this daemon as boot-start (launched at group startup).
    #[must_use]
    pub fn boot_start(mut self) -> Self {
        self.boot_start = true;
        self
    }
}

/// Readiness detection strategy — pick exactly one.
///
/// WHY: "Started" and "ready to serve" differ; dependents must wait for the
/// latter. Each variant encodes a different signal a process gives when usable.
///
/// WHAT: A delay, a stdout/stderr regex match, an HTTP 2xx probe, a TCP port
/// connect, an external command exit-0, or immediate readiness.
///
/// HOW: Consumed by F02's `ReadinessTask`. `Immediate` is the default.
#[derive(Debug, Clone)]
pub enum ReadinessConfig {
    /// Ready after a fixed delay from spawn.
    Delay(Duration),
    /// Ready when a line of stdout/stderr matches this compiled pattern.
    Output(regex::Regex),
    /// Ready when this URL returns a 2xx status.
    Http(String),
    /// Ready when this TCP port on localhost accepts a connection.
    Port(u16),
    /// Ready when this command exits 0.
    Cmd(Vec<String>),
    /// Ready the moment the process is spawned.
    Immediate,
}

impl Default for ReadinessConfig {
    fn default() -> Self {
        Self::Immediate
    }
}

impl ReadinessConfig {
    /// Construct a `Delay` strategy from whole seconds.
    ///
    /// WHY: The `daemons!` macro and TOML express delays as second counts, not
    /// `Duration` literals.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn delay_secs(secs: u64) -> Self {
        Self::Delay(Duration::from_secs(secs))
    }

    /// Construct an `Output` strategy by compiling a regex pattern.
    ///
    /// WHY: Macro-generated code should not need `regex` in scope; this compiles
    /// the developer-authored literal pattern.
    ///
    /// WHAT: A `ReadinessConfig::Output` holding the compiled regex.
    ///
    /// HOW: `regex::Regex::new`, panicking with a clear message on an invalid
    /// pattern (the pattern is a compile-time constant the author wrote).
    ///
    /// # Panics
    /// Panics if `pattern` is not a valid regular expression.
    #[must_use]
    pub fn output(pattern: &str) -> Self {
        let re = regex::Regex::new(pattern)
            .unwrap_or_else(|e| panic!("invalid readiness regex {pattern:?}: {e}"));
        Self::Output(re)
    }
}

/// Lifecycle hook commands run at daemon state transitions (F02).
///
/// WHY: Operators want to fire side effects (notify, warm caches, alert) at
/// well-defined moments without changing the supervised binary.
///
/// WHAT: Optional shell commands for each transition.
///
/// HOW: Each field, when set, is executed by the supervisor at the matching
/// transition.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LifecycleHooks {
    /// Run when the daemon becomes ready.
    pub on_ready: Option<String>,
    /// Run when the daemon fails to start or exceeds its restart limit.
    pub on_fail: Option<String>,
    /// Run before each restart attempt.
    pub on_retry: Option<String>,
    /// Run when the daemon is asked to stop.
    pub on_stop: Option<String>,
    /// Run when the daemon process exits (for any reason).
    pub on_exit: Option<String>,
}

// ---------------------------------------------------------------------------
// TOML surface
// ---------------------------------------------------------------------------

/// Top-level config loaded from a `daemon.toml` file.
///
/// WHY: Project/ops deployments declare daemons in a file rather than code.
///
/// WHAT: An optional namespace, a `root` stop-recursion flag, and the daemon
/// specs keyed by name.
///
/// HOW: Deserialized from TOML, then flattened into `Vec<DaemonDef>` via
/// [`DaemonConfig::into_daemon_defs`]. File layering is [`DaemonConfig::load_from_path`].
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DaemonConfig {
    /// Logical namespace for the daemons in this file.
    pub namespace: Option<String>,
    /// When `true`, stops upward config-file recursion at this file.
    #[serde(default)]
    pub root: Option<bool>,
    /// Daemon definitions keyed by name.
    #[serde(default)]
    pub daemons: BTreeMap<String, DaemonSpec>,
}

/// A single daemon's TOML representation.
///
/// WHY: `DaemonDef` carries a compiled `Regex` and `Duration` that do not
/// deserialize cleanly; this mirror uses TOML-friendly primitives.
///
/// WHAT: The same fields as `DaemonDef`, expressed as strings/ints/maps, plus a
/// `ReadinessSpec` sub-table.
///
/// HOW: Converted to `DaemonDef` by [`DaemonSpec::into_daemon_def`], which
/// compiles the readiness regex and validates the shape.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DaemonSpec {
    /// Binary + args. `run[0]` is the program.
    pub run: Vec<String>,
    /// Working directory.
    pub cwd: Option<PathBuf>,
    /// Startup dependency names.
    #[serde(default)]
    pub depends: Vec<String>,
    /// Readiness detection sub-table.
    #[serde(default)]
    pub readiness: ReadinessSpec,
    /// File glob patterns to watch (F06).
    #[serde(default)]
    pub watch: Vec<String>,
    /// Cron schedule (F06).
    pub cron_schedule: Option<String>,
    /// Graceful-stop timeout in seconds.
    pub stop_timeout: Option<u64>,
    /// Auto-restart on exit (default `true`).
    #[serde(default = "default_restart")]
    pub restart: bool,
    /// Max restart attempts.
    pub max_restarts: Option<u32>,
    /// Environment variables (explicit values).
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Environment variable names required to be present in the process env.
    #[serde(default)]
    pub must_env: Vec<String>,
    /// Memory limit string (F07).
    pub memory_limit: Option<String>,
    /// CPU limit percentage (F07).
    pub cpu_limit: Option<u32>,
    /// Lifecycle hooks.
    pub hooks: Option<LifecycleHooks>,
    /// Boot-start flag.
    #[serde(default)]
    pub boot_start: bool,
}

const fn default_restart() -> bool {
    true
}

/// TOML representation of [`ReadinessConfig`] — a table with at most one key set.
///
/// WHY: TOML has no tagged enums; `readiness = { port = 5432 }` is the natural
/// shape.
///
/// WHAT: One optional field per readiness strategy.
///
/// HOW: [`ReadinessSpec::into_config`] picks the single set field (erroring on
/// ambiguity) and compiles the `output` regex.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReadinessSpec {
    /// Ready after this many seconds.
    pub delay_secs: Option<u64>,
    /// Ready when a line matches this regex.
    pub output: Option<String>,
    /// Ready on HTTP 2xx from this URL.
    pub http: Option<String>,
    /// Ready when this TCP port accepts a connection.
    pub port: Option<u16>,
    /// Ready when this command exits 0.
    pub cmd: Option<Vec<String>>,
}

impl ReadinessSpec {
    /// Convert this TOML spec into a compiled [`ReadinessConfig`].
    ///
    /// WHY: The supervisor needs the compiled/validated form.
    ///
    /// WHAT: Exactly one strategy, or `Immediate` when none is set.
    ///
    /// HOW: Counts set fields; more than one is [`ConfigError::AmbiguousReadiness`];
    /// an invalid `output` regex is [`ConfigError::InvalidRegex`].
    ///
    /// # Errors
    /// Returns [`ConfigError::AmbiguousReadiness`] if more than one strategy is
    /// set, or [`ConfigError::InvalidRegex`] if `output` fails to compile.
    ///
    /// # Panics
    /// Never panics.
    pub fn into_config(self) -> Result<ReadinessConfig, ConfigError> {
        let set = usize::from(self.delay_secs.is_some())
            + usize::from(self.output.is_some())
            + usize::from(self.http.is_some())
            + usize::from(self.port.is_some())
            + usize::from(self.cmd.is_some());
        if set > 1 {
            return Err(ConfigError::AmbiguousReadiness);
        }

        if let Some(secs) = self.delay_secs {
            Ok(ReadinessConfig::Delay(Duration::from_secs(secs)))
        } else if let Some(pattern) = self.output {
            let re = regex::Regex::new(&pattern)
                .map_err(|e| ConfigError::InvalidRegex(e.to_string()))?;
            Ok(ReadinessConfig::Output(re))
        } else if let Some(url) = self.http {
            Ok(ReadinessConfig::Http(url))
        } else if let Some(port) = self.port {
            Ok(ReadinessConfig::Port(port))
        } else if let Some(cmd) = self.cmd {
            Ok(ReadinessConfig::Cmd(cmd))
        } else {
            Ok(ReadinessConfig::Immediate)
        }
    }
}

impl DaemonSpec {
    /// Convert this TOML spec into a [`DaemonDef`] with the given name.
    ///
    /// WHY: `DaemonConfig` keys daemons by name; the name lives outside the spec.
    ///
    /// WHAT: A fully-built `DaemonDef`, resolving `must_env` against the process
    /// environment and compiling the readiness strategy.
    ///
    /// HOW: Copies fields across, folds `must_env` values in after explicit
    /// `env` (so an explicit value is not clobbered by a same-named required
    /// var), and calls [`ReadinessSpec::into_config`].
    ///
    /// # Errors
    /// Returns [`ConfigError::MissingEnv`] if a `must_env` variable is unset,
    /// or any error from [`ReadinessSpec::into_config`].
    ///
    /// # Panics
    /// Never panics.
    pub fn into_daemon_def(self, name: impl Into<String>) -> Result<DaemonDef, ConfigError> {
        let name = name.into();
        if self.run.is_empty() {
            return Err(ConfigError::EmptyRun(name));
        }

        let readiness = self.readiness.into_config()?;

        let mut env = self.env;
        for key in &self.must_env {
            if env.contains_key(key) {
                continue;
            }
            let value = std::env::var(key)
                .map_err(|_| ConfigError::MissingEnv(key.clone()))?;
            env.insert(key.clone(), value);
        }

        Ok(DaemonDef {
            name,
            run: self.run,
            cwd: self.cwd,
            depends: self.depends,
            readiness,
            watch: self.watch,
            cron_schedule: self.cron_schedule,
            stop_timeout: self.stop_timeout,
            restart: self.restart,
            max_restarts: self.max_restarts,
            env,
            memory_limit: self.memory_limit,
            cpu_limit: self.cpu_limit,
            hooks: self.hooks,
            boot_start: self.boot_start,
        })
    }
}

impl DaemonConfig {
    /// Flatten this config's daemon specs into `DaemonDef`s.
    ///
    /// WHY: The supervisor consumes `Vec<DaemonDef>`, not the keyed TOML map.
    ///
    /// WHAT: One `DaemonDef` per entry, in name order (the map is a `BTreeMap`).
    ///
    /// HOW: Calls [`DaemonSpec::into_daemon_def`] per entry.
    ///
    /// # Errors
    /// Propagates any [`ConfigError`] from a spec conversion.
    ///
    /// # Panics
    /// Never panics.
    pub fn into_daemon_defs(self) -> Result<Vec<DaemonDef>, ConfigError> {
        self.daemons
            .into_iter()
            .map(|(name, spec)| spec.into_daemon_def(name))
            .collect()
    }

    /// Load and layer `daemon.toml` files from `cwd` up to the filesystem root.
    ///
    /// WHY: Nearer files should override farther ones (project overrides user
    /// global), and a `daemon.local.toml` (gitignored) should override the
    /// committed `daemon.toml` at the same level.
    ///
    /// WHAT: The merged config across all discovered files; upward recursion
    /// stops at a file whose `root = true`.
    ///
    /// HOW: Walks `cwd.ancestors()` collecting configs (nearest first), stopping
    /// at `root = true`, then folds farthest→nearest so nearer keys win.
    ///
    /// # Errors
    /// Returns [`ConfigError::Io`] or [`ConfigError::Toml`] on read/parse failure.
    ///
    /// # Panics
    /// Never panics.
    pub fn load_from_path(cwd: &Path) -> Result<Self, ConfigError> {
        let mut configs = Vec::new();
        for dir in cwd.ancestors() {
            if let Some(path) = Self::find_config(dir) {
                let cfg = Self::load_file(&path)?;
                let is_root = cfg.root == Some(true);
                configs.push(cfg);
                if is_root {
                    break;
                }
            }
        }

        // `configs` is nearest-first; fold farthest-first so nearer keys win.
        Ok(configs
            .into_iter()
            .rev()
            .reduce(DaemonConfig::merge)
            .unwrap_or_default())
    }

    /// Find the config file in a directory: `daemon.local.toml` beats `daemon.toml`.
    ///
    /// # Panics
    /// Never panics.
    fn find_config(dir: &Path) -> Option<PathBuf> {
        let local = dir.join("daemon.local.toml");
        if local.is_file() {
            return Some(local);
        }
        let main = dir.join("daemon.toml");
        if main.is_file() {
            return Some(main);
        }
        None
    }

    /// Read and parse a single TOML config file.
    ///
    /// # Errors
    /// Returns [`ConfigError::Io`] on read failure or [`ConfigError::Toml`] on
    /// parse failure.
    fn load_file(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Io(format!("{}: {e}", path.display())))?;
        toml::from_str(&text).map_err(|e| ConfigError::Toml(format!("{}: {e}", path.display())))
    }

    /// Fold the next (nearer, higher-priority) config over the accumulator.
    ///
    /// WHY: Layering means nearer files win per-daemon and per-scalar.
    ///
    /// WHAT: `next`'s daemons/namespace/root override the accumulator's on
    /// collision; unset scalars leave the accumulator's values intact.
    ///
    /// HOW: `load_from_path` reverses the nearest-first list to farthest-first,
    /// so `reduce` calls `merge(acc, next)` with `next` always the nearer file —
    /// hence the **second** argument wins.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    fn merge(mut acc: Self, next: Self) -> Self {
        for (name, spec) in next.daemons {
            acc.daemons.insert(name, spec);
        }
        if next.namespace.is_some() {
            acc.namespace = next.namespace;
        }
        if next.root.is_some() {
            acc.root = next.root;
        }
        acc
    }
}
