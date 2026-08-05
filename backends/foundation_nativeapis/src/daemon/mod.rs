//! DaemonMaster — a process supervisor for `foundation_nativeapis` (spec-58).
//!
//! WHY: Applications and dev tooling need to keep a set of child processes alive
//! with dependency ordering, readiness detection, and auto-restart — the union
//! of what `pitchfork` (daemon lifecycle) and `ecdysis` (graceful restart) offer.
//!
//! WHAT: The `daemon` feature provides:
//! - [`DaemonDef`] — one definition type built via builder, `daemons!` macro, or
//!   TOML ([`DaemonConfig`]).
//! - [`DaemonGroup`] / [`DaemonHandle`] — grouped RAII lifecycle.
//! - [`Supervisor`] — the engine: spawn, readiness, monitor, restart, teardown.
//! - Dependency graph helpers ([`startup_order`], [`shutdown_order`]).
//!
//! HOW: The public API is synchronous; internally all supervision runs on the
//! valtron pool (readiness via `execute`/`collect_one`, monitoring and restart
//! backoff via the background job pool). See [`supervisor`] for the rationale.
//!
//! Later features extend this module: ConnectRPC control (F03), PID 1 mode
//! (F04), graceful restart (F05), file watching (F06), resource monitoring
//! (F07), and boot registration (F09).

pub mod config;
pub mod deps;
pub mod error;
pub mod group;
pub mod id;
pub mod platform;
pub mod process;
pub mod supervisor;

pub use config::{DaemonConfig, DaemonDef, DaemonSpec, LifecycleHooks, ReadinessConfig, ReadinessSpec};
pub use deps::{resolve_id, shutdown_order, startup_order};
pub use error::{
    ConfigError, CycleError, DaemonError, ReadinessTimeout, SpawnError, StopError,
};
pub use group::{DaemonGroup, DaemonHandle};
pub use id::{DaemonId, DaemonStatus, DEFAULT_NAMESPACE};
pub use process::{ManagedDaemon, OutputLine, ReadinessTask, StreamKind, TimerReadiness};
pub use supervisor::Supervisor;
