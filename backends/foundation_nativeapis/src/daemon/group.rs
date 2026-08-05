//! Grouped daemon lifecycle: `DaemonGroup` and per-daemon `DaemonHandle`.
//!
//! WHY: Callers want one handle that boots a set of daemons in dependency order
//! and cleans them all up on drop — the RAII surface the macros and builder API
//! expand into.
//!
//! WHAT: [`DaemonGroup`] (owns the supervisor; `Drop` tears everything down) and
//! [`DaemonHandle`] (a live view of one daemon, queried through the supervisor).
//!
//! HOW: `boot` builds a [`Supervisor`], starts all daemons, and returns the
//! group. Handles hold the supervisor `Arc` + id and query live state, so a
//! `pid()`/`status()` call always reflects the current reality (not a stale
//! snapshot).

use std::sync::Arc;

use super::config::DaemonDef;
use super::error::{DaemonError, StopError};
use super::id::{DaemonId, DaemonStatus};
use super::supervisor::Supervisor;

/// A handle to a booted group of daemons.
///
/// See the module docs. On `Drop`, all daemons stop in reverse dependency order.
pub struct DaemonGroup {
    supervisor: Arc<Supervisor>,
}

impl DaemonGroup {
    /// Boot all daemons from their definitions in dependency order.
    ///
    /// WHY: The single entry point for the builder API and macro expansions.
    ///
    /// WHAT: A group whose daemons are all running and `Ready`; the crash
    /// monitor is active.
    ///
    /// HOW: Builds a [`Supervisor`] (validating the dependency graph) and calls
    /// `start_all`. Synchronous by design — see the supervisor module docs.
    ///
    /// # Errors
    /// Returns [`DaemonError`] on a dependency cycle, spawn failure, or readiness
    /// timeout.
    ///
    /// # Panics
    /// Never panics.
    pub fn boot(definitions: Vec<DaemonDef>) -> Result<Self, DaemonError> {
        let supervisor = Supervisor::new(definitions)?;
        supervisor.start_all()?;
        Ok(Self { supervisor })
    }

    /// Boot all daemons under an explicit namespace.
    ///
    /// # Errors
    /// As [`DaemonGroup::boot`].
    ///
    /// # Panics
    /// Never panics.
    pub fn boot_in(
        namespace: impl Into<String>,
        definitions: Vec<DaemonDef>,
    ) -> Result<Self, DaemonError> {
        let supervisor = Supervisor::with_namespace(namespace, definitions)?;
        supervisor.start_all()?;
        Ok(Self { supervisor })
    }

    /// Get a handle to a daemon by name.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn daemon(&self, name: &str) -> Option<DaemonHandle> {
        let id = self.supervisor.id_for(name);
        self.supervisor.snapshot(&id).map(|_| DaemonHandle {
            id,
            supervisor: self.supervisor.clone(),
        })
    }

    /// Get handles to all daemons, in name order.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn daemons(&self) -> Vec<DaemonHandle> {
        self.supervisor
            .ids()
            .into_iter()
            .map(|id| DaemonHandle {
                id,
                supervisor: self.supervisor.clone(),
            })
            .collect()
    }

    /// Restart a daemon by name.
    ///
    /// # Errors
    /// Returns [`DaemonError::NotFound`] if no such daemon, else any restart error.
    ///
    /// # Panics
    /// Never panics.
    pub fn restart(&self, name: &str) -> Result<(), DaemonError> {
        let id = self.supervisor.id_for(name);
        if self.supervisor.snapshot(&id).is_none() {
            return Err(DaemonError::NotFound(name.into()));
        }
        self.supervisor.restart_daemon(&id)
    }

    /// Explicitly stop all daemons now (also runs on `Drop`).
    ///
    /// # Panics
    /// Never panics.
    pub fn shutdown(&self) {
        self.supervisor.shutdown();
    }

    /// Access the underlying supervisor (for RPC control in F03).
    #[must_use]
    pub fn supervisor(&self) -> &Arc<Supervisor> {
        &self.supervisor
    }
}

impl Drop for DaemonGroup {
    fn drop(&mut self) {
        // Synchronous reverse-order teardown — a leaked child is worse than a
        // brief blocking drop.
        self.supervisor.shutdown();
    }
}

/// A live handle to one daemon within a group.
///
/// See the module docs — state is queried through the supervisor on each call.
pub struct DaemonHandle {
    id: DaemonId,
    supervisor: Arc<Supervisor>,
}

impl DaemonHandle {
    /// This daemon's id.
    #[must_use]
    pub fn id(&self) -> &DaemonId {
        &self.id
    }

    /// The running process's pid, if any (live query).
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.supervisor.snapshot(&self.id).and_then(|m| m.pid)
    }

    /// The daemon's current lifecycle status (live query).
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn status(&self) -> DaemonStatus {
        self.supervisor
            .snapshot(&self.id)
            .map_or(DaemonStatus::Pending, |m| m.status)
    }

    /// Restart this daemon.
    ///
    /// # Errors
    /// Any [`DaemonError`] from the restart.
    ///
    /// # Panics
    /// Never panics.
    pub fn restart(&self) -> Result<(), DaemonError> {
        self.supervisor.restart_daemon(&self.id)
    }

    /// Stop this daemon.
    ///
    /// # Errors
    /// Any [`StopError`] from the stop.
    ///
    /// # Panics
    /// Never panics.
    pub fn stop(&self) -> Result<(), StopError> {
        self.supervisor.stop_daemon(&self.id)
    }
}
