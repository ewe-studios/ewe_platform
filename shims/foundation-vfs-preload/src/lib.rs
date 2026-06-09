#![allow(non_camel_case_types)]

mod config;
mod fd_table;
mod real;

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};

use config::ShimConfig;
use fd_table::VirtualFdTable;
use once_cell::sync::Lazy;

static CONFIG: Lazy<ShimConfig> = Lazy::new(ShimConfig::from_env);
static FD_TABLE: Lazy<VirtualFdTable> = Lazy::new(VirtualFdTable::new);

const VIRTUAL_FD_BASE: c_int = 10_000;

fn is_virtual_fd(fd: c_int) -> bool {
    fd >= VIRTUAL_FD_BASE
}

fn path_matches_prefix(path: &str) -> bool {
    CONFIG.prefixes.iter().any(|p| path.starts_with(p))
}

unsafe fn c_path_to_str(path: *const c_char) -> Option<&'static str> {
    if path.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(path) }.to_str().ok()
}

// ── open ──
// Use 3-arg form — the kernel always accepts mode (ignores it without O_CREAT).
// This avoids unstable c_variadic.

#[unsafe(no_mangle)]
pub unsafe extern "C" fn open(pathname: *const c_char, flags: c_int, mode: libc::mode_t) -> c_int {
    if let Some(path) = unsafe { c_path_to_str(pathname) } {
        if path_matches_prefix(path) {
            return handle_virtual_open(path, flags, mode);
        }
    }

    unsafe { real::open(pathname, flags, mode) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn open64(pathname: *const c_char, flags: c_int, mode: libc::mode_t) -> c_int {
    if let Some(path) = unsafe { c_path_to_str(pathname) } {
        if path_matches_prefix(path) {
            return handle_virtual_open(path, flags, mode);
        }
    }

    unsafe { real::open64(pathname, flags, mode) }
}

// ── close ──

#[unsafe(no_mangle)]
pub unsafe extern "C" fn close(fd: c_int) -> c_int {
    if is_virtual_fd(fd) {
        FD_TABLE.remove(fd);
        return 0;
    }
    unsafe { real::close(fd) }
}

// ── read / write ──

#[unsafe(no_mangle)]
pub unsafe extern "C" fn read(fd: c_int, buf: *mut c_void, count: libc::size_t) -> libc::ssize_t {
    if is_virtual_fd(fd) {
        return handle_virtual_read(fd, buf, count);
    }
    unsafe { real::read(fd, buf, count) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn write(fd: c_int, buf: *const c_void, count: libc::size_t) -> libc::ssize_t {
    if is_virtual_fd(fd) {
        return handle_virtual_write(fd, buf, count);
    }
    unsafe { real::write(fd, buf, count) }
}

// ── stat ──

#[unsafe(no_mangle)]
pub unsafe extern "C" fn stat(pathname: *const c_char, statbuf: *mut libc::stat) -> c_int {
    if let Some(path) = unsafe { c_path_to_str(pathname) } {
        if path_matches_prefix(path) {
            return handle_virtual_stat(path, statbuf);
        }
    }
    unsafe { real::stat(pathname, statbuf) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lstat(pathname: *const c_char, statbuf: *mut libc::stat) -> c_int {
    if let Some(path) = unsafe { c_path_to_str(pathname) } {
        if path_matches_prefix(path) {
            return handle_virtual_stat(path, statbuf);
        }
    }
    unsafe { real::lstat(pathname, statbuf) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fstat(fd: c_int, statbuf: *mut libc::stat) -> c_int {
    if is_virtual_fd(fd) {
        if let Some(path) = FD_TABLE.path(fd) {
            return handle_virtual_stat(&path, statbuf);
        }
        set_errno(libc::EBADF);
        return -1;
    }
    unsafe { real::fstat(fd, statbuf) }
}

// ── access ──

#[unsafe(no_mangle)]
pub unsafe extern "C" fn access(pathname: *const c_char, mode: c_int) -> c_int {
    if let Some(path) = unsafe { c_path_to_str(pathname) } {
        if path_matches_prefix(path) {
            return handle_virtual_access(path, mode);
        }
    }
    unsafe { real::access(pathname, mode) }
}

// ── lseek ──

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lseek(fd: c_int, offset: libc::off_t, whence: c_int) -> libc::off_t {
    if is_virtual_fd(fd) {
        return handle_virtual_lseek(fd, offset, whence);
    }
    unsafe { real::lseek(fd, offset, whence) }
}

// ── mkdir / unlink / rename / rmdir ──

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mkdir(pathname: *const c_char, mode: libc::mode_t) -> c_int {
    if let Some(path) = unsafe { c_path_to_str(pathname) } {
        if path_matches_prefix(path) {
            return handle_virtual_mkdir(path, mode);
        }
    }
    unsafe { real::mkdir(pathname, mode) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn unlink(pathname: *const c_char) -> c_int {
    if let Some(path) = unsafe { c_path_to_str(pathname) } {
        if path_matches_prefix(path) {
            return handle_virtual_unlink(path);
        }
    }
    unsafe { real::unlink(pathname) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rename(
    oldpath: *const c_char,
    newpath: *const c_char,
) -> c_int {
    let old = unsafe { c_path_to_str(oldpath) };
    let new = unsafe { c_path_to_str(newpath) };
    if let (Some(o), Some(n)) = (old, new) {
        if path_matches_prefix(o) || path_matches_prefix(n) {
            return handle_virtual_rename(o, n);
        }
    }
    unsafe { real::rename(oldpath, newpath) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmdir(pathname: *const c_char) -> c_int {
    if let Some(path) = unsafe { c_path_to_str(pathname) } {
        if path_matches_prefix(path) {
            return handle_virtual_rmdir(path);
        }
    }
    unsafe { real::rmdir(pathname) }
}

// ── Helpers ──

fn set_errno(err: c_int) {
    unsafe { *libc::__errno_location() = err };
}

fn handle_virtual_open(_path: &str, _flags: c_int, _mode: libc::mode_t) -> c_int {
    // TODO: connect to VFS daemon via IPC, send open request, allocate virtual fd
    set_errno(libc::ENOSYS);
    -1
}

fn handle_virtual_read(_fd: c_int, _buf: *mut c_void, _count: libc::size_t) -> libc::ssize_t {
    set_errno(libc::ENOSYS);
    -1
}

fn handle_virtual_write(_fd: c_int, _buf: *const c_void, _count: libc::size_t) -> libc::ssize_t {
    set_errno(libc::ENOSYS);
    -1
}

fn handle_virtual_stat(_path: &str, _statbuf: *mut libc::stat) -> c_int {
    set_errno(libc::ENOSYS);
    -1
}

fn handle_virtual_access(_path: &str, _mode: c_int) -> c_int {
    set_errno(libc::ENOSYS);
    -1
}

fn handle_virtual_lseek(_fd: c_int, _offset: libc::off_t, _whence: c_int) -> libc::off_t {
    set_errno(libc::ENOSYS);
    -1
}

fn handle_virtual_mkdir(_path: &str, _mode: libc::mode_t) -> c_int {
    set_errno(libc::ENOSYS);
    -1
}

fn handle_virtual_unlink(_path: &str) -> c_int {
    set_errno(libc::ENOSYS);
    -1
}

fn handle_virtual_rename(_old: &str, _new: &str) -> c_int {
    set_errno(libc::ENOSYS);
    -1
}

fn handle_virtual_rmdir(_path: &str) -> c_int {
    set_errno(libc::ENOSYS);
    -1
}
