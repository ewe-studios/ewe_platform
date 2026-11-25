//! The child table and the reaper.
//!
//! Only this module calls `waitpid`. Every child is reaped, including the
//! orphans that process identifier 1 adopts.

use std::collections::HashMap;
use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::{Arc, Mutex, MutexGuard};

use foundation_reactor::fd_readiness::FdReadiness;

use super::exec::Session;
use super::sys;

/// The children of the slave, by process identifier.
///
/// `fork` holds this lock across the fork and the insert, so a child that exits
/// at once is in the table before the reaper looks.
#[derive(Default)]
pub struct ChildTable {
    children: Mutex<HashMap<i32, Arc<Session>>>,
}

impl ChildTable {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes the table lock.
    ///
    /// # Panics
    /// Only when a thread that held the lock panicked.
    pub fn lock(&self) -> MutexGuard<'_, HashMap<i32, Arc<Session>>> {
        self.children
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Gives the status of `pid` to its session and removes the entry.
    ///
    /// A process that is not in the table is an orphan: its status is dropped.
    pub fn deliver(&self, pid: i32, code: Option<i32>, signal: Option<i32>) {
        let session = self.lock().remove(&pid);
        if let Some(session) = session {
            session.set_exit(code, signal);
        }
    }

    /// The number of children in the table.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether the table holds no child.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }
}

/// Opens the `SIGCHLD` descriptor and starts the reaper task on the valtron
/// pool.
///
/// # Errors
/// The operating system error of `signalfd`, or of its readiness.
pub fn start(table: Arc<ChildTable>) -> io::Result<Arc<FdReadiness>> {
    let signal = sys::signalfd()?;
    // The shared selector is edge-triggered. The reaper clears the readiness
    // before each signalfd read, not before the wait; a clear before the wait
    // would erase the pending `SIGCHLD` edge and park for ever.
    let readiness = Arc::new(FdReadiness::register(signal.as_raw_fd())?);
    spawn_reaper(signal, Arc::clone(&readiness), table);
    Ok(readiness)
}

/// Starts the reaper task.
fn spawn_reaper(signal: OwnedFd, readiness: Arc<FdReadiness>, table: Arc<ChildTable>) {
    let future = run(signal, readiness, table);
    if let Err(error) =
        foundation_core::valtron::send(foundation_core::valtron::from_future(future))
    {
        tracing::error!(error = %error, "the slave could not start the reaper task");
    }
}

/// Waits for `SIGCHLD` and collects every child that changed state.
async fn run(signal: OwnedFd, readiness: Arc<FdReadiness>, table: Arc<ChildTable>) {
    let mut buffer = [0_u8; 128];
    loop {
        super::wait::ready(Arc::clone(&readiness)).await;
        // Clear before the read: the clear consumes the edge this wait returned
        // for, and the read consumes the pending signal.
        readiness.clear();
        // The descriptor stays readable while notifications wait; one read
        // takes them all, and the status comes from `waitpid`.
        let _ = sys::read(signal.as_raw_fd(), &mut buffer);
        loop {
            match sys::waitpid_nohang() {
                Ok(sys::WaitResult::Changed { pid, status }) => {
                    let (code, signal) = sys::decode_status(status);
                    table.deliver(pid, code, signal);
                }
                Ok(sys::WaitResult::None | sys::WaitResult::NoChildren) => break,
                Err(error) => {
                    tracing::warn!(error = %error, "the reaper could not call waitpid");
                    break;
                }
            }
        }
    }
}
