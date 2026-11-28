//! Local service emulators that the tests start and stop by task.
//!
//! A test that needs a cloud service runs it on the same machine where a free
//! continuous integration (CI) job can also run it. This module owns those
//! processes: it starts one, waits for its port, and stops it by the process
//! ID that it saved.

mod localgcp;

use foundation_tasks::{TaskError, TaskSet};

/// The `services` task namespace.
///
/// # Errors
/// Returns an error when two commands share a name.
pub fn set() -> Result<TaskSet, TaskError> {
    let mut set = TaskSet::new("foundation_ci_services").with_namespace("services");
    localgcp::register(&mut set)?;
    Ok(set)
}
