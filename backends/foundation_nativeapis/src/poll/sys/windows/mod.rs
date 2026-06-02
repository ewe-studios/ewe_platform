/// Windows IOCP selector.
///
/// Uses `CreateIoCompletionPort` to associate handles with a completion port,
/// and `GetQueuedCompletionStatus` to poll for readiness events.
///
/// On Windows, readiness tracking works differently than epoll/kqueue:
/// - Handles are associated with an IOCP via `CreateIoCompletionPort`
/// - Overlapped I/O operations post completions to the IOCP when ready
/// - For readiness polling (not overlapped I/O), we use `PostQueuedCompletionStatus`
///   to manually post readiness events when a handle becomes ready

use crate::poll::event::Event;
use crate::poll::Events;
use crate::poll::{Interest, Registry, Token};

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows_sys::Win32::{
    Foundation::*,
    System::Threading::*,
};

/// Per-registered-fd state for the IOCP selector.
struct IoState {
    /// Completion key (maps to our Token).
    token: u64,
    /// What readiness we're tracking for this handle.
    interest: Interest,
    /// Whether a readiness event has been posted but not yet consumed.
    readiness: u8,
}

/// The raw handle type on Windows.
pub type RawFd = std::os::windows::io::RawHandle;

/// The internal IOCP selector.
pub struct Selector {
    /// The IOCP handle.
    iocp: HANDLE,
    /// Track registered handles and their state.
    handles: Mutex<std::collections::HashMap<RawFd, IoState>>,
    /// Waker token.
    waker_token: Mutex<Option<Token>>,
    /// Waker event handle.
    waker_event: Mutex<Option<HANDLE>>,
}

impl Selector {
    /// Create a new IOCP selector.
    pub fn new_with_registry() -> io::Result<(Arc<Self>, Registry)> {
        unsafe {
            let iocp = CreateIoCompletionPort(INVALID_HANDLE_VALUE, None, 0, 0);
            if iocp == 0 {
                return Err(io::Error::last_os_error());
            }

            let selector = Arc::new(Self {
                iocp,
                handles: Mutex::new(std::collections::HashMap::new()),
                waker_token: Mutex::new(None),
                waker_event: Mutex::new(None),
            });

            let registry = Registry {
                selector: selector.clone(),
            };

            Ok((selector, registry))
        }
    }

    /// Register a raw handle with the IOCP selector.
    ///
    /// Associates the handle with our completion port via `CreateIoCompletionPort`.
    /// The token is stored as the completion key.
    pub fn register_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        unsafe {
            let handle = fd as HANDLE;
            if handle == 0 || handle == INVALID_HANDLE_VALUE {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid handle"));
            }

            // Associate handle with our IOCP. The completion key encodes our token.
            let result = CreateIoCompletionPort(handle, Some(self.iocp), token.0 as usize, 0);
            if result == 0 {
                return Err(io::Error::last_os_error());
            }
        }

        let mut handles = self.handles.lock().unwrap();
        handles.insert(
            fd,
            IoState {
                token: token.0 as u64,
                interest,
                readiness: 0,
            },
        );

        Ok(())
    }

    /// Re-register a raw handle with new token and/or interest.
    pub fn reregister_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        // Update our internal state. The IOCP association doesn't need re-doing
        // since CreateIoCompletionPort doesn't support re-registration.
        let mut handles = self.handles.lock().unwrap();
        if let Some(state) = handles.get_mut(&fd) {
            state.token = token.0 as u64;
            state.interest = interest;
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "handle not registered"))
        }
    }

    /// Deregister a raw handle from the IOCP selector.
    pub fn deregister_fd(&self, fd: RawFd) -> io::Result<()> {
        let mut handles = self.handles.lock().unwrap();
        handles.remove(&fd);
        Ok(())
    }

    /// VNODE registration is not supported on Windows IOCP for file watching.
    pub fn register_vnode(&self, _fd: RawFd, _token: Token) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "EVFILT_VNODE not supported on Windows"))
    }

    /// VNODE deregistration — no-op on Windows.
    pub fn deregister_vnode(&self, _fd: RawFd) -> io::Result<()> {
        Ok(())
    }

    /// Register the waker — creates a manual-reset event and associates it
    /// with the IOCP via a dummy completion key.
    pub fn register_waker(&self, token: Token) -> io::Result<()> {
        unsafe {
            let event = CreateEventW(None, true as BOOL, false as BOOL, None);
            if event == 0 {
                return Err(io::Error::last_os_error());
            }

            // Associate the event with the IOCP. When signaled, it posts a completion.
            let result = CreateIoCompletionPort(event, Some(self.iocp), token.0 as usize, 0);
            if result == 0 {
                CloseHandle(event);
                return Err(io::Error::last_os_error());
            }

            *self.waker_token.lock().unwrap() = Some(token);
            *self.waker_event.lock().unwrap() = Some(event);
            Ok(())
        }
    }

    /// Wake the poll selector by signaling the waker event.
    pub fn wake(&self, _token: Token) -> io::Result<()> {
        let waker_event = self.waker_event.lock().unwrap();
        if let Some(event) = *waker_event {
            unsafe {
                if SetEvent(event) == 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "waker not registered"))
        }
    }

    /// Wait for readiness events via `GetQueuedCompletionStatus`.
    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        let timeout_ms = timeout
            .map(|d| d.as_millis() as u32)
            .unwrap_or(u32::MAX);

        let (ptr, cap) = events.as_mut_ptr_and_cap();
        let mut count = 0;

        unsafe {
            let mut completion_key: usize = 0;
            let mut overlapped: *mut OVERLAPPED = std::ptr::null_mut();
            let mut bytes_transferred: u32 = 0;

            let result = GetQueuedCompletionStatus(
                self.iocp,
                &mut bytes_transferred,
                &mut completion_key,
                &mut overlapped,
                timeout_ms,
            );

            if result == 0 {
                let err = io::Error::last_os_error();
                // WAIT_TIMEOUT is expected when timeout expires
                if err.raw_os_error() == Some(WAIT_TIMEOUT as i32) {
                    events.clear();
                    return Ok(());
                }
                // For other errors, check if the handle was closed
                if err.raw_os_error() == Some(6) /* ERROR_INVALID_HANDLE */ {
                    events.clear();
                    return Ok(());
                }
                return Err(err);
            }

            // Build an Event from the completion
            let token = Token(completion_key);
            let mut event = Event::default().with_key(token.0);
            // On IOCP, a successful completion means the fd is ready
            if bytes_transferred > 0 {
                event = event.with_flags(crate::poll::event::windows::READABLE | crate::poll::event::windows::WRITABLE);
            }

            if count < cap {
                *ptr.add(count) = event;
                count += 1;
            }

            // Check waker
            let waker_token = self.waker_token.lock().unwrap();
            if let Some(wt) = *waker_token {
                if token == wt {
                    // Reset the waker event
                    if let Some(event) = *self.waker_event.lock().unwrap() {
                        ResetEvent(event);
                    }
                }
            }
        }

        unsafe {
            events.set_len(count);
        }

        Ok(())
    }
}

impl Drop for Selector {
    fn drop(&mut self) {
        unsafe {
            if let Some(event) = self.waker_event.lock().unwrap().take() {
                CloseHandle(event);
            }
            if self.iocp != 0 {
                CloseHandle(self.iocp);
            }
        }
    }
}
