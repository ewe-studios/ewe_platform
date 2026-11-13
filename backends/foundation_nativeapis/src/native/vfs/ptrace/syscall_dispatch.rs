/// Shared syscall → VFS dispatch logic.
///
/// This module maps raw syscall numbers + register arguments to `VfsFileSystem` method calls.
/// Used by both the nix and reverie backends — the backend-specific code handles:
/// 1. Reading registers (syscall number + arguments)
/// 2. Calling the appropriate dispatch function here
/// 3. Writing the result back to registers/tracee memory
use nix::libc;

use std::sync::{Arc, Mutex};

use foundation_errstacks::ErrorTrace;

use super::memory;
use super::{syscall_nr::*, DynFs, MountTable, VirtualFdEntry, VirtualFdTable, FD_VIRTUAL_BASE};
use crate::shared::vfs::error::VfsError;
use crate::shared::vfs::types::OpenMode;

// ── Syscall argument structure ──

/// Syscall arguments read from tracee registers.
#[derive(Debug, Clone, Copy)]
pub struct SyscallArgs {
    pub nr: i64,
    pub arg0: u64, // rdi
    pub arg1: u64, // rsi
    pub arg2: u64, // rdx
    pub arg3: u64, // r10
    pub arg4: u64, // r8
    pub arg5: u64, // r9
}

/// Result of processing a syscall.
pub enum SyscallAction {
    /// Let the kernel handle it normally.
    Passthrough,
    /// Skip the syscall — return the given value as the result.
    Skip { return_value: i64 },
    /// Skip and return an error.
    SkipError { errno: i64 },
    /// Run `close(fd)` in place of the intercepted syscall, then report
    /// `return_value` as the result.
    ///
    /// `dup2(2)` must close the file descriptor at the new number before it
    /// returns. When the duplicated descriptor is virtual, the kernel has no
    /// `dup2` to run, so the backend rewrites the syscall into `close(fd)`.
    /// The backend hides the close result, because `dup2(2)` ignores an error
    /// from closing the destination.
    CloseThenSkip { fd: u64, return_value: i64 },
}

impl SyscallAction {
    #[must_use]
    pub fn passthrough() -> Self {
        Self::Passthrough
    }
    #[must_use]
    pub fn skip(return_value: i64) -> Self {
        Self::Skip { return_value }
    }
    #[must_use]
    pub fn skip_error(errno: i64) -> Self {
        Self::SkipError { errno }
    }
    #[must_use]
    pub fn close_then_skip(fd: u64, return_value: i64) -> Self {
        Self::CloseThenSkip { fd, return_value }
    }
}

// ── Dispatch logic ──

/// Process a syscall on entry (before the kernel executes it).
///
/// Determines whether to intercept this syscall or let it pass through.
/// For intercepted syscalls that need tracee memory access (e.g., reading path strings),
/// the entry handler does the reading. For syscalls that write to tracee memory (e.g., stat),
/// the exit handler handles the writing.
pub fn on_syscall_entry(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    // exit(60) and exit_group(231) must always passthrough — if we intercept them,
    // the process hangs because the tracer continues through the exit stop
    // but the process is already terminating.
    if args.nr == 60 || args.nr == 231 {
        return SyscallAction::passthrough();
    }

    match args.nr {
        // ── File open/create ──
        OPEN => handle_open(pid, args, mount_table, fd_table),
        OPENAT => handle_openat(pid, args, mount_table, fd_table),
        CREAT => handle_creat(pid, args, mount_table, fd_table),

        // ── FD operations (virtual FD only) ──
        READ if is_tracked_virtual_fd(pid, args.arg0, fd_table) => {
            handle_vfs_read(pid, args, fd_table)
        }
        WRITE if is_tracked_virtual_fd(pid, args.arg0, fd_table) => {
            handle_vfs_write(pid, args, fd_table)
        }
        PREAD64 if is_tracked_virtual_fd(pid, args.arg0, fd_table) => {
            handle_vfs_pread(pid, args, fd_table)
        }
        PWRITE64 if is_tracked_virtual_fd(pid, args.arg0, fd_table) => {
            handle_vfs_pwrite(pid, args, fd_table)
        }
        LSEEK if is_tracked_virtual_fd(pid, args.arg0, fd_table) => {
            handle_vfs_lseek(pid, args, fd_table)
        }
        CLOSE if is_tracked_virtual_fd(pid, args.arg0, fd_table) => {
            handle_vfs_close(pid, args, fd_table)
        }
        FSTAT if is_tracked_virtual_fd(pid, args.arg0, fd_table) => {
            handle_vfs_fstat(pid, args, fd_table)
        }
        FTRUNCATE if is_tracked_virtual_fd(pid, args.arg0, fd_table) => {
            handle_vfs_ftruncate(pid, args, fd_table)
        }
        FCHMOD if is_tracked_virtual_fd(pid, args.arg0, fd_table) => {
            handle_vfs_fchmod(pid, args, fd_table)
        }
        GETDENTS64 if is_tracked_virtual_fd(pid, args.arg0, fd_table) => {
            handle_vfs_getdents64(pid, args, fd_table)
        }

        // ── Path operations (virtual path only) ──
        STAT if resolve_tracee_path(pid, args.arg0)
            .map(|p| mount_table.is_virtual(&p))
            .unwrap_or(false) =>
        {
            handle_path_stat(pid, args, mount_table)
        }
        LSTAT
            if {
                match memory::read_tracee_string(pid, args.arg0, 4096) {
                    Ok(s) => mount_table.is_virtual(&s),
                    Err(_) => false,
                }
            } =>
        {
            handle_path_stat(pid, args, mount_table)
        }
        ACCESS
            if {
                match memory::read_tracee_string(pid, args.arg0, 4096) {
                    Ok(s) => mount_table.is_virtual(&s),
                    Err(_) => false,
                }
            } =>
        {
            handle_path_access(pid, args, mount_table)
        }
        UNLINK
            if {
                match memory::read_tracee_string(pid, args.arg0, 4096) {
                    Ok(s) => mount_table.is_virtual(&s),
                    Err(_) => false,
                }
            } =>
        {
            handle_path_remove(pid, args, mount_table)
        }
        MKDIR
            if {
                match memory::read_tracee_string(pid, args.arg0, 4096) {
                    Ok(s) => mount_table.is_virtual(&s),
                    Err(_) => false,
                }
            } =>
        {
            handle_path_mkdir(pid, args, mount_table)
        }
        RMDIR
            if {
                match memory::read_tracee_string(pid, args.arg0, 4096) {
                    Ok(s) => mount_table.is_virtual(&s),
                    Err(_) => false,
                }
            } =>
        {
            handle_path_remove(pid, args, mount_table)
        }
        RENAME
            if {
                match (
                    memory::read_tracee_string(pid, args.arg0, 4096),
                    memory::read_tracee_string(pid, args.arg1, 4096),
                ) {
                    (Ok(from), Ok(to)) => {
                        mount_table.is_virtual(&from) || mount_table.is_virtual(&to)
                    }
                    _ => false,
                }
            } =>
        {
            handle_path_rename(pid, args, mount_table)
        }
        SYMLINK
            if {
                match memory::read_tracee_string(pid, args.arg1, 4096) {
                    Ok(s) => mount_table.is_virtual(&s),
                    Err(_) => false,
                }
            } =>
        {
            handle_path_symlink(pid, args, mount_table)
        }
        READLINK
            if {
                match memory::read_tracee_string(pid, args.arg0, 4096) {
                    Ok(s) => mount_table.is_virtual(&s),
                    Err(_) => false,
                }
            } =>
        {
            handle_path_readlink(pid, args, mount_table)
        }
        CHMOD
            if {
                match memory::read_tracee_string(pid, args.arg0, 4096) {
                    Ok(s) => mount_table.is_virtual(&s),
                    Err(_) => false,
                }
            } =>
        {
            handle_path_chmod(pid, args, mount_table)
        }
        TRUNCATE
            if {
                match memory::read_tracee_string(pid, args.arg0, 4096) {
                    Ok(s) => mount_table.is_virtual(&s),
                    Err(_) => false,
                }
            } =>
        {
            handle_path_truncate(pid, args, mount_table)
        }

        // ── *at variants ──
        NEWFSTATAT => handle_newfstatat(pid, args, mount_table, fd_table),
        FACCESSAT => handle_faccessat(pid, args, mount_table, fd_table),
        UNLINKAT => handle_unlinkat(pid, args, mount_table, fd_table),
        MKDIRAT => handle_mkdirat(pid, args, mount_table, fd_table),
        RENAMEAT | RENAMEAT2 => handle_renameat(pid, args, mount_table, fd_table),
        READLINKAT => handle_readlinkat(pid, args, mount_table, fd_table),
        SYMLINKAT => handle_symlinkat(pid, args, mount_table, fd_table),
        FCHMODAT => handle_fchmodat(pid, args, mount_table, fd_table),

        // ── FD duplication ──
        DUP | DUP2 | DUP3 => handle_dup(pid, args, fd_table),

        // ── Everything else: pass through to kernel ──
        _ => SyscallAction::passthrough(),
    }
}

/// Resolve a path argument from tracee memory and check if it's virtual.
fn resolve_tracee_path(pid: nix::unistd::Pid, addr: u64) -> Option<String> {
    memory::read_tracee_string(pid, addr, 4096).ok()
}

/// Whether `fd` names a virtual entry for `pid`.
///
/// The table is the source of truth. A shell redirect calls `dup2` on the
/// virtual fd, which moves the entry to a low number (fd 1, for example), so
/// the `FD_VIRTUAL_BASE` range alone does not find it.
fn is_tracked_virtual_fd(pid: nix::unistd::Pid, fd: u64, fd_table: &VirtualFdTable) -> bool {
    VirtualFdTable::is_virtual_fd(fd) || fd_table.get(pid.as_raw() as u32, fd).is_some()
}

/// Resolve a path from tracee memory, handling AT_FDCWD for relative paths.
fn resolve_tracee_path_at(pid: nix::unistd::Pid, dirfd: i64, addr: u64) -> Option<String> {
    let path = memory::read_tracee_string(pid, addr, 4096).ok()?;
    if path.starts_with('/') {
        Some(path)
    } else if dirfd == libc::AT_FDCWD as i64 {
        // Read cwd from /proc/<pid>/cwd
        let cwd_path = format!("/proc/{}/cwd", pid.as_raw());
        match std::fs::read_link(&cwd_path) {
            Ok(cwd) => Some(format!("{}/{}", cwd.display(), path)),
            Err(_) => Some(path), // fallback: treat as absolute
        }
    } else {
        // dirfd is a real FD — we can't easily resolve relative paths
        // For now, treat as absolute
        Some(path)
    }
}

// ── File open handlers ──

/// Open a virtual path the way `open(2)` does.
///
/// `O_CREAT` with `O_EXCL` fails with `EEXIST` when the path exists. Without
/// `O_EXCL`, the path opens when it exists, and the call creates it when
/// `O_CREAT` is set and the path is missing. `O_TRUNC` truncates the opened
/// file to zero before the dispatch returns the virtual file descriptor.
fn open_virtual_path(
    pid: nix::unistd::Pid,
    fs: &DynFs,
    path: String,
    flags: i32,
    perm: u32,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let mode = flags_to_open_mode(flags);
    let create = flags & libc::O_CREAT != 0;

    let file = if create && flags & libc::O_EXCL != 0 {
        // `create` reports `AlreadyExists`, so an existing path fails either
        // way. Check first, because `open(2)` must report `EEXIST` and not
        // open the file.
        match fs.exists(&path) {
            Ok(true) => return SyscallAction::skip_error(libc::EEXIST as i64),
            Ok(false) => {}
            Err(e) => return SyscallAction::skip_error(vfs_error_to_errno(&e)),
        }
        match fs.create(&path, perm) {
            Ok(file) => file,
            Err(e) => return SyscallAction::skip_error(vfs_error_to_errno(&e)),
        }
    } else {
        // Open an existing file; create it only when `O_CREAT` asks for it and
        // the path is missing. Without `O_CREAT` a missing path stays
        // `ENOENT`, which `open_file` reports.
        match fs.open_file(&path, mode) {
            Ok(file) => file,
            Err(e) if create && is_not_found(&e) => match fs.create(&path, perm) {
                Ok(file) => file,
                Err(e) => return SyscallAction::skip_error(vfs_error_to_errno(&e)),
            },
            Err(e) => return SyscallAction::skip_error(vfs_error_to_errno(&e)),
        }
    };

    if flags & libc::O_TRUNC != 0 {
        if let Err(e) = file.truncate(0) {
            return SyscallAction::skip_error(vfs_error_to_errno(&e));
        }
    }

    let vfd = fd_table.alloc_fd();
    fd_table.insert(
        pid.as_raw() as u32,
        vfd,
        VirtualFdEntry::File {
            handle: file,
            path,
            offset: Arc::new(Mutex::new(0)),
            mode,
            flags,
        },
    );
    SyscallAction::skip(vfd as i64)
}

fn handle_open(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let path = match resolve_tracee_path(pid, args.arg0) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };
    let flags = args.arg1 as i32;
    // `open(2)` takes the mode as its third argument. The kernel reads it
    // only with `O_CREAT`, but pass the register through either way.
    let perm = args.arg2 as u32;

    if !mount_table.is_virtual(&path) {
        return SyscallAction::passthrough();
    }

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    open_virtual_path(pid, fs, path, flags, perm, fd_table)
}

fn handle_openat(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let dirfd = args.arg0 as i64;
    let path = match resolve_tracee_path_at(pid, dirfd, args.arg1) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };
    let flags = args.arg2 as i32;
    // `openat(2)` takes the mode as its fourth argument, after `dirfd`,
    // `path` and `flags`.
    let perm = args.arg3 as u32;

    if !mount_table.is_virtual(&path) {
        return SyscallAction::passthrough();
    }

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    open_virtual_path(pid, fs, path, flags, perm, fd_table)
}

fn handle_creat(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let path = match resolve_tracee_path(pid, args.arg0) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };
    let perm = args.arg1 as u32;

    if !mount_table.is_virtual(&path) {
        return SyscallAction::passthrough();
    }

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    // `creat(2)` is `open(2)` with these flags: it truncates an existing file.
    open_virtual_path(
        pid,
        fs,
        path,
        libc::O_CREAT | libc::O_WRONLY | libc::O_TRUNC,
        perm,
        fd_table,
    )
}

// ── Virtual FD handlers ──

fn handle_vfs_read(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let fd = args.arg0;
    let buf_addr = args.arg1;
    let count = args.arg2 as usize;

    let entry = match fd_table.get(pid.as_raw() as u32, fd) {
        Some(e) => e,
        None => return SyscallAction::skip_error(libc::EBADF as i64),
    };

    match entry {
        VirtualFdEntry::File { handle, offset, .. } => {
            let mut buf = vec![0u8; count];
            let off = *offset.lock().unwrap();
            match handle.read_at(&mut buf, off) {
                Ok(n) => {
                    *offset.lock().unwrap() += n as u64;
                    match memory::write_tracee_buf(pid, buf_addr, &buf[..n]) {
                        Ok(()) => SyscallAction::skip(n as i64),
                        Err(_) => SyscallAction::skip_error(libc::EFAULT as i64),
                    }
                }
                Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
            }
        }
        _ => SyscallAction::skip_error(libc::EBADF as i64),
    }
}

fn handle_vfs_write(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let fd = args.arg0;
    let buf_addr = args.arg1;
    let count = args.arg2 as usize;

    let entry = match fd_table.get(pid.as_raw() as u32, fd) {
        Some(e) => e,
        None => return SyscallAction::skip_error(libc::EBADF as i64),
    };

    match entry {
        VirtualFdEntry::File {
            handle,
            offset,
            flags,
            ..
        } => {
            let buf = match memory::read_tracee_buf(pid, buf_addr, count) {
                Ok(b) => b,
                Err(_) => return SyscallAction::skip_error(libc::EFAULT as i64),
            };
            // `O_APPEND` writes at the end of the file and moves the file
            // offset there, as `write(2)` does.
            let write_offset = if flags & libc::O_APPEND != 0 {
                match handle.size() {
                    Ok(size) => size,
                    Err(e) => return SyscallAction::skip_error(vfs_error_to_errno(&e)),
                }
            } else {
                *offset.lock().unwrap()
            };
            match handle.write_at(&buf, write_offset) {
                Ok(n) => {
                    *offset.lock().unwrap() = write_offset + n as u64;
                    SyscallAction::skip(n as i64)
                }
                Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
            }
        }
        _ => SyscallAction::skip_error(libc::EBADF as i64),
    }
}

fn handle_vfs_pread(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let fd = args.arg0;
    let buf_addr = args.arg1;
    let count = args.arg2 as usize;
    let offset = args.arg3;

    let entry = match fd_table.get(pid.as_raw() as u32, fd) {
        Some(e) => e,
        None => return SyscallAction::skip_error(libc::EBADF as i64),
    };

    match entry {
        VirtualFdEntry::File { handle, .. } => {
            let mut buf = vec![0u8; count];
            match handle.read_at(&mut buf, offset) {
                Ok(n) => match memory::write_tracee_buf(pid, buf_addr, &buf[..n]) {
                    Ok(()) => SyscallAction::skip(n as i64),
                    Err(_) => SyscallAction::skip_error(libc::EFAULT as i64),
                },
                Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
            }
        }
        _ => SyscallAction::skip_error(libc::EBADF as i64),
    }
}

fn handle_vfs_pwrite(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let fd = args.arg0;
    let buf_addr = args.arg1;
    let count = args.arg2 as usize;
    let offset = args.arg3;

    let entry = match fd_table.get(pid.as_raw() as u32, fd) {
        Some(e) => e,
        None => return SyscallAction::skip_error(libc::EBADF as i64),
    };

    match entry {
        VirtualFdEntry::File { handle, .. } => {
            let buf = match memory::read_tracee_buf(pid, buf_addr, count) {
                Ok(b) => b,
                Err(_) => return SyscallAction::skip_error(libc::EFAULT as i64),
            };
            match handle.write_at(&buf, offset) {
                Ok(n) => SyscallAction::skip(n as i64),
                Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
            }
        }
        _ => SyscallAction::skip_error(libc::EBADF as i64),
    }
}

fn handle_vfs_lseek(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let fd = args.arg0;
    let offset = args.arg1 as i64;
    let whence = args.arg2 as i32;

    let entry = match fd_table.get(pid.as_raw() as u32, fd) {
        Some(e) => e,
        None => return SyscallAction::skip_error(libc::EBADF as i64),
    };

    match entry {
        VirtualFdEntry::File {
            handle,
            offset: file_offset,
            ..
        } => {
            let current = *file_offset.lock().unwrap();
            let file_size = match handle.size() {
                Ok(s) => s as i64,
                Err(e) => return SyscallAction::skip_error(vfs_error_to_errno(&e)),
            };
            let new_pos = match whence {
                libc::SEEK_SET => offset,
                libc::SEEK_CUR => current as i64 + offset,
                libc::SEEK_END => file_size + offset,
                _ => return SyscallAction::skip_error(libc::EINVAL as i64),
            };
            if new_pos < 0 {
                return SyscallAction::skip_error(libc::EINVAL as i64);
            }
            *file_offset.lock().unwrap() = new_pos as u64;
            SyscallAction::skip(new_pos)
        }
        _ => SyscallAction::skip_error(libc::EBADF as i64),
    }
}

fn handle_vfs_close(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let fd = args.arg0;
    if fd_table.remove(pid.as_raw() as u32, fd).is_some() {
        SyscallAction::skip(0)
    } else {
        SyscallAction::skip_error(libc::EBADF as i64)
    }
}

fn handle_vfs_fstat(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let fd = args.arg0;
    let buf_addr = args.arg1;

    let entry = match fd_table.get(pid.as_raw() as u32, fd) {
        Some(e) => e,
        None => return SyscallAction::skip_error(libc::EBADF as i64),
    };

    match entry {
        VirtualFdEntry::File { handle, .. } => match handle.metadata() {
            Ok(meta) => match memory::write_tracee_stat(pid, buf_addr, &meta) {
                Ok(()) => SyscallAction::skip(0),
                Err(_) => SyscallAction::skip_error(libc::EFAULT as i64),
            },
            Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
        },
        VirtualFdEntry::Directory { handle, .. } => match handle.metadata() {
            Ok(meta) => match memory::write_tracee_stat(pid, buf_addr, &meta) {
                Ok(()) => SyscallAction::skip(0),
                Err(_) => SyscallAction::skip_error(libc::EFAULT as i64),
            },
            Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
        },
    }
}

fn handle_vfs_ftruncate(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let fd = args.arg0;
    let length = args.arg1;

    let entry = match fd_table.get(pid.as_raw() as u32, fd) {
        Some(e) => e,
        None => return SyscallAction::skip_error(libc::EBADF as i64),
    };

    match entry {
        VirtualFdEntry::File { handle, .. } => match handle.truncate(length) {
            Ok(()) => SyscallAction::skip(0),
            Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
        },
        _ => SyscallAction::skip_error(libc::EBADF as i64),
    }
}

fn handle_vfs_fchmod(
    _pid: nix::unistd::Pid,
    args: &SyscallArgs,
    _fd_table: &VirtualFdTable,
) -> SyscallAction {
    // fchmod on a virtual FD — we don't track the path, so we skip
    let _fd = args.arg0;
    let _mode = args.arg1;
    SyscallAction::skip(0) // best effort: don't fail
}

fn handle_vfs_getdents64(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let fd = args.arg0;
    let _buf_addr = args.arg1;
    let _count = args.arg2 as usize;

    let entry = match fd_table.get(pid.as_raw() as u32, fd) {
        Some(e) => e,
        None => return SyscallAction::skip_error(libc::EBADF as i64),
    };

    match entry {
        VirtualFdEntry::Directory { .. } => {
            // getdents64 is complex — it writes linux_dirent64 structs to tracee memory.
            // For now, we pass it through and let the kernel handle it.
            // A full implementation would call handle.list() and format dirent structs.
            SyscallAction::passthrough()
        }
        _ => SyscallAction::skip_error(libc::ENOTDIR as i64),
    }
}

// ── Path operation handlers ──

fn handle_path_stat(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
) -> SyscallAction {
    let path = match resolve_tracee_path(pid, args.arg0) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };
    let buf_addr = args.arg1;

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.stat(&path) {
        Ok(meta) => match memory::write_tracee_stat(pid, buf_addr, &meta) {
            Ok(()) => SyscallAction::skip(0),
            Err(_) => SyscallAction::skip_error(libc::EFAULT as i64),
        },
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

fn handle_path_access(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
) -> SyscallAction {
    let path = match resolve_tracee_path(pid, args.arg0) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.exists(&path) {
        Ok(true) => SyscallAction::skip(0),
        Ok(false) => SyscallAction::skip_error(libc::ENOENT as i64),
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

fn handle_path_remove(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
) -> SyscallAction {
    let path = match resolve_tracee_path(pid, args.arg0) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.remove(&path) {
        Ok(()) => SyscallAction::skip(0),
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

fn handle_path_mkdir(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
) -> SyscallAction {
    let path = match resolve_tracee_path(pid, args.arg0) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.mkdir(&path) {
        Ok(()) => SyscallAction::skip(0),
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

fn handle_path_rename(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
) -> SyscallAction {
    let from = match resolve_tracee_path(pid, args.arg0) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };
    let to = match resolve_tracee_path(pid, args.arg1) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };

    let fs = match mount_table.resolve(&from) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.rename(&from, &to) {
        Ok(()) => SyscallAction::skip(0),
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

fn handle_path_symlink(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
) -> SyscallAction {
    let target = match resolve_tracee_path(pid, args.arg0) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };
    let link = match resolve_tracee_path(pid, args.arg1) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };

    let fs = match mount_table.resolve(&link) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.symlink(&target, &link) {
        Ok(()) => SyscallAction::skip(0),
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

fn handle_path_readlink(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
) -> SyscallAction {
    let path = match resolve_tracee_path(pid, args.arg0) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };
    let buf_addr = args.arg1;
    let buf_size = args.arg2 as usize;

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.readlink(&path) {
        Ok(target) => {
            if target.len() >= buf_size {
                return SyscallAction::skip_error(libc::ENAMETOOLONG as i64);
            }
            match memory::write_tracee_buf(pid, buf_addr, target.as_bytes()) {
                Ok(()) => SyscallAction::skip(target.len() as i64),
                Err(_) => SyscallAction::skip_error(libc::EFAULT as i64),
            }
        }
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

fn handle_path_chmod(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
) -> SyscallAction {
    let path = match resolve_tracee_path(pid, args.arg0) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };
    let mode = args.arg1 as u32;

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.chmod(&path, mode) {
        Ok(()) => SyscallAction::skip(0),
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

fn handle_path_truncate(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
) -> SyscallAction {
    let path = match resolve_tracee_path(pid, args.arg0) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };
    let length = args.arg1;

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.open_file(&path, OpenMode::Write) {
        Ok(file) => match file.truncate(length) {
            Ok(()) => SyscallAction::skip(0),
            Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
        },
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

// ── *at variant handlers ──

fn handle_newfstatat(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
    _fd_table: &VirtualFdTable,
) -> SyscallAction {
    let dirfd = args.arg0 as i64;
    let path = match resolve_tracee_path_at(pid, dirfd, args.arg1) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };
    let buf_addr = args.arg2;
    // args.arg3 = flags (AT_SYMLINK_NOFOLLOW etc.)

    if !mount_table.is_virtual(&path) {
        return SyscallAction::passthrough();
    }

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.stat(&path) {
        Ok(meta) => match memory::write_tracee_stat(pid, buf_addr, &meta) {
            Ok(()) => SyscallAction::skip(0),
            Err(_) => SyscallAction::skip_error(libc::EFAULT as i64),
        },
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

fn handle_faccessat(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
    _fd_table: &VirtualFdTable,
) -> SyscallAction {
    let dirfd = args.arg0 as i64;
    let path = match resolve_tracee_path_at(pid, dirfd, args.arg1) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };

    if !mount_table.is_virtual(&path) {
        return SyscallAction::passthrough();
    }

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.exists(&path) {
        Ok(true) => SyscallAction::skip(0),
        Ok(false) => SyscallAction::skip_error(libc::ENOENT as i64),
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

fn handle_unlinkat(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
    _fd_table: &VirtualFdTable,
) -> SyscallAction {
    let dirfd = args.arg0 as i64;
    let path = match resolve_tracee_path_at(pid, dirfd, args.arg1) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };
    // args.arg2 = flags (AT_REMOVEDIR)

    if !mount_table.is_virtual(&path) {
        return SyscallAction::passthrough();
    }

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.remove(&path) {
        Ok(()) => SyscallAction::skip(0),
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

fn handle_mkdirat(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
    _fd_table: &VirtualFdTable,
) -> SyscallAction {
    let dirfd = args.arg0 as i64;
    let path = match resolve_tracee_path_at(pid, dirfd, args.arg1) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };

    if !mount_table.is_virtual(&path) {
        return SyscallAction::passthrough();
    }

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.mkdir(&path) {
        Ok(()) => SyscallAction::skip(0),
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

fn handle_renameat(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
    _fd_table: &VirtualFdTable,
) -> SyscallAction {
    let olddirfd = args.arg0 as i64;
    let newdirfd = args.arg2 as i64;
    let from = match resolve_tracee_path_at(pid, olddirfd, args.arg1) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };
    let to = match resolve_tracee_path_at(pid, newdirfd, args.arg3) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };

    if !mount_table.is_virtual(&from) && !mount_table.is_virtual(&to) {
        return SyscallAction::passthrough();
    }

    let fs = match mount_table.resolve(&from) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.rename(&from, &to) {
        Ok(()) => SyscallAction::skip(0),
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

fn handle_readlinkat(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
    _fd_table: &VirtualFdTable,
) -> SyscallAction {
    let dirfd = args.arg0 as i64;
    let path = match resolve_tracee_path_at(pid, dirfd, args.arg1) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };
    let buf_addr = args.arg2;
    let buf_size = args.arg3 as usize;

    if !mount_table.is_virtual(&path) {
        return SyscallAction::passthrough();
    }

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.readlink(&path) {
        Ok(target) => {
            if target.len() >= buf_size {
                return SyscallAction::skip_error(libc::ENAMETOOLONG as i64);
            }
            match memory::write_tracee_buf(pid, buf_addr, target.as_bytes()) {
                Ok(()) => SyscallAction::skip(target.len() as i64),
                Err(_) => SyscallAction::skip_error(libc::EFAULT as i64),
            }
        }
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

fn handle_symlinkat(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
    _fd_table: &VirtualFdTable,
) -> SyscallAction {
    let link = match resolve_tracee_path_at(pid, args.arg1 as i64, args.arg2) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };
    let target = match memory::read_tracee_string(pid, args.arg0, 4096).ok() {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };

    if !mount_table.is_virtual(&link) {
        return SyscallAction::passthrough();
    }

    let fs = match mount_table.resolve(&link) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.symlink(&target, &link) {
        Ok(()) => SyscallAction::skip(0),
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

fn handle_fchmodat(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    mount_table: &MountTable,
    _fd_table: &VirtualFdTable,
) -> SyscallAction {
    let dirfd = args.arg0 as i64;
    let path = match resolve_tracee_path_at(pid, dirfd, args.arg1) {
        Some(p) => p,
        None => return SyscallAction::passthrough(),
    };
    let mode = args.arg2 as u32;

    if !mount_table.is_virtual(&path) {
        return SyscallAction::passthrough();
    }

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.chmod(&path, mode) {
        Ok(()) => SyscallAction::skip(0),
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
}

// ── FD duplication ──

fn handle_dup(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    fd_table: &VirtualFdTable,
) -> SyscallAction {
    let pid_raw = pid.as_raw() as u32;
    let old_fd = args.arg0;
    let explicit_new_fd = args.nr == DUP2 || args.nr == DUP3;
    let new_fd = if explicit_new_fd {
        args.arg1
    } else {
        // DUP allocates a new FD
        fd_table.alloc_fd()
    };

    // The table lookup, not the fd number, decides: a previous `dup2` can move
    // a virtual entry to a low fd.
    let source = fd_table.get(pid_raw, old_fd);

    if explicit_new_fd && new_fd == old_fd {
        // `dup2(2)` with equal numbers returns the number and closes nothing.
        // `dup3(2)` rejects it. A real source passes through: the kernel then
        // reports `EBADF` for a missing source, as it does for `dup3(2)`.
        return match source {
            Some(_) if args.nr == DUP3 => SyscallAction::skip_error(libc::EINVAL as i64),
            Some(_) => SyscallAction::skip(new_fd as i64),
            None => SyscallAction::passthrough(),
        };
    }

    // `dup2(2)` and `dup3(2)` reject a new number outside `[0, RLIMIT_NOFILE)`
    // with `EBADF`. Check it before any table change: the real-source branch
    // below removes the entry at `new_fd`.
    if explicit_new_fd && !fd_number_in_range(pid, new_fd) {
        return SyscallAction::skip_error(libc::EBADF as i64);
    }

    let Some(entry) = source else {
        // The source is a real fd, so the kernel handles the call. It
        // replaces whatever `new_fd` was: drop a virtual entry there, or a
        // restored stdout would keep writing into the virtual file.
        fd_table.remove(pid_raw, new_fd);
        return SyscallAction::passthrough();
    };

    // `dup(2)` and `dup2(2)` clear `O_CLOEXEC` on the new descriptor.
    // `dup3(2)` sets it from its `flags` argument (`arg2`).
    let cloexec = args.nr == DUP3 && (args.arg2 as i32 & libc::O_CLOEXEC) != 0;
    let entry = with_cloexec(entry, cloexec);

    // A number below `FD_VIRTUAL_BASE` and not in the table may hold a real
    // kernel fd. `dup2(2)` closes whatever the new number held before it
    // returns, so the loop runs `close(new_fd)` in place of the intercepted
    // call and reports `new_fd`. Numbers at or above `FD_VIRTUAL_BASE` are
    // virtual by the table's convention, and the kernel holds no fd there.
    let replaces_kernel_fd = explicit_new_fd && new_fd < FD_VIRTUAL_BASE;

    fd_table.remove(pid_raw, new_fd);
    fd_table.insert(pid_raw, new_fd, entry);

    if replaces_kernel_fd {
        SyscallAction::close_then_skip(new_fd, new_fd as i64)
    } else {
        SyscallAction::skip(new_fd as i64)
    }
}

/// Copy `entry` with `O_CLOEXEC` set or cleared on its open flags.
///
/// A `Directory` entry holds no open flags, so it cannot record `O_CLOEXEC`;
/// it keeps its current behaviour across `exec`.
fn with_cloexec(mut entry: VirtualFdEntry, cloexec: bool) -> VirtualFdEntry {
    if let VirtualFdEntry::File { flags, .. } = &mut entry {
        if cloexec {
            *flags |= libc::O_CLOEXEC;
        } else {
            *flags &= !libc::O_CLOEXEC;
        }
    }
    entry
}

/// Whether `fd` is in `[0, RLIMIT_NOFILE)` for the tracee.
///
/// A negative value (the register holds a sign-extended 64-bit number) and a
/// value at or above the limit are outside the range. A limit that cannot be
/// read rejects the number: the tracee is gone, so the kernel fails the call
/// too.
fn fd_number_in_range(pid: nix::unistd::Pid, fd: u64) -> bool {
    if (fd as i64) < 0 {
        return false;
    }
    match tracee_nofile_limit(pid) {
        Some(limit) => fd < limit,
        None => false,
    }
}

/// The tracee's soft `RLIMIT_NOFILE`, read from `/proc/<pid>/limits`.
///
/// Read the tracee's own limit, not the tracer's: `setrlimit(2)` lets a tracee
/// lower it, and the kernel checks `dup2(2)` against the tracee's value. The
/// line reads `Max open files  <soft>  <hard>  files`. `None` means the file
/// or the line is missing; `unlimited` maps to `u64::MAX`.
fn tracee_nofile_limit(pid: nix::unistd::Pid) -> Option<u64> {
    let limits = std::fs::read_to_string(format!("/proc/{}/limits", pid.as_raw())).ok()?;
    let line = limits
        .lines()
        .find(|line| line.starts_with("Max open files"))?;
    let soft = line
        .strip_prefix("Max open files")?
        .split_whitespace()
        .next()?;
    if soft == "unlimited" {
        return Some(u64::MAX);
    }
    soft.parse().ok()
}

// ── Helpers ──

fn flags_to_open_mode(flags: i32) -> OpenMode {
    let acc_mode = flags & libc::O_ACCMODE;
    if acc_mode == libc::O_RDONLY {
        OpenMode::Read
    } else if acc_mode == libc::O_WRONLY {
        OpenMode::Write
    } else {
        OpenMode::ReadWrite
    }
}

/// Whether the error is a missing path. `open(2)` with `O_CREAT` creates the
/// file in that one case.
fn is_not_found(e: &ErrorTrace<VfsError>) -> bool {
    matches!(e.current_context(), VfsError::NotFound { .. })
}

fn vfs_error_to_errno(e: &ErrorTrace<VfsError>) -> i64 {
    match e.current_context() {
        VfsError::NotFound { .. } => libc::ENOENT as i64,
        VfsError::AlreadyExists { .. } => libc::EEXIST as i64,
        VfsError::PermissionDenied { .. } => libc::EACCES as i64,
        VfsError::NotAFile { .. } => libc::EISDIR as i64,
        VfsError::NotADirectory { .. } => libc::ENOTDIR as i64,
        VfsError::Unsupported { .. } => libc::ENOSYS as i64,
        VfsError::InvalidPath { .. } => libc::EINVAL as i64,
        VfsError::ReadOnly => libc::EROFS as i64,
        VfsError::EntryPending { .. } => libc::EAGAIN as i64,
        VfsError::SymlinkLoop { .. } => libc::ELOOP as i64,
        VfsError::DirectoryNotEmpty { .. } => libc::ENOTEMPTY as i64,
        VfsError::NotASymlink { .. } => libc::EINVAL as i64,
        _ => libc::EIO as i64,
    }
}
