// SignalHandle — platform-specific signal readiness tracker.
// Implements EventReadiness for valtron Depends parking.

use foundation_core::valtron::EventReadiness;

#[cfg(target_os = "macos")]
use crate::signal::event::SignalKind;


/// Platform-specific signal handle.
///
/// On Linux: wraps eventfd + epoll_fd.
/// On macOS: wraps kqueue fd.
/// On Windows: wraps HANDLE (event object).
pub enum SignalHandle {
    /// Not yet initialized (stub for unsupported platforms).
    None,
    /// Linux: eventfd for signal notification, epoll_fd for waiting.
    #[cfg(target_os = "linux")]
    Linux {
        event_fd: std::os::fd::RawFd,
        epoll_fd: std::os::fd::RawFd,
    },
    /// macOS: kqueue fd with EVFILT_SIGNAL registrations.
    #[cfg(target_os = "macos")]
    Macos {
        kq_fd: std::os::fd::RawFd,
    },
    /// Windows: event HANDLE.
    #[cfg(target_os = "windows")]
    Windows {
        event: windows_sys::Win32::Foundation::HANDLE,
    },
}

// SAFETY: every variant holds a thread-safe kernel handle — a Unix `RawFd`
// (a plain `i32`) or a Windows `HANDLE` (an opaque pointer-sized value the
// Win32 APIs accept from any thread). `None` carries nothing.
unsafe impl Send for SignalHandle {}
unsafe impl Sync for SignalHandle {}

impl Clone for SignalHandle {
    fn clone(&self) -> Self {
        match self {
            Self::None => Self::None,
            #[cfg(target_os = "linux")]
            Self::Linux { event_fd, epoll_fd } => Self::Linux {
                event_fd: *event_fd,
                epoll_fd: *epoll_fd,
            },
            #[cfg(target_os = "macos")]
            Self::Macos { kq_fd } => Self::Macos { kq_fd: *kq_fd },
            #[cfg(target_os = "windows")]
            Self::Windows { event } => Self::Windows { event: *event },
        }
    }
}

impl SignalHandle {
    /// Create a no-op handle (for unsupported platforms).
    pub fn none() -> Self {
        Self::None
    }
}

impl EventReadiness for SignalHandle {
    fn is_ready(&self, dur: Option<std::time::Duration>) -> bool {
        match self {
            Self::None => false,
            #[cfg(target_os = "linux")]
            Self::Linux { epoll_fd, .. } => {
                let mut events = [libc::epoll_event { events: 0, u64: 0 }];
                let timeout = dur.map(|d| d.as_millis() as i32).unwrap_or(-1);
                let count = unsafe {
                    libc::epoll_wait(*epoll_fd, events.as_mut_ptr(), 1, timeout)
                };
                count > 0
            }
            #[cfg(target_os = "macos")]
            Self::Macos { kq_fd } => {
                use std::mem::MaybeUninit;
                let mut event = MaybeUninit::<libc::kevent>::uninit();
                let timeout = dur.map(|d| {
                    libc::timespec {
                        tv_sec: d.as_secs() as _,
                        tv_nsec: d.subsec_nanos() as _,
                    }
                });
                let timeout_ptr = timeout.as_ref().map_or(std::ptr::null(), |t| t as *const _);
                let count = unsafe {
                    libc::kevent(*kq_fd, std::ptr::null(), 0, event.as_mut_ptr(), 1, timeout_ptr)
                };
                if count <= 0 {
                    return false;
                }

                // kqueue captures the signal for us (there is no handler to run),
                // so translate the EVFILT_SIGNAL event back into a SignalKind and
                // deliver it to the bus here — the macOS counterpart to the
                // Linux/Windows handlers' `deliver_signal` call.
                let ev = unsafe { event.assume_init() };
                if let Some(kind) = signal_kind_from_kevent(ev) {
                    crate::signal::deliver_signal(kind);
                }
                true
            }
            #[cfg(target_os = "windows")]
            Self::Windows { event } => {
                use windows_sys::Win32::System::Threading::{WaitForSingleObject, INFINITE};
                let timeout = dur.map(|d| d.as_millis() as u32).unwrap_or(INFINITE);
                let result = unsafe { WaitForSingleObject(*event, timeout) };
                result == windows_sys::Win32::Foundation::WAIT_OBJECT_0
            }
        }
    }
}

/// Map a kqueue `EVFILT_SIGNAL` event back to a [`SignalKind`].
#[cfg(target_os = "macos")]
fn signal_kind_from_kevent(ev: libc::kevent) -> Option<SignalKind> {
    if ev.filter != libc::EVFILT_SIGNAL as i16 {
        return None;
    }
    match ev.ident as libc::c_int {
        libc::SIGINT => Some(SignalKind::Interrupt),
        libc::SIGTERM => Some(SignalKind::Terminate),
        libc::SIGHUP => Some(SignalKind::Hangup),
        libc::SIGQUIT => Some(SignalKind::Quit),
        _ => None,
    }
}
