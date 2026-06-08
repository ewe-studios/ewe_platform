/// Platform abstractions for the ptrace interceptor.
///
/// Provides errno translation (macOS BSD errno → Linux errno) and error helpers.
///
/// ## Errno Translation
///
/// The VFS error protocol always expects Linux errno values. On Linux, errors pass through
/// unchanged. On macOS (for future cross-platform ptrace support), BSD errno values are
/// mapped to their Linux equivalents.
///
/// ## openat2 Containment
///
/// `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS)` (Linux 5.6+)
/// provides kernel-enforced path containment. Availability is probed at init time.
/// Falls back to regular `openat` on older kernels.

use std::{io, os::fd::RawFd};

//--------------------------------------------------------------------------------------------------
// Linux errno constants (for macOS translation)
//--------------------------------------------------------------------------------------------------

const LINUX_EPERM: i32 = 1;
const LINUX_ENOENT: i32 = 2;
const LINUX_ESRCH: i32 = 3;
const LINUX_EINTR: i32 = 4;
const LINUX_EIO: i32 = 5;
const LINUX_ENXIO: i32 = 6;
const LINUX_ENOEXEC: i32 = 8;
const LINUX_EBADF: i32 = 9;
const LINUX_ECHILD: i32 = 10;
const LINUX_EAGAIN: i32 = 11;
const LINUX_ENOMEM: i32 = 12;
const LINUX_EACCES: i32 = 13;
const LINUX_EFAULT: i32 = 14;
const LINUX_ENOTBLK: i32 = 15;
const LINUX_EBUSY: i32 = 16;
const LINUX_EEXIST: i32 = 17;
const LINUX_EXDEV: i32 = 18;
const LINUX_ENODEV: i32 = 19;
const LINUX_ENOTDIR: i32 = 20;
const LINUX_EISDIR: i32 = 21;
const LINUX_EINVAL: i32 = 22;
const LINUX_ENFILE: i32 = 23;
const LINUX_EMFILE: i32 = 24;
const LINUX_ENOTTY: i32 = 25;
const LINUX_ETXTBSY: i32 = 26;
const LINUX_EFBIG: i32 = 27;
const LINUX_ENOSPC: i32 = 28;
const LINUX_ESPIPE: i32 = 29;
const LINUX_EROFS: i32 = 30;
const LINUX_EMLINK: i32 = 31;
const LINUX_EPIPE: i32 = 32;
const LINUX_ENAMETOOLONG: i32 = 36;
const LINUX_ENOSYS: i32 = 38;
const LINUX_ENOTEMPTY: i32 = 39;
const LINUX_ELOOP: i32 = 40;
const LINUX_ERANGE: i32 = 34;
const LINUX_EDEADLK: i32 = 35;
const LINUX_ENOLCK: i32 = 37;
const LINUX_ENOMSG: i32 = 42;
const LINUX_EIDRM: i32 = 43;
const LINUX_ENOSTR: i32 = 60;
const LINUX_ENODATA: i32 = 61;
const LINUX_ETIME: i32 = 62;
const LINUX_ENOSR: i32 = 63;
const LINUX_EREMOTE: i32 = 66;
const LINUX_ENOLINK: i32 = 67;
const LINUX_EPROTO: i32 = 71;
const LINUX_EMULTIHOP: i32 = 72;
const LINUX_EBADMSG: i32 = 74;
const LINUX_EOVERFLOW: i32 = 75;
const LINUX_EILSEQ: i32 = 84;
const LINUX_EUSERS: i32 = 87;
const LINUX_ENOTSOCK: i32 = 88;
const LINUX_EDESTADDRREQ: i32 = 89;
const LINUX_EMSGSIZE: i32 = 90;
const LINUX_EPROTOTYPE: i32 = 91;
const LINUX_ENOPROTOOPT: i32 = 92;
const LINUX_EPROTONOSUPPORT: i32 = 93;
const LINUX_ESOCKTNOSUPPORT: i32 = 94;
const LINUX_EOPNOTSUPP: i32 = 95;
const LINUX_EPFNOSUPPORT: i32 = 96;
const LINUX_EAFNOSUPPORT: i32 = 97;
const LINUX_EADDRINUSE: i32 = 98;
const LINUX_EADDRNOTAVAIL: i32 = 99;
const LINUX_ENETDOWN: i32 = 100;
const LINUX_ENETUNREACH: i32 = 101;
const LINUX_ENETRESET: i32 = 102;
const LINUX_ECONNABORTED: i32 = 103;
const LINUX_ECONNRESET: i32 = 104;
const LINUX_ENOBUFS: i32 = 105;
const LINUX_EISCONN: i32 = 106;
const LINUX_ENOTCONN: i32 = 107;
const LINUX_ESHUTDOWN: i32 = 108;
const LINUX_ETOOMANYREFS: i32 = 109;
const LINUX_ETIMEDOUT: i32 = 110;
const LINUX_ECONNREFUSED: i32 = 111;
const LINUX_EHOSTDOWN: i32 = 112;
const LINUX_EHOSTUNREACH: i32 = 113;
const LINUX_EALREADY: i32 = 114;
const LINUX_EINPROGRESS: i32 = 115;
const LINUX_ESTALE: i32 = 116;
const LINUX_EDQUOT: i32 = 122;
const LINUX_ECANCELED: i32 = 125;
const LINUX_EOWNERDEAD: i32 = 130;
const LINUX_ENOTRECOVERABLE: i32 = 131;

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Translate a native OS error to a Linux errno value.
///
/// On Linux this is an identity function. On macOS, BSD errno values are
/// mapped to their Linux equivalents, since the VFS protocol always
/// expects Linux errno values.
#[cfg(target_os = "linux")]
pub(crate) fn linux_error(error: io::Error) -> io::Error {
    error
}

/// Translate a native errno to its Linux equivalent.
#[cfg(target_os = "macos")]
fn linux_errno_raw(errno: i32) -> i32 {
    match errno {
        libc::EPERM => LINUX_EPERM,
        libc::ENOENT => LINUX_ENOENT,
        libc::ESRCH => LINUX_ESRCH,
        libc::EINTR => LINUX_EINTR,
        libc::EIO => LINUX_EIO,
        libc::ENXIO => LINUX_ENXIO,
        libc::ENOEXEC => LINUX_ENOEXEC,
        libc::EBADF => LINUX_EBADF,
        libc::ECHILD => LINUX_ECHILD,
        libc::EDEADLK => LINUX_EDEADLK,
        libc::ENOMEM => LINUX_ENOMEM,
        libc::EACCES => LINUX_EACCES,
        libc::EFAULT => LINUX_EFAULT,
        libc::ENOTBLK => LINUX_ENOTBLK,
        libc::EBUSY => LINUX_EBUSY,
        libc::EEXIST => LINUX_EEXIST,
        libc::EXDEV => LINUX_EXDEV,
        libc::ENODEV => LINUX_ENODEV,
        libc::ENOTDIR => LINUX_ENOTDIR,
        libc::EISDIR => LINUX_EISDIR,
        libc::EINVAL => LINUX_EINVAL,
        libc::ENFILE => LINUX_ENFILE,
        libc::EMFILE => LINUX_EMFILE,
        libc::ENOTTY => LINUX_ENOTTY,
        libc::ETXTBSY => LINUX_ETXTBSY,
        libc::EFBIG => LINUX_EFBIG,
        libc::ENOSPC => LINUX_ENOSPC,
        libc::ESPIPE => LINUX_ESPIPE,
        libc::EROFS => LINUX_EROFS,
        libc::EMLINK => LINUX_EMLINK,
        libc::EPIPE => LINUX_EPIPE,
        libc::EDOM => LINUX_EDOM,
        libc::EAGAIN => LINUX_EAGAIN,
        libc::EINPROGRESS => LINUX_EINPROGRESS,
        libc::EALREADY => LINUX_EALREADY,
        libc::ENOTSOCK => LINUX_ENOTSOCK,
        libc::EDESTADDRREQ => LINUX_EDESTADDRREQ,
        libc::EMSGSIZE => LINUX_EMSGSIZE,
        libc::EPROTOTYPE => LINUX_EPROTOTYPE,
        libc::ENOPROTOOPT => LINUX_ENOPROTOOPT,
        libc::EPROTONOSUPPORT => LINUX_EPROTONOSUPPORT,
        libc::ESOCKTNOSUPPORT => LINUX_ESOCKTNOSUPPORT,
        libc::EPFNOSUPPORT => LINUX_EPFNOSUPPORT,
        libc::EAFNOSUPPORT => LINUX_EAFNOSUPPORT,
        libc::EADDRINUSE => LINUX_EADDRINUSE,
        libc::EADDRNOTAVAIL => LINUX_EADDRNOTAVAIL,
        libc::ENETDOWN => LINUX_ENETDOWN,
        libc::ENETUNREACH => LINUX_ENETUNREACH,
        libc::ENETRESET => LINUX_ENETRESET,
        libc::ECONNABORTED => LINUX_ECONNABORTED,
        libc::ECONNRESET => LINUX_ECONNRESET,
        libc::ENOBUFS => LINUX_ENOBUFS,
        libc::EISCONN => LINUX_EISCONN,
        libc::ENOTCONN => LINUX_ENOTCONN,
        libc::ESHUTDOWN => LINUX_ESHUTDOWN,
        libc::ETOOMANYREFS => LINUX_ETOOMANYREFS,
        libc::ETIMEDOUT => LINUX_ETIMEDOUT,
        libc::ECONNREFUSED => LINUX_ECONNREFUSED,
        libc::ELOOP => LINUX_ELOOP,
        libc::ENAMETOOLONG => LINUX_ENAMETOOLONG,
        libc::EHOSTDOWN => LINUX_EHOSTDOWN,
        libc::EHOSTUNREACH => LINUX_EHOSTUNREACH,
        libc::ENOTEMPTY => LINUX_ENOTEMPTY,
        libc::EUSERS => LINUX_EUSERS,
        libc::EDQUOT => LINUX_EDQUOT,
        libc::ESTALE => LINUX_ESTALE,
        libc::EREMOTE => LINUX_EREMOTE,
        libc::ENOLCK => LINUX_ENOLCK,
        libc::ENOSYS => LINUX_ENOSYS,
        libc::EOVERFLOW => LINUX_EOVERFLOW,
        libc::ECANCELED => LINUX_ECANCELED,
        libc::EIDRM => LINUX_EIDRM,
        libc::ENOMSG => LINUX_ENOMSG,
        libc::EILSEQ => LINUX_EILSEQ,
        libc::EBADMSG => LINUX_EBADMSG,
        libc::EMULTIHOP => LINUX_EMULTIHOP,
        libc::ENODATA => LINUX_ENODATA,
        libc::ENOLINK => LINUX_ENOLINK,
        libc::ENOSR => LINUX_ENOSR,
        libc::ENOSTR => LINUX_ENOSTR,
        libc::EPROTO => LINUX_EPROTO,
        libc::ETIME => LINUX_ETIME,
        libc::EOPNOTSUPP => LINUX_EOPNOTSUPP,
        libc::ENOTRECOVERABLE => LINUX_ENOTRECOVERABLE,
        libc::EOWNERDEAD => LINUX_EOWNERDEAD,
        _ => LINUX_EIO,
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn linux_error(error: io::Error) -> io::Error {
    io::Error::from_raw_os_error(linux_errno_raw(error.raw_os_error().unwrap_or(libc::EIO)))
}

/// Create an `io::Error` with Linux `EIO`.
pub(crate) fn eio() -> io::Error {
    io::Error::from_raw_os_error(LINUX_EIO)
}

/// Create an `io::Error` with Linux `EBADF`.
pub(crate) fn ebadf() -> io::Error {
    io::Error::from_raw_os_error(LINUX_EBADF)
}

/// Create an `io::Error` with Linux `EINVAL`.
pub(crate) fn einval() -> io::Error {
    io::Error::from_raw_os_error(LINUX_EINVAL)
}

/// Create an `io::Error` with Linux `EACCES`.
pub(crate) fn eacces() -> io::Error {
    io::Error::from_raw_os_error(LINUX_EACCES)
}

/// Create an `io::Error` with Linux `EPERM`.
pub(crate) fn eperm() -> io::Error {
    io::Error::from_raw_os_error(LINUX_EPERM)
}

/// Create an `io::Error` with Linux `ENOSYS`.
pub(crate) fn enosys() -> io::Error {
    io::Error::from_raw_os_error(LINUX_ENOSYS)
}

/// Create an `io::Error` with Linux `ENOENT`.
pub(crate) fn enoent() -> io::Error {
    io::Error::from_raw_os_error(LINUX_ENOENT)
}

/// Create an `io::Error` with Linux `EISDIR`.
pub(crate) fn eisdir() -> io::Error {
    io::Error::from_raw_os_error(LINUX_EISDIR)
}

/// Create an `io::Error` with Linux `ENOTDIR`.
pub(crate) fn enotdir() -> io::Error {
    io::Error::from_raw_os_error(LINUX_ENOTDIR)
}

/// Create an `io::Error` with Linux `ENOTEMPTY`.
pub(crate) fn enotempty() -> io::Error {
    io::Error::from_raw_os_error(LINUX_ENOTEMPTY)
}

/// Create an `io::Error` with Linux `ELOOP`.
pub(crate) fn eloop() -> io::Error {
    io::Error::from_raw_os_error(LINUX_ELOOP)
}

/// Create an `io::Error` with Linux `ENAMETOOLONG`.
pub(crate) fn enametoolong() -> io::Error {
    io::Error::from_raw_os_error(LINUX_ENAMETOOLONG)
}

/// Create an `io::Error` with Linux `EEXIST`.
pub(crate) fn eexist() -> io::Error {
    io::Error::from_raw_os_error(LINUX_EEXIST)
}

/// Create an `io::Error` with Linux `ENOSPC`.
pub(crate) fn enospc() -> io::Error {
    io::Error::from_raw_os_error(LINUX_ENOSPC)
}

/// Create an `io::Error` with Linux `EROFS`.
pub(crate) fn erofs() -> io::Error {
    io::Error::from_raw_os_error(LINUX_EROFS)
}

//--------------------------------------------------------------------------------------------------
// openat2 containment (Linux 5.6+)
//--------------------------------------------------------------------------------------------------

#[cfg(target_os = "linux")]
pub(crate) const RESOLVE_BENEATH: u64 = 0x08;
#[cfg(target_os = "linux")]
pub(crate) const RESOLVE_NO_SYMLINKS: u64 = 0x04;
#[cfg(target_os = "linux")]
pub(crate) const RESOLVE_NO_MAGICLINKS: u64 = 0x02;

#[cfg(target_os = "linux")]
#[repr(C)]
pub(crate) struct OpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}

#[cfg(target_os = "linux")]
const SYS_OPENAT2: libc::c_long = 437;

/// Probe whether the `openat2` syscall is available (Linux 5.6+).
#[cfg(target_os = "linux")]
pub(crate) fn probe_openat2() -> bool {
    let resolve = RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS;
    let how = OpenHow {
        flags: libc::O_CLOEXEC as u64 | libc::O_PATH as u64,
        mode: 0,
        resolve,
    };
    let ret = unsafe {
        libc::syscall(
            SYS_OPENAT2,
            libc::AT_FDCWD,
            c".".as_ptr(),
            &how as *const OpenHow,
            std::mem::size_of::<OpenHow>(),
        )
    };
    if ret >= 0 {
        unsafe { libc::close(ret as i32) };
        true
    } else {
        !matches!(
            io::Error::last_os_error().raw_os_error(),
            Some(libc::ENOSYS | libc::EINVAL)
        )
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eio_returns_correct_errno() {
        let err = eio();
        assert_eq!(err.raw_os_error(), Some(LINUX_EIO));
    }

    #[test]
    fn ebadf_returns_correct_errno() {
        let err = ebadf();
        assert_eq!(err.raw_os_error(), Some(LINUX_EBADF));
    }

    #[test]
    fn einval_returns_correct_errno() {
        let err = einval();
        assert_eq!(err.raw_os_error(), Some(LINUX_EINVAL));
    }

    #[test]
    fn eacces_returns_correct_errno() {
        let err = eacces();
        assert_eq!(err.raw_os_error(), Some(LINUX_EACCES));
    }

    #[test]
    fn eperm_returns_correct_errno() {
        let err = eperm();
        assert_eq!(err.raw_os_error(), Some(LINUX_EPERM));
    }

    #[test]
    fn enosys_returns_correct_errno() {
        let err = enosys();
        assert_eq!(err.raw_os_error(), Some(LINUX_ENOSYS));
    }

    #[test]
    fn enoent_returns_correct_errno() {
        let err = enoent();
        assert_eq!(err.raw_os_error(), Some(LINUX_ENOENT));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_error_is_identity_on_linux() {
        let err = io::Error::from_raw_os_error(libc::ENOENT);
        let mapped = linux_error(err);
        assert_eq!(mapped.raw_os_error(), Some(libc::ENOENT));
    }
}
