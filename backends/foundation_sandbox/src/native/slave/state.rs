//! The slave state and the record of `Prepare`.

use std::path::PathBuf;

use crate::shared::rpc::messages::{PrepareRequest, SlaveLimits};
use crate::shared::spec::RootMode;

/// Where the slave is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The process started and `Prepare` did not succeed.
    Booted,
    /// `Prepare` succeeded and programs may run.
    Prepared,
    /// `Shutdown` was accepted. No new session or upload starts.
    ShuttingDown,
}

/// What `Prepare` fixed for the rest of the life of the slave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrepareRecord {
    /// The request, kept to compare a second `Prepare` with the first.
    pub request: PrepareRequest,
    /// The root mode that the slave applied.
    pub root_mode: RootMode,
    /// The directory that each program gets as `/`.
    pub exec_root: PathBuf,
}

impl PrepareRecord {
    /// The work directory of the programs.
    #[must_use]
    pub fn workdir(&self) -> &str {
        &self.request.workdir
    }

    /// The base environment of the programs.
    #[must_use]
    pub fn env(&self) -> &[(String, String)] {
        &self.request.env
    }

    /// The limits of the slave.
    #[must_use]
    pub fn limits(&self) -> SlaveLimits {
        self.request.limits
    }
}

/// The state of the slave, protected by one lock.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// Where the slave is in its life.
    pub phase: Option<Phase>,
    /// The `Prepare` record, when `Prepare` succeeded.
    pub prepare: Option<PrepareRecord>,
}

impl State {
    /// A slave that did not get `Prepare`.
    #[must_use]
    pub fn booted() -> Self {
        Self {
            phase: Some(Phase::Booted),
            prepare: None,
        }
    }

    /// The phase, or [`Phase::Booted`] before the first write.
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase.unwrap_or(Phase::Booted)
    }

    /// Whether `Prepare` succeeded.
    #[must_use]
    pub fn is_prepared(&self) -> bool {
        self.prepare.is_some() && self.phase() != Phase::Booted
    }
}
