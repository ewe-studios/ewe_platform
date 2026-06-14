// macOS signal handling — kqueue EVFILT_SIGNAL.

use crate::signal::error::SignalError;
use crate::signal::handle::SignalHandle;

/// Register signal handlers for SIGINT, SIGTERM, SIGHUP, SIGQUIT via kqueue.
///
/// # Errors
/// Returns an error if kqueue creation or kevent registration fails.
pub fn register_signals() -> Result<SignalHandle, SignalError> {
    let kq_fd = unsafe { libc::kqueue() };
    if kq_fd < 0 {
        return Err(SignalError::Io(std::io::Error::last_os_error()));
    }

    // Register signals with kqueue
    let signals = [libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGQUIT];

    for &signum in &signals {
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        event.ident = signum as _;
        event.filter = libc::EVFILT_SIGNAL;
        event.flags = libc::EV_ADD;
        event.fflags = 0;
        event.data = 0;
        event.udata = std::ptr::null_mut();

        let ret = unsafe {
            libc::kevent(
                kq_fd,
                &event,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };
        if ret < 0 {
            return Err(SignalError::Io(std::io::Error::last_os_error()));
        }
    }

    Ok(SignalHandle::Macos { kq_fd })
}
