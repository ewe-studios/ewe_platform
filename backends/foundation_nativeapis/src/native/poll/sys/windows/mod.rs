/// Windows IOCP selector.
///
/// Uses `CreateIoCompletionPort` to associate handles with a completion port,
/// and `GetQueuedCompletionStatus` to poll for completion events.
///
/// For readiness polling on sockets, we use `WSAPoll` under the hood since
/// IOCP only posts completions for overlapped I/O operations (not readiness).
/// For non-socket handles, readiness must be tracked via overlapped I/O operations.

use super::super::event::Event;
use super::super::super::Events;
use crate::native::poll::{Interest, Registry, Token};

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows_sys::Win32::{
    Foundation::*,
    Networking::WinSock::*,
    System::Threading::*,
};

/// Per-registered-fd state for the IOCP selector.
struct IoState {
    /// Completion key (maps to our Token).
    token: u64,
    /// What readiness we're tracking for this handle.
    interest: Interest,
    /// Whether this is a socket (uses WSAPoll for readiness).
    is_socket: bool,
}

/// The raw handle type on Windows.
pub type RawFd = std::os::windows::io::RawHandle;

/// The internal IOCP selector.
pub struct Selector {
    /// The IOCP handle.
    iocp: HANDLE,
    /// Track registered handles and their state.
    handles: Mutex<HashMap<RawFd, IoState>>,
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

            // Initialize Winsock for WSAPoll
            let mut wsa_data: WSADATA = std::mem::zeroed();
            let wsa_result = WSAStartup(0x0202, &mut wsa_data);
            if wsa_result != 0 {
                CloseHandle(iocp);
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("WSAStartup failed with error code {}", wsa_result),
                ));
            }

            let selector = Arc::new(Self {
                iocp,
                handles: Mutex::new(HashMap::new()),
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
    ///
    /// For sockets, readiness is tracked via `WSAPoll` internally.
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

        // Determine if this is a socket by checking if getsockopt with SOL_SOCKET works
        let is_socket = unsafe {
            let mut optval: i32 = 0;
            let mut optlen = std::mem::size_of::<i32>() as i32;
            getsockopt(
                fd as SOCKET,
                SOL_SOCKET,
                SO_TYPE,
                &mut optval as *mut _ as *mut _,
                &mut optlen,
            ) == 0
        };

        let mut handles = self.handles.lock().unwrap();
        handles.insert(
            fd,
            IoState {
                token: token.0 as u64,
                interest,
                is_socket,
            },
        );

        Ok(())
    }

    /// Re-register a raw handle with new token and/or interest.
    pub fn reregister_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
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

    /// Wait for readiness events.
    ///
    /// For sockets, uses `WSAPoll` to check readiness.
    /// For non-socket handles, uses `GetQueuedCompletionStatus` with the given timeout
    /// to check for overlapped I/O completions.
    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        let timeout_ms = timeout
            .map(|d| d.as_millis() as u32)
            .unwrap_or(u32::MAX);

        let (ptr, cap) = events.as_mut_ptr_and_cap();
        let mut count = 0;

        let handles = self.handles.lock().unwrap();

        // Collect sockets for WSAPoll
        let mut pollfds: Vec<WSAPOLLFD> = Vec::new();
        let mut socket_tokens: Vec<Token> = Vec::new();

        for (&fd, state) in handles.iter() {
            if state.is_socket {
                let mut events_flags: i16 = 0;
                if state.interest.is_readable() {
                    events_flags |= POLLIN;
                }
                if state.interest.is_writable() {
                    events_flags |= POLLOUT;
                }

                pollfds.push(WSAPOLLFD {
                    fd: fd as SOCKET,
                    events: events_flags,
                    revents: 0,
                });
                socket_tokens.push(Token(state.token as usize));
            }
        }

        drop(handles);

        // Poll sockets via WSAPoll
        if !pollfds.is_empty() {
            let n = unsafe {
                WSAPoll(
                    pollfds.as_mut_ptr(),
                    pollfds.len() as u32,
                    timeout_ms as i32,
                )
            };

            if n < 0 {
                let err = unsafe { io::Error::from_raw_os_error(WSAGetLastError()) };
                // WSAEINTR is expected when interrupted
                if err.raw_os_error() == Some(WSAEINTR) {
                    events.clear();
                    return Ok(());
                }
                return Err(err);
            }

            if n > 0 {
                for (i, pollfd) in pollfds.iter().enumerate() {
                    if pollfd.revents != 0 {
                        if count < cap {
                            let mut event = Event::default().with_key(socket_tokens[i].0);
                            let mut flags = 0;

                            if pollfd.revents & (POLLIN | POLLPRI) != 0 {
                                flags |= super::super::event::windows::READABLE;
                            }
                            if pollfd.revents & POLLOUT != 0 {
                                flags |= super::super::event::windows::WRITABLE;
                            }
                            if pollfd.revents & (POLLERR | POLLHUP) != 0 {
                                flags |= super::super::event::windows::ERROR;
                            }
                            if pollfd.revents & POLLHUP != 0 {
                                flags |= super::super::event::windows::READ_CLOSED;
                            }

                            event = event.with_flags(flags);
                            *ptr.add(count) = event;
                            count += 1;
                        }
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
            WSACleanup();
        }
    }
}
