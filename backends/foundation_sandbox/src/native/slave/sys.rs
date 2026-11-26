//! Thin wrappers over the Linux system calls. Every function returns
//! [`std::io::Result`] and every `unsafe` block is here or in the `fork` child
//! block of [`crate::native::slave::exec`].

use std::ffi::CString;
use std::io;
use std::mem::size_of;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// The type of an `ioctl` request number. The C libraries disagree: the GNU C
/// library takes an unsigned long, and musl takes an int.
#[cfg(target_env = "musl")]
type IoctlRequest = libc::c_int;
#[cfg(not(target_env = "musl"))]
type IoctlRequest = libc::c_ulong;
/// The Linux request that reads the flags of a network interface.
const SIOCGIFFLAGS: IoctlRequest = 0x8913;
/// The Linux request that writes the flags of a network interface.
const SIOCSIFFLAGS: IoctlRequest = 0x8914;
/// The largest interface name that the kernel accepts.
const IF_NAMESIZE: usize = 16;
/// The kernel value of the `IFF_UP` interface flag.
const IFF_UP: i16 = 0x1;

/// The kernel layout of `struct open_how`, in bytes.
///
/// The libc type is `#[non_exhaustive]`, so this module builds the kernel
/// layout itself: three 64-bit fields.
#[repr(C)]
#[derive(Clone, Copy)]
struct OpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}

/// The `struct ifreq` buffer of an interface flag request. Only the flags
/// member is read or written, so the union is a byte array of the kernel size.
#[repr(C)]
struct IfReq {
    name: [u8; IF_NAMESIZE],
    flags: i16,
    _rest: [u8; 22],
}

/// The last operating system error of this thread.
#[must_use]
pub fn last_errno() -> i32 {
    io::Error::last_os_error()
        .raw_os_error()
        .unwrap_or(libc::EIO)
}

/// An `io::Error` of one error number.
#[must_use]
pub fn error_of(errno: i32) -> io::Error {
    io::Error::from_raw_os_error(errno)
}

/// A pipe whose two ends are close-on-exec. Both ends start blocking.
///
/// # Errors
/// The operating system error of `pipe2`.
pub fn pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0_i32; 2];
    // SAFETY: `fds` is a valid array of two descriptors.
    let result = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the call filled both entries with descriptors that this process
    // owns and no other value holds.
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

/// Makes `fd` non-blocking.
///
/// # Errors
/// The operating system error of `fcntl`.
pub fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    set_nonblocking_mode(fd, true)
}

/// Sets the capacity of the pipe that `fd` belongs to.
///
/// The kernel rounds `bytes` up to a page multiple. A smaller size always
/// works. A larger one needs a size at or below `/proc/sys/fs/pipe-max-size`.
///
/// # Errors
/// The operating system error of `fcntl`.
pub fn set_pipe_size(fd: RawFd, bytes: u32) -> io::Result<()> {
    // SAFETY: `fd` is an open pipe descriptor of this process, and
    // `F_SETPIPE_SZ` reads only the integer argument.
    let result = unsafe { libc::fcntl(fd, libc::F_SETPIPE_SZ, bytes) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Sets (`true`) or clears (`false`) the non-blocking flag of `fd`.
///
/// # Errors
/// The operating system error of `fcntl`.
pub fn set_nonblocking_mode(fd: RawFd, nonblocking: bool) -> io::Result<()> {
    // SAFETY: `fd` is an open descriptor of this process.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    let flags = if nonblocking {
        flags | libc::O_NONBLOCK
    } else {
        flags & !libc::O_NONBLOCK
    };
    // SAFETY: the same descriptor, with a valid flag set.
    let result = unsafe { libc::fcntl(fd, libc::F_SETFL, flags) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Creates a listening `AF_VSOCK` socket on `VMADDR_CID_ANY` and `port`.
///
/// `backlog` bounds the connections that the kernel queues before an accept.
///
/// # Errors
/// The operating system error of `socket`, `bind` or `listen`.
pub fn vsock_listen(port: u32, backlog: i32) -> io::Result<OwnedFd> {
    // SAFETY: `socket` takes three integers.
    let fd = unsafe { libc::socket(libc::AF_VSOCK, libc::SOCK_STREAM | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the call returned a new descriptor that this process owns.
    let socket = unsafe { OwnedFd::from_raw_fd(fd) };
    let address = libc::sockaddr_vm {
        svm_family: u16::try_from(libc::AF_VSOCK).map_err(|_| error_of(libc::EINVAL))?,
        svm_reserved1: 0,
        svm_port: port,
        svm_cid: libc::VMADDR_CID_ANY,
        svm_zero: [0; 4],
    };
    let length = libc::socklen_t::try_from(std::mem::size_of::<libc::sockaddr_vm>())
        .map_err(|_| error_of(libc::EINVAL))?;
    // SAFETY: `address` has the kernel layout of `sockaddr_vm`.
    let result = unsafe {
        libc::bind(
            socket.as_raw_fd(),
            std::ptr::addr_of!(address).cast::<libc::sockaddr>(),
            length,
        )
    };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the descriptor is a socket that is ready to listen.
    if unsafe { libc::listen(socket.as_raw_fd(), backlog) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(socket)
}

/// Accepts one connection from a listening socket, close-on-exec.
///
/// # Errors
/// The operating system error of `accept4`.
pub fn accept_stream(fd: RawFd) -> io::Result<OwnedFd> {
    // SAFETY: the descriptor stays open for the life of the caller.
    let accepted = unsafe {
        libc::accept4(
            fd,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            libc::SOCK_CLOEXEC,
        )
    };
    if accepted < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the call returned a new descriptor that this process owns.
    Ok(unsafe { OwnedFd::from_raw_fd(accepted) })
}

/// Reads at most one buffer from `fd`.
///
/// # Errors
/// The operating system error of `read`, including `WouldBlock`.
pub fn read(fd: RawFd, buffer: &mut [u8]) -> io::Result<usize> {
    loop {
        // SAFETY: `buffer` is valid for its length and `fd` is open.
        let result =
            unsafe { libc::read(fd, buffer.as_mut_ptr().cast::<libc::c_void>(), buffer.len()) };
        if result >= 0 {
            return Ok(result.unsigned_abs());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

/// Writes at most one buffer to `fd`.
///
/// # Errors
/// The operating system error of `write`, including `WouldBlock`.
pub fn write(fd: RawFd, buffer: &[u8]) -> io::Result<usize> {
    loop {
        // SAFETY: `buffer` is valid for its length and `fd` is open.
        let result =
            unsafe { libc::write(fd, buffer.as_ptr().cast::<libc::c_void>(), buffer.len()) };
        if result >= 0 {
            return Ok(result.unsigned_abs());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

/// Reads at `offset` from `fd`.
///
/// # Errors
/// The operating system error of `pread`.
pub fn pread(fd: RawFd, buffer: &mut [u8], offset: u64) -> io::Result<usize> {
    // SAFETY: `buffer` is valid for its length and `fd` is open.
    let result = unsafe {
        libc::pread(
            fd,
            buffer.as_mut_ptr().cast::<libc::c_void>(),
            buffer.len(),
            libc::off_t::try_from(offset).map_err(|_| error_of(libc::EINVAL))?,
        )
    };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(result.unsigned_abs())
}

/// Writes `buffer` at `offset` to `fd` and returns the bytes written.
///
/// # Errors
/// The operating system error of `pwrite`.
pub fn pwrite(fd: RawFd, buffer: &[u8], offset: u64) -> io::Result<usize> {
    // SAFETY: `buffer` is valid for its length and `fd` is open.
    let result = unsafe {
        libc::pwrite(
            fd,
            buffer.as_ptr().cast::<libc::c_void>(),
            buffer.len(),
            libc::off_t::try_from(offset).map_err(|_| error_of(libc::EINVAL))?,
        )
    };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(result.unsigned_abs())
}

/// Writes every byte of `buffer` at `offset`, and returns the bytes written.
///
/// # Errors
/// The operating system error of `pwrite`.
pub fn write_all_at(fd: RawFd, buffer: &[u8], offset: u64) -> io::Result<usize> {
    let mut written = 0;
    while written < buffer.len() {
        match pwrite(fd, &buffer[written..], offset + written as u64) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "the file accepted no byte",
                ));
            }
            Ok(bytes) => written += bytes,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(written)
}

/// Opens `"/"` as a directory descriptor.
///
/// # Errors
/// The operating system error of `open`.
pub fn open_root() -> io::Result<OwnedFd> {
    let path = CString::new("/").map_err(|_| error_of(libc::EINVAL))?;
    open_path(&path, libc::O_PATH | libc::O_DIRECTORY, 0)
}

/// Opens the null device for reading.
///
/// # Errors
/// The operating system error of `open`.
pub fn open_null() -> io::Result<OwnedFd> {
    let path = CString::new("/dev/null").map_err(|_| error_of(libc::EINVAL))?;
    open_path(&path, libc::O_RDONLY, 0)
}

/// Opens `path` with `flags` and `mode`.
///
/// # Errors
/// The operating system error of `open`.
pub fn open_path(path: &CString, flags: i32, mode: u32) -> io::Result<OwnedFd> {
    loop {
        // SAFETY: `path` is a valid zero-terminated string and `mode` is unused
        // unless `O_CREAT` is set.
        let fd = unsafe { libc::open(path.as_ptr(), flags | libc::O_CLOEXEC, mode) };
        if fd >= 0 {
            // SAFETY: the call returned a new descriptor that this process owns.
            return Ok(unsafe { OwnedFd::from_raw_fd(fd) });
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

/// Opens `path` below `root`, where `path` is absolute and every symbolic link
/// resolves as if `root` were `/` (`RESOLVE_IN_ROOT`).
///
/// # Errors
/// The operating system error of `openat2`.
pub fn open_in_root(root: RawFd, path: &CString, flags: i32, mode: u32) -> io::Result<OwnedFd> {
    let how = OpenHow {
        flags: u64::try_from(i64::from(flags | libc::O_CLOEXEC))
            .map_err(|_| error_of(libc::EINVAL))?,
        mode: u64::from(mode),
        resolve: libc::RESOLVE_IN_ROOT,
    };
    // SAFETY: the three arguments have the types of `openat2`, and `how` is the
    // kernel layout of `struct open_how`.
    let fd = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            root,
            path.as_ptr(),
            std::ptr::addr_of!(how),
            size_of::<OpenHow>(),
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the call returned a new descriptor that this process owns.
    Ok(unsafe { OwnedFd::from_raw_fd(fd as RawFd) })
}

/// Opens the directory `name` below `dir`.
///
/// # Errors
/// The operating system error of `openat`.
pub fn openat_dir(dir: RawFd, name: &CString) -> io::Result<OwnedFd> {
    // SAFETY: `name` is a valid zero-terminated string and `dir` is open.
    let fd = unsafe {
        libc::openat(
            dir,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the call returned a new descriptor that this process owns.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// Opens the file `name` below `dir`.
///
/// # Errors
/// The operating system error of `openat`.
pub fn openat(dir: RawFd, name: &CString, flags: i32, mode: u32) -> io::Result<OwnedFd> {
    loop {
        // SAFETY: `name` is a valid zero-terminated string and `dir` is open.
        let fd = unsafe { libc::openat(dir, name.as_ptr(), flags | libc::O_CLOEXEC, mode) };
        if fd >= 0 {
            // SAFETY: the call returned a new descriptor that this process owns.
            return Ok(unsafe { OwnedFd::from_raw_fd(fd) });
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

/// Renames `from` to `to`, both below `dir`.
///
/// # Errors
/// The operating system error of `renameat`.
pub fn renameat(dir: RawFd, from: &CString, to: &CString) -> io::Result<()> {
    // SAFETY: both names are valid zero-terminated strings and `dir` is open.
    let result = unsafe { libc::renameat(dir, from.as_ptr(), dir, to.as_ptr()) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Removes `name` below `dir`.
///
/// # Errors
/// The operating system error of `unlinkat`.
pub fn unlinkat(dir: RawFd, name: &CString) -> io::Result<()> {
    // SAFETY: `name` is a valid zero-terminated string and `dir` is open.
    let result = unsafe { libc::unlinkat(dir, name.as_ptr(), 0) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Flushes the file `fd` to the storage.
///
/// # Errors
/// The operating system error of `fsync`.
pub fn fsync(fd: RawFd) -> io::Result<()> {
    // SAFETY: `fd` is an open descriptor of this process.
    let result = unsafe { libc::fsync(fd) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Reads the state of `fd`.
///
/// # Errors
/// The operating system error of `fstat`.
pub fn fstat(fd: RawFd) -> io::Result<libc::stat> {
    // SAFETY: a zeroed `stat` is a valid output buffer for `fstat`.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: `stat` is a valid output buffer and `fd` is open.
    let result = unsafe { libc::fstat(fd, std::ptr::addr_of_mut!(stat)) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(stat)
}

/// Whether the state of `fd` is a directory.
///
/// # Errors
/// The error of [`fstat`].
pub fn is_directory(fd: RawFd) -> io::Result<bool> {
    let stat = fstat(fd)?;
    Ok(stat.st_mode & libc::S_IFMT == libc::S_IFDIR)
}

/// Starts one child process.
///
/// # Errors
/// The operating system error of `fork`.
pub fn fork() -> io::Result<libc::pid_t> {
    // SAFETY: `fork` has no argument. The caller follows the async-signal-safe
    // rules of the child side.
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(pid)
}

/// This process identifier.
#[must_use]
pub fn getpid() -> i32 {
    // SAFETY: `getpid` has no argument and cannot fail.
    unsafe { libc::getpid() }
}

/// The process identifier of the parent of this process.
#[must_use]
pub fn getppid() -> i32 {
    // SAFETY: `getppid` has no argument and cannot fail.
    unsafe { libc::getppid() }
}

/// Blocks `SIGCHLD` on the calling thread. A new thread copies the mask, so the
/// slave calls this before it starts any thread.
///
/// # Errors
/// The operating system error of `pthread_sigmask`.
pub fn block_sigchld() -> io::Result<()> {
    // SAFETY: a zeroed signal set is a valid output buffer for `sigemptyset`.
    let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
    // SAFETY: `set` is a valid signal set.
    unsafe {
        libc::sigemptyset(std::ptr::addr_of_mut!(set));
        libc::sigaddset(std::ptr::addr_of_mut!(set), libc::SIGCHLD);
    }
    // SAFETY: `set` is a valid signal set.
    let result = unsafe {
        libc::pthread_sigmask(
            libc::SIG_BLOCK,
            std::ptr::addr_of!(set),
            std::ptr::null_mut(),
        )
    };
    if result != 0 {
        return Err(error_of(result));
    }
    Ok(())
}

/// Makes this process the reaper of its orphaned grandchildren.
///
/// # Errors
/// The operating system error of `prctl`.
pub fn become_subreaper() -> io::Result<()> {
    // SAFETY: `PR_SET_CHILD_SUBREAPER` takes a single integer argument.
    let result = unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Opens a descriptor that becomes readable when `SIGCHLD` arrives.
///
/// # Errors
/// The operating system error of `signalfd`.
pub fn signalfd() -> io::Result<OwnedFd> {
    // SAFETY: a zeroed signal set is a valid output buffer for `sigemptyset`.
    let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
    // SAFETY: `set` is a valid signal set.
    unsafe {
        libc::sigemptyset(std::ptr::addr_of_mut!(set));
        libc::sigaddset(std::ptr::addr_of_mut!(set), libc::SIGCHLD);
    }
    // SAFETY: `set` is a valid signal set of one signal.
    let fd = unsafe {
        libc::signalfd(
            -1,
            std::ptr::addr_of!(set),
            libc::SFD_NONBLOCK | libc::SFD_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the call returned a new descriptor that this process owns.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// One result of a non-blocking `waitpid`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitResult {
    /// A child changed state.
    Changed {
        /// The child process identifier.
        pid: i32,
        /// The raw wait status.
        status: i32,
    },
    /// No child changed state.
    None,
    /// This process has no child.
    NoChildren,
}

/// Collects one child that changed state, without waiting.
///
/// # Errors
/// The operating system error of `waitpid`.
pub fn waitpid_nohang() -> io::Result<WaitResult> {
    let mut status = 0_i32;
    // SAFETY: `status` is a valid output buffer.
    let pid = unsafe { libc::waitpid(-1, std::ptr::addr_of_mut!(status), libc::WNOHANG) };
    if pid > 0 {
        return Ok(WaitResult::Changed { pid, status });
    }
    if pid == 0 {
        return Ok(WaitResult::None);
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ECHILD) {
        return Ok(WaitResult::NoChildren);
    }
    Err(error)
}

/// Splits a raw wait status into an exit code or a signal.
#[must_use]
pub fn decode_status(status: i32) -> (Option<i32>, Option<i32>) {
    if libc::WIFEXITED(status) {
        (Some(libc::WEXITSTATUS(status)), None)
    } else if libc::WIFSIGNALED(status) {
        (None, Some(libc::WTERMSIG(status)))
    } else {
        (None, None)
    }
}

/// Sends `signal` to one process.
///
/// # Errors
/// The operating system error of `kill`.
pub fn kill(pid: i32, signal: i32) -> io::Result<()> {
    // SAFETY: `kill` takes two integers.
    let result = unsafe { libc::kill(pid, signal) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Sends `signal` to the process group `group`.
///
/// # Errors
/// The operating system error of `kill`.
pub fn kill_group(group: i32, signal: i32) -> io::Result<()> {
    // SAFETY: `kill` takes two integers; a negative identifier names a group.
    let result = unsafe { libc::kill(-group, signal) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Mounts `source` on `target`.
///
/// # Errors
/// The operating system error of `mount`.
pub fn mount(source: &str, target: &Path, fstype: &str, flags: u64, data: &str) -> io::Result<()> {
    let source = CString::new(source).map_err(|_| error_of(libc::EINVAL))?;
    let target = CString::new(target.as_os_str().as_bytes()).map_err(|_| error_of(libc::EINVAL))?;
    let fstype = CString::new(fstype).map_err(|_| error_of(libc::EINVAL))?;
    let data = CString::new(data).map_err(|_| error_of(libc::EINVAL))?;
    // SAFETY: all four strings are valid and zero-terminated.
    let result = unsafe {
        libc::mount(
            source.as_ptr(),
            target.as_ptr(),
            fstype.as_ptr(),
            flags as libc::c_ulong,
            data.as_ptr().cast::<libc::c_void>(),
        )
    };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Unmounts `target`.
///
/// # Errors
/// The operating system error of `umount2`, such as `EBUSY` when a mount below
/// `target` is still mounted.
pub fn unmount(target: &Path, flags: i32) -> io::Result<()> {
    let target = CString::new(target.as_os_str().as_bytes()).map_err(|_| error_of(libc::EINVAL))?;
    // SAFETY: `target` is a valid zero-terminated string.
    let result = unsafe { libc::umount2(target.as_ptr(), flags) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Flushes every file system buffer.
pub fn sync() {
    // SAFETY: `sync` has no argument.
    unsafe { libc::sync() };
}

/// Powers the machine off.
///
/// # Errors
/// The operating system error of `reboot`. A successful call does not return.
pub fn power_off() -> io::Result<std::convert::Infallible> {
    // SAFETY: `reboot` takes one integer.
    unsafe { libc::reboot(libc::RB_AUTOBOOT) };
    Err(io::Error::last_os_error())
}

/// Waits for ever.
pub fn pause_forever() -> ! {
    loop {
        // SAFETY: `pause` has no argument; it returns on a signal.
        unsafe { libc::pause() };
    }
}

/// The machine architecture and the kernel release from `uname`.
///
/// # Errors
/// The operating system error of `uname`.
pub fn uname() -> io::Result<(String, String)> {
    // SAFETY: a zeroed `utsname` is a valid output buffer for `uname`.
    let mut name: libc::utsname = unsafe { std::mem::zeroed() };
    // SAFETY: `name` is a valid output buffer.
    let result = unsafe { libc::uname(std::ptr::addr_of_mut!(name)) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: each field is a zero-terminated string after a successful call.
    let machine = unsafe { std::ffi::CStr::from_ptr(name.machine.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: the same for the release field.
    let release = unsafe { std::ffi::CStr::from_ptr(name.release.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    Ok((machine, release))
}

/// Sets the `IFF_UP` flag of the loopback interface.
///
/// # Errors
/// The operating system error of `socket` or `ioctl`.
pub fn bring_up_loopback() -> io::Result<()> {
    // SAFETY: `socket` takes three integers. The stream is close-on-exec.
    let socket = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
    if socket < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the call returned a new descriptor that this process owns.
    let socket = unsafe { OwnedFd::from_raw_fd(socket) };
    let mut request = IfReq {
        name: [0; IF_NAMESIZE],
        flags: 0,
        _rest: [0; 22],
    };
    request.name[..3].copy_from_slice(b"lo\0");
    // SAFETY: `request` has the kernel layout of `struct ifreq`.
    let read = unsafe {
        libc::ioctl(
            socket.as_raw_fd(),
            SIOCGIFFLAGS,
            std::ptr::addr_of_mut!(request),
        )
    };
    if read < 0 {
        return Err(io::Error::last_os_error());
    }
    request.flags |= IFF_UP;
    // SAFETY: the same valid `ifreq` buffer.
    let written = unsafe {
        libc::ioctl(
            socket.as_raw_fd(),
            SIOCSIFFLAGS,
            std::ptr::addr_of!(request),
        )
    };
    if written < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Gives `path` the mode `mode`.
///
/// # Errors
/// The operating system error of `chmod`.
pub fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| error_of(libc::EINVAL))?;
    // SAFETY: `path` is a valid zero-terminated string.
    let result = unsafe { libc::chmod(path.as_ptr(), mode as libc::mode_t) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

// ── The child side of `fork` ────────────────────────────────────────────────
//
// Each function below runs between `fork` and `execve` in the child. Each one
// uses only an async-signal-safe call, as `signal-safety(7)` lists them. The
// error number is read from the thread-local `errno` directly: building an
// `io::Error` is not on that list.

/// The error number of this thread, for the child side of `fork`.
fn child_errno() -> i32 {
    // SAFETY: `__errno_location` returns this thread's `errno` slot, which is
    // always valid.
    unsafe { *libc::__errno_location() }
}

/// Starts a new session and process group. It returns the error number, or 0.
#[must_use]
pub fn child_setsid() -> i32 {
    // SAFETY: `setsid` has no argument.
    if unsafe { libc::setsid() } < 0 {
        return child_errno();
    }
    0
}

/// Replaces descriptor `new` with `old`. It returns the error number, or 0.
#[must_use]
pub fn child_dup2(old: RawFd, new: RawFd) -> i32 {
    // SAFETY: both descriptors are open in the child.
    if unsafe { libc::dup2(old, new) } < 0 {
        return child_errno();
    }
    0
}

/// Changes the root of the child. It returns the error number, or 0.
#[must_use]
pub fn child_chroot(path: &CString) -> i32 {
    // SAFETY: `path` is a valid zero-terminated string.
    if unsafe { libc::chroot(path.as_ptr()) } < 0 {
        return child_errno();
    }
    0
}

/// Changes the working directory of the child. It returns the error number, or
/// 0.
#[must_use]
pub fn child_chdir(path: &CString) -> i32 {
    // SAFETY: `path` is a valid zero-terminated string.
    if unsafe { libc::chdir(path.as_ptr()) } < 0 {
        return child_errno();
    }
    0
}

/// Replaces the image of the child. It returns the error number, or 0 when the
/// call did not return.
#[must_use]
pub fn child_execve(
    path: &CString,
    argv: &[*const libc::c_char],
    envp: &[*const libc::c_char],
) -> i32 {
    // SAFETY: every pointer comes from a `CString` that lives across the call,
    // and both arrays end with a null pointer.
    unsafe { libc::execve(path.as_ptr(), argv.as_ptr(), envp.as_ptr()) };
    child_errno()
}

/// Ends the child with `code`.
pub fn child_exit(code: i32) -> ! {
    // SAFETY: `_exit` ends this process and never returns.
    unsafe { libc::_exit(code) }
}

/// Writes the start status of the child. It returns the bytes written, or a
/// negative value.
#[must_use]
pub fn child_write(fd: RawFd, bytes: &[u8]) -> isize {
    // SAFETY: the buffer is valid for its length and the descriptor is open.
    unsafe { libc::write(fd, bytes.as_ptr().cast::<libc::c_void>(), bytes.len()) }
}

/// Resets the signal mask and the dispositions of the common signals.
pub fn child_reset_signals() {
    // SAFETY: a zeroed signal set is a valid output buffer for `sigemptyset`.
    let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
    // SAFETY: `set` is a valid signal set.
    unsafe {
        libc::sigemptyset(std::ptr::addr_of_mut!(set));
        libc::sigprocmask(
            libc::SIG_SETMASK,
            std::ptr::addr_of!(set),
            std::ptr::null_mut(),
        );
    }
    for signal in [
        libc::SIGPIPE,
        libc::SIGCHLD,
        libc::SIGTERM,
        libc::SIGINT,
        libc::SIGHUP,
        libc::SIGQUIT,
        libc::SIGUSR1,
        libc::SIGUSR2,
    ] {
        // SAFETY: a zeroed `sigaction` holds `SIG_DFL` after the assignment.
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = libc::SIG_DFL;
        // SAFETY: `action` is a valid signal action for one signal.
        unsafe {
            libc::sigemptyset(std::ptr::addr_of_mut!(action.sa_mask));
            libc::sigaction(signal, std::ptr::addr_of!(action), std::ptr::null_mut());
        }
    }
}
