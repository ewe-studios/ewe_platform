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
