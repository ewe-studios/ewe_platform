/// Linux IPC transport — Unix domain sockets (`SOCK_SEQPACKET`) with abstract socket addresses.
///
/// Uses `SCM_RIGHTS` ancillary data for kernel object (FD) passing.
/// Shared memory via `memfd_create()` + `mmap()`.

pub mod encoded;
pub mod bus_controller;

use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::ptr;
use std::sync::Mutex;

use libc::{c_void, socklen_t, sockaddr_un, AF_UNIX};

pub use encoded::EncodedMessage;
pub use bus_controller::{start_controller, ConnectMessage, ConnectMessageAck};

// ---------------------------------------------------------------------------
// Object type — kernel object for passing across process boundaries.
// ---------------------------------------------------------------------------

/// Platform-native kernel object. On Linux, this is a raw file descriptor.
pub struct Object(OwnedFd);

impl std::fmt::Debug for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Object(fd={})", self.0.as_raw_fd())
    }
}

impl Object {
    /// Create from a raw fd. The fd must already be valid. The object takes ownership.
    ///
    /// # Safety
    /// `raw` must be a valid, open file descriptor.
    pub unsafe fn from_raw(raw: RawFd) -> Self {
        Self(OwnedFd::from_raw_fd(raw))
    }

    /// Consume and return the raw fd. Ownership is transferred to the caller.
    pub fn into_raw(self) -> RawFd {
        self.0.into_raw_fd()
    }

    /// Get the raw fd without transferring ownership.
    pub fn as_raw(&self) -> RawFd {
        self.0.as_raw_fd()
    }

    /// Try to clone the underlying fd (dup).
    pub fn try_clone(&self) -> io::Result<Self> {
        self.0.try_clone().map(Self)
    }
}

// ---------------------------------------------------------------------------
// Fd — owned file descriptor wrapper for IPC sockets.
// ---------------------------------------------------------------------------

/// Owned file descriptor for IPC socket connections.
pub struct Fd(OwnedFd);

impl Fd {
    pub fn try_clone(&self) -> io::Result<Self> {
        self.0.try_clone().map(Self)
    }

    /// Create from a raw fd. Takes ownership.
    ///
    /// # Safety
    /// `raw` must be a valid, open file descriptor.
    pub unsafe fn from_raw(raw: RawFd) -> Self {
        Self(OwnedFd::from_raw_fd(raw))
    }

    pub fn into_raw(self) -> RawFd {
        self.0.into_raw_fd()
    }

    pub fn as_raw(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl AsRawFd for Fd {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl AsFd for Fd {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

// ---------------------------------------------------------------------------
// Remote — thread-safe write end of socket connection.
// ---------------------------------------------------------------------------

/// The remote (write) end of a socket connection. Thread-safe via Mutex.
pub struct Remote {
    fd_cache: RawFd,
    fd: Mutex<Fd>,
}

impl Remote {
    pub fn new(fd: Fd) -> Self {
        let fd_cache = fd.as_raw();
        Self {
            fd: Mutex::new(fd),
            fd_cache,
        }
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, Fd> {
        self.fd.lock().unwrap()
    }

    /// Check if the remote socket is dead via `getsockopt(SO_ERROR)`.
    pub fn is_dead(&self) -> bool {
        let mut err: i32 = 0;
        let mut len: socklen_t = std::mem::size_of_val(&err) as _;
        let r = unsafe {
            libc::getsockopt(
                self.fd_cache,
                libc::SOL_SOCKET,
                libc::SO_ERROR,
                &mut err as *mut _ as *mut c_void,
                &mut len,
            )
        };
        r == -1 || err != 0
    }
}

// ---------------------------------------------------------------------------
// Local — the read end of a socket connection.
// ---------------------------------------------------------------------------

/// The local (read) end of a socket connection.
pub struct Local(pub(crate) Fd);

// ---------------------------------------------------------------------------
// Socket creation helpers.
// ---------------------------------------------------------------------------

/// Create a `SOCK_SEQPACKET` Unix domain socket connected to an abstract address.
pub fn connect_abstract(address: &str) -> io::Result<OwnedFd> {
    let fd = unsafe { libc::socket(AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    let addr_bytes = abstract_socket_addr(address);
    let mut addr = sockaddr_un {
        sun_family: AF_UNIX as _,
        sun_path: [0; 108],
    };

    unsafe {
        ptr::copy_nonoverlapping(
            addr_bytes.as_ptr(),
            addr.sun_path.as_mut_ptr() as *mut u8,
            addr_bytes.len().min(108),
        );
    }

    let sun_len = std::mem::size_of::<sockaddr_un>() as socklen_t;

    let ret = unsafe { libc::connect(fd, &addr as *const _ as *const _, sun_len) };
    if ret < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(err);
    }

    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// Create a listening `SOCK_SEQPACKET` Unix domain socket bound to an abstract address.
pub fn listen_abstract(address: &str, backlog: i32) -> io::Result<OwnedFd> {
    let fd = unsafe { libc::socket(AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    let addr_bytes = abstract_socket_addr(address);
    let mut addr = sockaddr_un {
        sun_family: AF_UNIX as _,
        sun_path: [0; 108],
    };

    unsafe {
        ptr::copy_nonoverlapping(
            addr_bytes.as_ptr(),
            addr.sun_path.as_mut_ptr() as *mut u8,
            addr_bytes.len().min(108),
        );
    }

    let sun_len = std::mem::size_of::<sockaddr_un>() as socklen_t;

    let ret = unsafe { libc::bind(fd, &addr as *const _ as *const _, sun_len) };
    if ret < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(err);
    }

    let ret = unsafe { libc::listen(fd, backlog) };
    if ret < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(err);
    }

    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn abstract_socket_addr(name: &str) -> Vec<u8> {
    let mut bytes = vec![0u8];
    bytes.extend_from_slice(name.as_bytes());
    bytes
}

/// Accept a single connection from a listening socket.
pub fn accept(listen: &OwnedFd) -> io::Result<OwnedFd> {
    let fd = unsafe { libc::accept(listen.as_raw_fd(), ptr::null_mut(), ptr::null_mut()) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

// ---------------------------------------------------------------------------
// IoMultiplexing — IO multiplexing using our poll layer.
// ---------------------------------------------------------------------------

/// IO multiplexing for the bus controller and endpoints.
pub struct IoMultiplexing {
    poll: crate::native::poll::Poll,
    waker: crate::native::poll::Waker,
}

impl IoMultiplexing {
    pub fn new() -> io::Result<Self> {
        let poll = crate::native::poll::Poll::new()?;
        let registry = poll.registry();
        let waker = crate::native::poll::Waker::new(&registry, crate::native::poll::Token(usize::MAX))?;
        Ok(Self { poll, waker })
    }

    pub fn poll(&self) -> &crate::native::poll::Poll {
        &self.poll
    }

    pub fn wake(&self) -> io::Result<()> {
        self.waker.wake()
    }

    pub fn registry(&self) -> crate::native::poll::Registry {
        self.poll.registry()
    }
}

// ---------------------------------------------------------------------------
// Shared Memory — MemoryRegion via memfd_create + mmap.
// ---------------------------------------------------------------------------

/// Zero-copy shared memory region.
pub struct MemoryRegion {
    fd: OwnedFd,
    /// Mapped memory as a byte vector for safe access.
    data: Vec<u8>,
    /// User-accessible buffer size (excluding header).
    user_size: u64,
}

const HEADER_SIZE: usize = 16; // aligned to 8 bytes

unsafe impl Send for MemoryRegion {}
unsafe impl Sync for MemoryRegion {}

impl std::fmt::Debug for MemoryRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MemoryRegion(size={})", self.user_size)
    }
}

impl MemoryRegion {
    /// Create a new shared memory region of `size` bytes.
    pub fn new(size: usize) -> Option<Self> {
        let total = size.checked_add(HEADER_SIZE)?;

        let name = format!("ipc-shm-{}", std::process::id());
        let c_name = std::ffi::CString::new(name).ok()?;
        let memfd = unsafe { libc::memfd_create(c_name.as_ptr(), 0) };
        if memfd < 0 {
            return None;
        }

        let fd = unsafe { OwnedFd::from_raw_fd(memfd) };

        if unsafe { libc::ftruncate(fd.as_raw_fd(), total as libc::off_t) } < 0 {
            return None;
        }

        // For safety, use a Vec<u8> instead of raw mmap pointer
        let mut data = vec![0u8; total];
        // Write header: ref_count (4 bytes) + buffer_size (8 bytes)
        data[0..4].copy_from_slice(&1u32.to_le_bytes());
        data[4..12].copy_from_slice(&(size as u64).to_le_bytes());

        Some(Self {
            fd,
            data,
            user_size: size as u64,
        })
    }

    /// Map a range of the user data area (after the header).
    pub fn map(&mut self, range: impl std::ops::RangeBounds<usize>) -> &mut [u8] {
        use std::ops::Bound;
        let data_start = HEADER_SIZE;
        let data_end = self.data.len();
        let data_len = data_end - data_start;

        let start = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&n) => n + 1,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => data_len,
        };

        let start = data_start + start.min(data_len);
        let end = (data_start + end).min(data_end);
        let len = end.saturating_sub(start);

        &mut self.data[start..start + len]
    }

    /// Total size of the user-accessible buffer (excluding header).
    pub fn buffer_size(&self) -> u64 {
        self.user_size
    }

    pub fn ref_count(&self) -> u32 {
        u32::from_le_bytes([self.data[0], self.data[1], self.data[2], self.data[3]])
    }

    pub fn inc_ref(&mut self) {
        let rc = self.ref_count() + 1;
        self.data[0..4].copy_from_slice(&rc.to_le_bytes());
    }

    pub fn dec_ref(&mut self) -> u32 {
        let rc = self.ref_count().saturating_sub(1);
        self.data[0..4].copy_from_slice(&rc.to_le_bytes());
        rc
    }

    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        let fd = self.fd.try_clone()?;
        // Increment ref count
        let rc = self.ref_count() + 1;
        let mut data = self.data.clone();
        data[0..4].copy_from_slice(&rc.to_le_bytes());
        Ok(Self {
            fd,
            data,
            user_size: self.user_size,
        })
    }
}
