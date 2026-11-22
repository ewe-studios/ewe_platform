//! Plan continuous integration checks from Cargo metadata and package policy.
mod affected;
mod config;
mod execution;
mod plan;
mod workspace;

pub use config::{ExpectedError, GroupRecipe, Settings};
use foundation_tasks::{TaskError, TaskSet};
pub use workspace::{Package, Workspace};

/// Register planning and execution commands for both command hosts.
///
/// # Errors
/// Returns an error when two commands share a name or path.
pub fn set() -> Result<TaskSet, TaskError> {
    let mut set = TaskSet::new("foundation_ci").with_namespace("ci");
    plan::register(&mut set)?;
    execution::register(&mut set)?;
    Ok(set)
}

use foundation_tasks::{TaskResult, TaskRunner};
use std::{ffi::OsString, path::PathBuf, sync::Arc};

type EnvironmentLookup = dyn Fn(&str) -> Option<OsString> + Send + Sync;

/// Location and environment of the host that starts a task.
pub struct CiEnvironment {
    pub root: PathBuf,
    pub binary: PathBuf,
    lookup: Arc<EnvironmentLookup>,
}
impl CiEnvironment {
    pub fn new(
        root: PathBuf,
        binary: PathBuf,
        lookup: impl Fn(&str) -> Option<OsString> + Send + Sync + 'static,
    ) -> Self {
        Self {
            root,
            binary,
            lookup: Arc::new(lookup),
        }
    }
    /// Read the current directory and executable path.
    ///
    /// # Errors
    /// Returns an error if either path is unavailable.
    pub fn from_workspace() -> Result<Self, TaskError> {
        Ok(Self::new(
            std::env::current_dir().map_err(TaskError::operation)?,
            std::env::current_exe().map_err(TaskError::operation)?,
            |k| std::env::var_os(k),
        ))
    }
    #[must_use]
    pub fn get(&self, key: &str) -> Option<OsString> {
        (self.lookup)(key)
    }
    pub(crate) fn text(&self, key: &str) -> Option<String> {
        self.get(key)
            .and_then(|s| s.into_string().ok())
            .filter(|s| !s.is_empty())
    }
}
/// Install host services once before dispatch.
///
/// # Errors
/// Returns an error when the host paths are unavailable.
pub fn install_host(runner: &mut TaskRunner) -> TaskResult {
    if runner.context().get::<CiEnvironment>().is_none() {
        runner.context().store(CiEnvironment::from_workspace()?);
    }
    Ok(())
}
