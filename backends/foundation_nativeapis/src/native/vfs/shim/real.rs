use std::os::raw::{c_char, c_int, c_void};
use std::sync::OnceLock;

use libc::{mode_t, off_t, size_t, ssize_t};

macro_rules! real_fn {
    ($name:ident, $sig:ty) => {
        static $name: OnceLock<$sig> = OnceLock::new();
    };
}

real_fn!(REAL_OPEN, unsafe extern "C" fn(*const c_char, c_int, mode_t) -> c_int);
real_fn!(REAL_OPEN64, unsafe extern "C" fn(*const c_char, c_int, mode_t) -> c_int);
real_fn!(REAL_CLOSE, unsafe extern "C" fn(c_int) -> c_int);
real_fn!(REAL_READ, unsafe extern "C" fn(c_int, *mut c_void, size_t) -> ssize_t);
real_fn!(REAL_WRITE, unsafe extern "C" fn(c_int, *const c_void, size_t) -> ssize_t);
real_fn!(REAL_STAT, unsafe extern "C" fn(*const c_char, *mut libc::stat) -> c_int);
real_fn!(REAL_LSTAT, unsafe extern "C" fn(*const c_char, *mut libc::stat) -> c_int);
real_fn!(REAL_FSTAT, unsafe extern "C" fn(c_int, *mut libc::stat) -> c_int);
real_fn!(REAL_ACCESS, unsafe extern "C" fn(*const c_char, c_int) -> c_int);
real_fn!(REAL_LSEEK, unsafe extern "C" fn(c_int, off_t, c_int) -> off_t);
real_fn!(REAL_MKDIR, unsafe extern "C" fn(*const c_char, mode_t) -> c_int);
real_fn!(REAL_UNLINK, unsafe extern "C" fn(*const c_char) -> c_int);
real_fn!(REAL_RENAME, unsafe extern "C" fn(*const c_char, *const c_char) -> c_int);
real_fn!(REAL_RMDIR, unsafe extern "C" fn(*const c_char) -> c_int);
real_fn!(REAL_OPENAT, unsafe extern "C" fn(c_int, *const c_char, c_int, mode_t) -> c_int);

pub unsafe fn openat(dirfd: c_int, pathname: *const c_char, flags: c_int, mode: mode_t) -> c_int {
    let f = REAL_OPENAT.get_or_init(|| unsafe { resolve(b"openat\0") });
    unsafe { f(dirfd, pathname, flags, mode) }
}

unsafe fn resolve<T: Copy>(name: &[u8]) -> T {
    let ptr = unsafe { libc::dlsym(libc::RTLD_NEXT, name.as_ptr() as *const c_char) };
    assert!(!ptr.is_null(), "dlsym failed to resolve {:?}", std::str::from_utf8(name));
    unsafe { std::mem::transmute_copy(&ptr) }
}

pub unsafe fn open(pathname: *const c_char, flags: c_int, mode: mode_t) -> c_int {
    let f = REAL_OPEN.get_or_init(|| unsafe { resolve(b"open\0") });
    unsafe { f(pathname, flags, mode) }
}

pub unsafe fn open64(pathname: *const c_char, flags: c_int, mode: mode_t) -> c_int {
    let f = REAL_OPEN64.get_or_init(|| unsafe { resolve(b"open64\0") });
    unsafe { f(pathname, flags, mode) }
}

pub unsafe fn close(fd: c_int) -> c_int {
    let f = REAL_CLOSE.get_or_init(|| unsafe { resolve(b"close\0") });
    unsafe { f(fd) }
}

pub unsafe fn read(fd: c_int, buf: *mut c_void, count: size_t) -> ssize_t {
    let f = REAL_READ.get_or_init(|| unsafe { resolve(b"read\0") });
    unsafe { f(fd, buf, count) }
}

pub unsafe fn write(fd: c_int, buf: *const c_void, count: size_t) -> ssize_t {
    let f = REAL_WRITE.get_or_init(|| unsafe { resolve(b"write\0") });
    unsafe { f(fd, buf, count) }
}

pub unsafe fn stat(pathname: *const c_char, statbuf: *mut libc::stat) -> c_int {
    let f = REAL_STAT.get_or_init(|| unsafe { resolve(b"stat\0") });
    unsafe { f(pathname, statbuf) }
}

pub unsafe fn lstat(pathname: *const c_char, statbuf: *mut libc::stat) -> c_int {
    let f = REAL_LSTAT.get_or_init(|| unsafe { resolve(b"lstat\0") });
    unsafe { f(pathname, statbuf) }
}

pub unsafe fn fstat(fd: c_int, statbuf: *mut libc::stat) -> c_int {
    let f = REAL_FSTAT.get_or_init(|| unsafe { resolve(b"fstat\0") });
    unsafe { f(fd, statbuf) }
}

pub unsafe fn access(pathname: *const c_char, mode: c_int) -> c_int {
    let f = REAL_ACCESS.get_or_init(|| unsafe { resolve(b"access\0") });
    unsafe { f(pathname, mode) }
}

pub unsafe fn lseek(fd: c_int, offset: off_t, whence: c_int) -> off_t {
    let f = REAL_LSEEK.get_or_init(|| unsafe { resolve(b"lseek\0") });
    unsafe { f(fd, offset, whence) }
}

pub unsafe fn mkdir(pathname: *const c_char, mode: mode_t) -> c_int {
    let f = REAL_MKDIR.get_or_init(|| unsafe { resolve(b"mkdir\0") });
    unsafe { f(pathname, mode) }
}

pub unsafe fn unlink(pathname: *const c_char) -> c_int {
    let f = REAL_UNLINK.get_or_init(|| unsafe { resolve(b"unlink\0") });
    unsafe { f(pathname) }
}

pub unsafe fn rename(oldpath: *const c_char, newpath: *const c_char) -> c_int {
    let f = REAL_RENAME.get_or_init(|| unsafe { resolve(b"rename\0") });
    unsafe { f(oldpath, newpath) }
}

pub unsafe fn rmdir(pathname: *const c_char) -> c_int {
    let f = REAL_RMDIR.get_or_init(|| unsafe { resolve(b"rmdir\0") });
    unsafe { f(pathname) }
}

// ── Directory streams ──
//
// WHY: `opendir`/`readdir`/`closedir` are interposed by this shim, so calling
// `libc::opendir` from inside the shim resolves *back to the shim* — both when
// the crate is statically linked into a binary and when it is `LD_PRELOAD`ed,
// because a preloaded definition interposes on itself. That is unbounded
// recursion; it overflowed the stack the moment any non-virtual directory was
// opened. Every interposed symbol must reach the real implementation through
// `RTLD_NEXT`, never through `libc::`.

real_fn!(REAL_OPENDIR, unsafe extern "C" fn(*const c_char) -> *mut libc::DIR);
real_fn!(REAL_READDIR, unsafe extern "C" fn(*mut libc::DIR) -> *mut libc::dirent);
real_fn!(REAL_CLOSEDIR, unsafe extern "C" fn(*mut libc::DIR) -> c_int);

/// # Safety
/// `pathname` must be a valid NUL-terminated C string.
pub unsafe fn opendir(pathname: *const c_char) -> *mut libc::DIR {
    let f = REAL_OPENDIR.get_or_init(|| unsafe { resolve(b"opendir\0") });
    unsafe { f(pathname) }
}

/// # Safety
/// `dirp` must be a directory stream returned by [`opendir`].
pub unsafe fn readdir(dirp: *mut libc::DIR) -> *mut libc::dirent {
    let f = REAL_READDIR.get_or_init(|| unsafe { resolve(b"readdir\0") });
    unsafe { f(dirp) }
}

/// # Safety
/// `dirp` must be a directory stream returned by [`opendir`], not used again.
pub unsafe fn closedir(dirp: *mut libc::DIR) -> c_int {
    let f = REAL_CLOSEDIR.get_or_init(|| unsafe { resolve(b"closedir\0") });
    unsafe { f(dirp) }
}

// ── The remaining interposed symbols ──
//
// Same rule as the directory stream above: a symbol this shim exports must never
// be reached through `libc::`, or the call lands back in the shim. Under
// `LD_PRELOAD` a preloaded definition interposes on itself just as it does when
// statically linked, so this recursed in production, not only in tests.

real_fn!(REAL_CHMOD, unsafe extern "C" fn(*const c_char, mode_t) -> c_int);
real_fn!(REAL_FCHMOD, unsafe extern "C" fn(c_int, mode_t) -> c_int);
real_fn!(REAL_FSYNC, unsafe extern "C" fn(c_int) -> c_int);
real_fn!(REAL_FTRUNCATE, unsafe extern "C" fn(c_int, off_t) -> c_int);
real_fn!(REAL_TRUNCATE, unsafe extern "C" fn(*const c_char, off_t) -> c_int);
real_fn!(REAL_PREAD, unsafe extern "C" fn(c_int, *mut c_void, size_t, off_t) -> ssize_t);
real_fn!(REAL_PWRITE, unsafe extern "C" fn(c_int, *const c_void, size_t, off_t) -> ssize_t);
real_fn!(REAL_READLINK, unsafe extern "C" fn(*const c_char, *mut c_char, size_t) -> ssize_t);
real_fn!(REAL_SYMLINK, unsafe extern "C" fn(*const c_char, *const c_char) -> c_int);

/// # Safety
/// `pathname` must be a valid NUL-terminated C string.
pub unsafe fn chmod(pathname: *const c_char, mode: mode_t) -> c_int {
    let f = REAL_CHMOD.get_or_init(|| unsafe { resolve(b"chmod\0") });
    unsafe { f(pathname, mode) }
}

/// # Safety
/// `fd` must be an open descriptor.
pub unsafe fn fchmod(fd: c_int, mode: mode_t) -> c_int {
    let f = REAL_FCHMOD.get_or_init(|| unsafe { resolve(b"fchmod\0") });
    unsafe { f(fd, mode) }
}

/// # Safety
/// `fd` must be an open descriptor.
pub unsafe fn fsync(fd: c_int) -> c_int {
    let f = REAL_FSYNC.get_or_init(|| unsafe { resolve(b"fsync\0") });
    unsafe { f(fd) }
}

/// # Safety
/// `fd` must be an open descriptor.
pub unsafe fn ftruncate(fd: c_int, length: off_t) -> c_int {
    let f = REAL_FTRUNCATE.get_or_init(|| unsafe { resolve(b"ftruncate\0") });
    unsafe { f(fd, length) }
}

/// # Safety
/// `pathname` must be a valid NUL-terminated C string.
pub unsafe fn truncate(pathname: *const c_char, length: off_t) -> c_int {
    let f = REAL_TRUNCATE.get_or_init(|| unsafe { resolve(b"truncate\0") });
    unsafe { f(pathname, length) }
}

/// # Safety
/// `fd` must be open and `buf` writable for `count` bytes.
pub unsafe fn pread(fd: c_int, buf: *mut c_void, count: size_t, offset: off_t) -> ssize_t {
    let f = REAL_PREAD.get_or_init(|| unsafe { resolve(b"pread\0") });
    unsafe { f(fd, buf, count, offset) }
}

/// # Safety
/// `fd` must be open and `buf` readable for `count` bytes.
pub unsafe fn pwrite(fd: c_int, buf: *const c_void, count: size_t, offset: off_t) -> ssize_t {
    let f = REAL_PWRITE.get_or_init(|| unsafe { resolve(b"pwrite\0") });
    unsafe { f(fd, buf, count, offset) }
}

/// # Safety
/// `pathname` must be a valid NUL-terminated C string and `buf` writable for
/// `bufsiz` bytes.
pub unsafe fn readlink(pathname: *const c_char, buf: *mut c_char, bufsiz: size_t) -> ssize_t {
    let f = REAL_READLINK.get_or_init(|| unsafe { resolve(b"readlink\0") });
    unsafe { f(pathname, buf, bufsiz) }
}

/// # Safety
/// Both arguments must be valid NUL-terminated C strings.
pub unsafe fn symlink(target: *const c_char, linkpath: *const c_char) -> c_int {
    let f = REAL_SYMLINK.get_or_init(|| unsafe { resolve(b"symlink\0") });
    unsafe { f(target, linkpath) }
}
