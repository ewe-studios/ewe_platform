//! Daemon identity and status.
//!
//! WHY: Daemons are addressed by a `namespace/name` pair so groups loaded from
//! different config files (or programmatically) can cross-reference without
//! collisions, and the supervisor tracks a small lifecycle state machine.
//!
//! WHAT: `DaemonId` (namespace + name, ordered for stable `BTreeMap` keys) and
//! `DaemonStatus` (the lifecycle states a managed daemon moves through).
//!
//! HOW: Both are plain `Copy`/`Clone` value types with `Display` impls for logs.

use std::fmt;

/// The default namespace used when a group is built without an explicit one.
pub const DEFAULT_NAMESPACE: &str = "default";

/// A daemon's unique identity within (and across) groups.
///
/// WHY: Names alone collide across groups; the namespace disambiguates and lets
/// a dependency reference another group's daemon via `namespace/name`.
///
/// WHAT: An ordered `(namespace, name)` pair.
///
/// HOW: `Ord`/`Eq` derive over the two strings so it is a stable `BTreeMap` key;
/// `Display` renders `namespace/name`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DaemonId {
    /// The namespace this daemon belongs to.
    pub namespace: String,
    /// The daemon's name, unique within its namespace.
    pub name: String,
}

impl DaemonId {
    /// Construct a `DaemonId` from namespace and name.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
        }
    }

    /// Construct a `DaemonId` in the [`DEFAULT_NAMESPACE`].
    ///
    /// # Panics
    /// Never panics.
    pub fn in_default(name: impl Into<String>) -> Self {
        Self::new(DEFAULT_NAMESPACE, name)
    }
}

impl fmt::Display for DaemonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.namespace, self.name)
    }
}

/// The lifecycle state of a managed daemon.
///
/// WHY: The supervisor, RPC status queries (F03), and restart logic all branch
/// on where a daemon is in its lifecycle.
///
/// WHAT: The states a daemon moves through from spawn to teardown.
///
/// HOW: A `Copy` enum with a `Display` impl for logs and RPC responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonStatus {
    /// Defined but not yet spawned.
    Pending,
    /// Process spawned, readiness not yet confirmed.
    Starting,
    /// Readiness confirmed — serving.
    Ready,
    /// Being stopped (SIGTERM sent, awaiting exit).
    Stopping,
    /// Not running (clean exit or stopped).
    Stopped,
    /// Waiting out a restart backoff before respawning.
    Restarting,
    /// Gave up after exceeding the restart limit.
    Failed,
}

impl fmt::Display for DaemonStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Pending => "pending",
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Restarting => "restarting",
            Self::Failed => "failed",
        };
        f.write_str(s)
    }
}
