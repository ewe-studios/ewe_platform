/// Windows IOCP selector.
///
/// Uses `CreateIoCompletionPort` to associate handles with a completion port,
/// and `GetQueuedCompletionStatus` to poll for completion events.
///
/// For readiness polling on sockets, we use `WSAPoll` under the hood since
/// IOCP only posts completions for overlapped I/O operations (not readiness).
/// For non-socket handles, readiness must be tracked via overlapped I/O operations.
use super::super::event::Event;
use super::super::Events;
use crate::poll::{Interest, Registry, Token};

use std::collections::HashMap;
use std::io;
use std::net::UdpSocket;
use std::os::windows::io::AsRawSocket;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows_sys::Win32::{
    Foundation::*, Networking::WinSock::*, System::IO::CreateIoCompletionPort,
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
    /// Waker token, reported when [`Selector::wake`] ends a poll.
    waker_token: Mutex<Option<Token>>,
    /// Wake channel: a loopback UDP socket connected to itself (#532).
    ///
    /// `WSAPoll` waits only on sockets, so an event handle cannot end it. Every
    /// poll puts this socket in its set. One byte sent to it ends a blocked
    /// poll: for a wake, and for a change of the registered set.
    notify: UdpSocket,
}

// SAFETY: The IOCP handle and the event handle are kernel handles. Windows allows
// any thread to use them. All other fields sit behind a `Mutex`.
unsafe impl Send for Selector {}
unsafe impl Sync for Selector {}

impl Selector {
    /// Create a new IOCP selector.
    pub fn new() -> io::Result<Self> {
        unsafe {
            let iocp = CreateIoCompletionPort(INVALID_HANDLE_VALUE, std::ptr::null_mut(), 0, 0);
            if iocp.is_null() {
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

            let notify = match Self::notify_socket() {
                Ok(socket) => socket,
                Err(e) => {
                    WSACleanup();
                    CloseHandle(iocp);
                    return Err(e);
                }
            };

            Ok(Self {
                iocp,
                handles: Mutex::new(HashMap::new()),
                waker_token: Mutex::new(None),
                notify,
            })
        }
    }

    /// A non-blocking loopback UDP socket that sends to itself.
    fn notify_socket() -> io::Result<UdpSocket> {
        let socket = UdpSocket::bind("127.0.0.1:0")?;
        socket.connect(socket.local_addr()?)?;
        socket.set_nonblocking(true)?;
        Ok(socket)
    }

    /// End a blocked [`Selector::poll`].
    ///
    /// A full send buffer means a byte is already queued, and that byte ends
    /// the poll too, so `WouldBlock` is success.
    fn notify(&self) -> io::Result<()> {
        match self.notify.send(&[1]) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Read every queued notify byte, so the next poll can block again.
    fn drain_notify(&self) {
        let mut buf = [0_u8; 64];
        loop {
            match self.notify.recv(&mut buf) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                // Windows reports an earlier ICMP error on a UDP socket as
                // ConnectionReset on the next recv; read on.
                Err(e) if e.kind() == io::ErrorKind::ConnectionReset => {}
                Err(e) => {
                    tracing::warn!(error = %e, "draining the Windows selector notify socket failed");
                    break;
                }
            }
        }
    }

    /// Create a new IOCP selector and its `Registry` together.
    pub fn new_with_registry() -> io::Result<(Arc<Self>, Registry)> {
        let selector = Arc::new(Self::new()?);
        let registry = Registry {
            selector: selector.clone(),
        };
        Ok((selector, registry))
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
            if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid handle",
                ));
            }

            // Associate handle with our IOCP. The completion key encodes our token.
            let result = CreateIoCompletionPort(handle, self.iocp, token.0 as usize, 0);
            if result.is_null() {
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
        drop(handles);

        // A poll that blocks now does not watch this socket. End it, so the
        // next poll includes it.
        self.notify()
    }

    /// Re-register a raw handle with new token and/or interest.
    pub fn reregister_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let mut handles = self.handles.lock().unwrap();
        if let Some(state) = handles.get_mut(&fd) {
            state.token = token.0 as u64;
            state.interest = interest;
            drop(handles);
            self.notify()
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "handle not registered",
            ))
        }
    }

    /// Deregister a raw handle from the IOCP selector.
    pub fn deregister_fd(&self, fd: RawFd) -> io::Result<()> {
        let mut handles = self.handles.lock().unwrap();
        let removed = handles.remove(&fd).is_some();
        drop(handles);
        if removed {
            // A blocked poll may still watch the removed socket.
            self.notify()?;
        }
        Ok(())
    }

    /// VNODE registration is not supported on Windows IOCP for file watching.
    pub fn register_vnode(&self, _fd: RawFd, _token: Token) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "EVFILT_VNODE not supported on Windows",
        ))
    }

    /// VNODE deregistration — no-op on Windows.
    pub fn deregister_vnode(&self, _fd: RawFd) -> io::Result<()> {
        Ok(())
    }

    /// Register the waker token. [`Selector::wake`] then ends a blocked poll,
    /// and the poll reports this token as readable.
    pub fn register_waker(&self, token: Token) -> io::Result<()> {
        *self.waker_token.lock().unwrap() = Some(token);
        Ok(())
    }

    /// Wake the poll selector: send one byte to the notify socket.
    pub fn wake(&self, _token: Token) -> io::Result<()> {
        if self.waker_token.lock().unwrap().is_none() {
            return Err(io::Error::new(io::ErrorKind::Other, "waker not registered"));
        }
        self.notify()
    }

    /// Wait for readiness events.
    ///
    /// For sockets, uses `WSAPoll` to check readiness.
    /// For non-socket handles, uses `GetQueuedCompletionStatus` with the given timeout
    /// to check for overlapped I/O completions.
    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        // WSAPoll takes an i32: -1 waits with no limit. Clamp long timeouts.
        let timeout_ms: i32 =
            timeout.map_or(-1, |d| i32::try_from(d.as_millis()).unwrap_or(i32::MAX));

        let (ptr, cap) = events.as_mut_ptr_and_cap();
        let mut count = 0;

        let handles = self.handles.lock().unwrap();

        // Collect sockets for WSAPoll. Slot 0 is the notify socket, so a wake
        // or a change of the set ends the wait, and the set is never empty.
        let mut pollfds: Vec<WSAPOLLFD> = vec![WSAPOLLFD {
            fd: self.notify.as_raw_socket() as SOCKET,
            events: POLLIN,
            revents: 0,
        }];
        let mut socket_tokens: Vec<Token> = vec![Token(usize::MAX)];

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

        // Poll sockets via WSAPoll. The set always holds the notify socket.
        {
            let n = unsafe { WSAPoll(pollfds.as_mut_ptr(), pollfds.len() as u32, timeout_ms) };

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
                if pollfds[0].revents != 0 {
                    self.drain_notify();
                    let waker = *self.waker_token.lock().unwrap();
                    if let (Some(token), true) = (waker, count < cap) {
                        let event = Event::default()
                            .with_key(token.0)
                            .with_flags(super::super::event::windows::READABLE);
                        unsafe {
                            *ptr.add(count) = event;
                        }
                        count += 1;
                    }
                }
                for (i, pollfd) in pollfds.iter().enumerate().skip(1) {
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
                            // POLLHUP is a normal peer close, not an error. The
                            // reader still drains the bytes that came before it.
                            if pollfd.revents & (POLLERR | POLLNVAL) != 0 {
                                flags |= super::super::event::windows::ERROR;
                            }
                            if pollfd.revents & POLLHUP != 0 {
                                flags |= super::super::event::windows::READ_CLOSED;
                            }

                            event = event.with_flags(flags);
                            unsafe {
                                *ptr.add(count) = event;
                            }
                            count += 1;
                        }
                    }
                }
            }
        }

        events.set_len(count);

        Ok(())
    }
}

impl Drop for Selector {
    fn drop(&mut self) {
        unsafe {
            if !self.iocp.is_null() {
                CloseHandle(self.iocp);
            }
            WSACleanup();
        }
    }
}
