/// Valtron task: `FdMonitorTask` — wraps a `RegisteredFd` and invokes a callback
/// when the fd becomes readable/writable.
use std::io;
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::valtron::{
    EventReadiness, ReadinessInstant, ReadinessWaker, TaskIterator, TaskStatus,
};

use super::super::FdState;
use super::super::StopSignal;
use crate::native::fd::{PollResult, Ready, RegisteredFd};
use crate::native::poll::Interest;

/// A valtron task that monitors a `RegisteredFd` for readiness.
///
/// When the fd is ready, invokes the user-provided callback with a reference
/// to the inner IO object. Uses `TaskStatus::Depends` so the executor parks
/// the task and only wakes it when the OS signals fd readiness (epoll/kqueue),
/// avoiding wasteful periodic polling.
pub struct FdMonitorTask<T: AsRawFd> {
    fd: Arc<RegisteredFd<T>>,
    callback: Option<MonitorCallback<T>>,
    interest: Interest,
    stop: StopSignal,
}

/// The fd-ready callback held by [`FdMonitorTask`].
type MonitorCallback<T> = Box<dyn FnMut(&T) -> io::Result<()> + Send>;

impl<T: AsRawFd> FdMonitorTask<T> {
    /// Create a new `FdMonitorTask` wrapping the given `RegisteredFd`.
    pub fn new(fd: RegisteredFd<T>) -> Self {
        Self {
            fd: Arc::new(fd),
            callback: None,
            interest: Interest::READABLE,
            stop: StopSignal::new(),
        }
    }

    /// Set the callback to invoke when the fd becomes readable.
    #[must_use]
    pub fn with_callback(
        mut self,
        callback: impl FnMut(&T) -> io::Result<()> + Send + 'static,
    ) -> Self {
        self.callback = Some(Box::new(callback));
        self
    }

    /// Set the interest to poll for. Default: `Interest::READABLE`.
    #[must_use]
    pub fn with_interest(mut self, interest: Interest) -> Self {
        self.interest = interest;
        self
    }

    /// Get a `StopSignal` that can terminate this task when signaled.
    #[must_use]
    pub fn stop_signal(&self) -> StopSignal {
        self.stop.clone()
    }

    /// Get a reference to the inner `RegisteredFd`.
    #[must_use]
    pub fn fd(&self) -> &RegisteredFd<T> {
        &self.fd
    }

    /// Get a mutable reference to the inner `RegisteredFd`.
    ///
    /// # Panics
    /// Panics if another handle to the fd still exists.
    pub fn fd_mut(&mut self) -> &mut RegisteredFd<T> {
        Arc::get_mut(&mut self.fd).expect("FdMonitorTask: fd_mut called while shared handle exists")
    }
}

/// Readiness a parked [`FdMonitorTask`] hands to the executor.
///
/// The executor requires a `Send + Sync` readiness pointer. `RegisteredFd<T>`
/// is `Send + Sync` when `T` is, and `StopSignal` always is. This type keeps
/// both as concrete fields, so the compiler proves the bound.
struct FdMonitorReadiness<T: AsRawFd> {
    fd: Arc<RegisteredFd<T>>,
    stop: StopSignal,
}

impl<T: AsRawFd + Send + Sync> EventReadiness for FdMonitorReadiness<T> {
    /// Ready when the fd is ready or the task was stopped.
    fn is_ready(&self, dur: Option<Duration>) -> bool {
        self.fd.is_ready(dur) || self.stop.is_ready(dur)
    }

    fn deadline(&self) -> Option<ReadinessInstant> {
        match (self.fd.deadline(), self.stop.deadline()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Registers the waker with both signals; `true` only if both wake.
    fn register(&self, waker: &ReadinessWaker) -> bool {
        let a = self.fd.register(waker);
        let b = self.stop.register(waker);
        a && b
    }
}

impl<T: AsRawFd + Send + Sync + 'static> TaskIterator for FdMonitorTask<T> {
    type Ready = FdState;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.stop.is_stopped() {
            return None;
        }

        let readiness = match self.interest {
            Interest::READABLE => self.fd.poll_readable(),
            Interest::WRITABLE => self.fd.poll_writable(),
            _ => self.fd.poll_ready(self.interest),
        };

        match readiness {
            PollResult::Ready(mut guard) => {
                let r = guard.ready();
                let state = fd_state_from_ready(r);
                if let Some(ref mut cb) = self.callback {
                    if let Ok(Err(e)) = guard.try_io(|fd| cb(fd.get_ref())) {
                        tracing::error!("FdMonitorTask callback I/O error: {}", e);
                    }
                }
                let state = state.unwrap_or(FdState::Readable);
                Some(TaskStatus::Ready(state))
            }
            PollResult::NotReady => Some(TaskStatus::Depends(Arc::new(FdMonitorReadiness {
                fd: Arc::clone(&self.fd),
                stop: self.stop.clone(),
            }))),
            PollResult::Error(e) => {
                tracing::error!("FdMonitorTask poll error: {}", e);
                None
            }
        }
    }
}

/// Convert a `Ready` bitmask to `FdState`, keeping only readable/writable.
fn fd_state_from_ready(r: Ready) -> Option<FdState> {
    FdState::from_ready(r.is_readable(), r.is_writable())
}
