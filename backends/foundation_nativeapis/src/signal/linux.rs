// Linux signal handling — eventfd + sigaction + epoll.

use std::os::fd::RawFd;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::signal::error::SignalError;
use crate::signal::event::SignalKind;
use crate::signal::handle::SignalHandle;
use crate::signal::note_signal;

// Global eventfd — set once at startup.
static EVENT_FD: AtomicU64 = AtomicU64::new(0);

/// Signal handler — records the signal and writes the eventfd.
///
/// Both calls are async-signal-safe: one lock-free atomic OR and one syscall.
/// The handler does not deliver to the bus; the readiness poll does that.
extern "C" fn signal_handler(signum: libc::c_int) {
    let kind = match signum {
        libc::SIGINT => SignalKind::Interrupt,
        libc::SIGTERM => SignalKind::Terminate,
        libc::SIGHUP => SignalKind::Hangup,
        libc::SIGQUIT => SignalKind::Quit,
        _ => return,
    };

    // Record first: a poll that sees the eventfd ready must find a set bit.
    note_signal(kind);

    // Wake the poll. A zero fd is the unregistered state, so test `> 0`; fd 0
    // is usually the standard input.
    let fd = EVENT_FD.load(Ordering::Relaxed) as RawFd;
    if fd > 0 {
        let buf: u64 = 1;
        unsafe {
            libc::write(fd, (&raw const buf).cast(), 8);
        }
    }
}

fn install_handler(signum: libc::c_int) -> Result<(), SignalError> {
    let mut sa: libc::sigaction = unsafe { std::mem::zeroed() };
    sa.sa_sigaction = signal_handler as *const () as libc::sighandler_t;
    unsafe { libc::sigfillset(&raw mut sa.sa_mask) };
    sa.sa_flags = 0; // SA_RESTART not set — we want interruptible waits

    let ret = unsafe { libc::sigaction(signum, &raw const sa, std::ptr::null_mut()) };
    if ret != 0 {
        return Err(SignalError::RegistrationFailed(format!(
            "sigaction({}) failed: {}",
            signum,
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

/// Register signal handlers for SIGINT, SIGTERM, SIGHUP, SIGQUIT.
///
/// # Errors
/// Returns an error if eventfd creation or sigaction fails.
pub fn register_signals() -> Result<SignalHandle, SignalError> {
    // Create eventfd (non-blocking)
    let event_fd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
    if event_fd < 0 {
        return Err(SignalError::Io(std::io::Error::last_os_error()));
    }

    // Store the eventfd for the handler. A failed conversion must not fall back
    // to the unregistered state, because the handler would then not wake a poll.
    let stored_fd = u64::try_from(event_fd).map_err(|_| {
        SignalError::RegistrationFailed(format!("eventfd {event_fd} does not fit in u64"))
    })?;
    EVENT_FD.store(stored_fd, Ordering::Relaxed);

    // Create epoll fd
    let epoll_fd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
    if epoll_fd < 0 {
        return Err(SignalError::Io(std::io::Error::last_os_error()));
    }

    // Register eventfd with epoll
    let mut event = libc::epoll_event {
        events: (libc::EPOLLIN) as u32,
        u64: 0,
    };
    let ret = unsafe { libc::epoll_ctl(epoll_fd, libc::EPOLL_CTL_ADD, event_fd, &raw mut event) };
    if ret != 0 {
        return Err(SignalError::Io(std::io::Error::last_os_error()));
    }

    // Install signal handlers
    install_handler(libc::SIGINT)?;
    install_handler(libc::SIGTERM)?;
    install_handler(libc::SIGHUP)?;
    install_handler(libc::SIGQUIT)?;

    Ok(SignalHandle::Linux { event_fd, epoll_fd })
}
