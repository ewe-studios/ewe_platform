//! Error types for the daemon supervisor.
//!
//! WHY: Config parsing, dependency resolution, spawning, stopping, and readiness
//! each fail differently; callers need to distinguish (e.g. a missing secret vs
//! a dependency cycle vs a spawn `ENOENT`).
//!
//! WHAT: `ConfigError`, `CycleError`, `SpawnError`, `StopError`, `ReadinessTimeout`,
//! and the umbrella `DaemonError` returned by group/supervisor operations.
//!
//! HOW: `thiserror`-derived enums matching this crate's existing `WatchError`
//! style; `DaemonError` `#[from]`-converts the specific errors so `?` composes.

use thiserror::Error;

use super::id::DaemonId;

/// Errors from loading, parsing, or validating daemon configuration.
#[derive(Error, Debug)]
pub enum ConfigError {
    /// A config file could not be read.
    #[error("config I/O error: {0}")]
    Io(String),

    /// A config file could not be parsed as TOML.
    #[error("config parse error: {0}")]
    Toml(String),

    /// A daemon spec had an empty `run` command.
    #[error("daemon {0:?} has an empty `run` command")]
    EmptyRun(String),

    /// More than one readiness strategy was set on a single daemon.
    #[error("ambiguous readiness: set exactly one of delay_secs/output/http/port/cmd")]
    AmbiguousReadiness,

    /// A readiness `output` regex failed to compile.
    #[error("invalid readiness regex: {0}")]
    InvalidRegex(String),

    /// A `must_env` variable was not present in the process environment.
    #[error("required env var {0:?} is not set")]
    MissingEnv(String),
}

/// A dependency cycle among daemons — the involved members are listed.
#[derive(Error, Debug, Clone)]
#[error("dependency cycle among daemons: {}", format_cycle(.cycle))]
pub struct CycleError {
    /// The daemons that could not be ordered (their in-degree never reached 0).
    pub cycle: Vec<DaemonId>,
}

fn format_cycle(cycle: &[DaemonId]) -> String {
    cycle
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Errors from spawning a daemon process.
#[derive(Error, Debug)]
pub enum SpawnError {
    /// No daemon with this id exists in the config.
    #[error("daemon not found: {0}")]
    NotFound(DaemonId),

    /// The child process could not be spawned.
    #[error("failed to spawn {id}: {source}")]
    Io {
        /// The daemon that failed to spawn.
        id: DaemonId,
        /// The underlying OS error.
        source: std::io::Error,
    },
}

/// Errors from stopping a daemon process.
#[derive(Error, Debug)]
pub enum StopError {
    /// No daemon with this id is managed.
    #[error("daemon not found: {0}")]
    NotFound(DaemonId),

    /// The daemon has no running process to stop.
    #[error("daemon not running: {0}")]
    NotRunning(DaemonId),

    /// A signalling syscall failed.
    #[error("failed to signal {id}: {message}")]
    Signal {
        /// The daemon that could not be signalled.
        id: DaemonId,
        /// The underlying error message.
        message: String,
    },
}

/// A readiness strategy did not succeed within its timeout.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error("daemon {id} did not become ready within {timeout_secs}s")]
pub struct ReadinessTimeout {
    /// The daemon that timed out.
    pub id: DaemonId,
    /// The timeout that elapsed, in seconds.
    pub timeout_secs: u64,
}

/// Umbrella error for group/supervisor operations.
#[derive(Error, Debug)]
pub enum DaemonError {
    /// Lookup by name found no matching daemon.
    #[error("daemon not found: {0}")]
    NotFound(String),

    /// A configuration error occurred.
    #[error(transparent)]
    Config(#[from] ConfigError),

    /// A dependency cycle was detected.
    #[error(transparent)]
    Cycle(#[from] CycleError),

    /// A daemon failed to spawn.
    #[error(transparent)]
    Spawn(#[from] SpawnError),

    /// A daemon failed to stop.
    #[error(transparent)]
    Stop(#[from] StopError),

    /// A daemon did not become ready in time.
    #[error(transparent)]
    Readiness(#[from] ReadinessTimeout),
}
