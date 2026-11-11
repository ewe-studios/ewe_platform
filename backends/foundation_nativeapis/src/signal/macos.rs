// macOS signal handling — kqueue EVFILT_SIGNAL.

use crate::signal::error::SignalError;
use crate::signal::handle::SignalHandle;

/// A handler that stops the default action and does nothing else.
///
/// The kernel still posts the `EVFILT_SIGNAL` event, so the readiness poll
/// delivers the signal. The handler does nothing, so it is async-signal-safe.
extern "C" fn stop_default_action(_signum: libc::c_int) {}

/// Install the no-op handler for one signal.
fn install_handler(signum: libc::c_int) -> Result<(), SignalError> {
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    // SAFETY: a handler function pointer has the `sighandler_t` type.
    action.sa_sigaction = stop_default_action as *const () as libc::sighandler_t;
    unsafe { libc::sigfillset(&raw mut action.sa_mask) };
    action.sa_flags = 0;

    let result = unsafe { libc::sigaction(signum, &raw const action, std::ptr::null_mut()) };
    if result != 0 {
        return Err(SignalError::RegistrationFailed(format!(
            "sigaction({signum}) failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

/// Register signal handlers for SIGINT, SIGTERM, SIGHUP, SIGQUIT via kqueue.
///
/// # Errors
/// Returns an error if kqueue creation or kevent registration fails, or if a
/// `sigaction` call fails.
pub fn register_signals() -> Result<SignalHandle, SignalError> {
    let kq_fd = unsafe { libc::kqueue() };
    if kq_fd < 0 {
        return Err(SignalError::Io(std::io::Error::last_os_error()));
    }

    // Register signals with kqueue
    let signals = [libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGQUIT];

    for &signum in &signals {
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        // Signal numbers are positive, so this conversion cannot fail.
        event.ident = usize::try_from(signum).unwrap_or_default();
        event.filter = libc::EVFILT_SIGNAL;
        event.flags = libc::EV_ADD;
        event.fflags = 0;
        event.data = 0;
        event.udata = std::ptr::null_mut();

        let ret = unsafe {
            libc::kevent(
                kq_fd,
                &raw const event,
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

    // `EVFILT_SIGNAL` reports a signal but does not suppress its default
    // action: without a handler, SIGTERM still terminates the process and the
    // readiness poll never sees the event. Install a no-op handler for each
    // signal, so the default action does not run.
    for &signum in &signals {
        install_handler(signum)?;
    }

    Ok(SignalHandle::Macos { kq_fd })
}
