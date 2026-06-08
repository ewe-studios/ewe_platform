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
use super::{DynFs, ErasedDir, ErasedFile, ErasedSeekableFile, MountTable, VirtualFdEntry, VirtualFdTable, syscall_nr::*};
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
}

impl SyscallAction {
    pub fn passthrough() -> Self { Self::Passthrough }
    pub fn skip(return_value: i64) -> Self { Self::Skip { return_value } }
    pub fn skip_error(errno: i64) -> Self { Self::SkipError { errno } }
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
        READ if VirtualFdTable::is_virtual_fd(args.arg0 as u64) => {
            handle_vfs_read(pid, args, fd_table)
        }
        WRITE if VirtualFdTable::is_virtual_fd(args.arg0 as u64) => {
            handle_vfs_write(pid, args, fd_table)
        }
        PREAD64 if VirtualFdTable::is_virtual_fd(args.arg0 as u64) => {
            handle_vfs_pread(pid, args, fd_table)
        }
        PWRITE64 if VirtualFdTable::is_virtual_fd(args.arg0 as u64) => {
            handle_vfs_pwrite(pid, args, fd_table)
        }
        LSEEK if VirtualFdTable::is_virtual_fd(args.arg0 as u64) => {
            handle_vfs_lseek(pid, args, fd_table)
        }
        CLOSE if VirtualFdTable::is_virtual_fd(args.arg0 as u64) => {
            handle_vfs_close(pid, args, fd_table)
        }
        FSTAT if VirtualFdTable::is_virtual_fd(args.arg0 as u64) => {
            handle_vfs_fstat(pid, args, fd_table)
        }
        FTRUNCATE if VirtualFdTable::is_virtual_fd(args.arg0 as u64) => {
            handle_vfs_ftruncate(pid, args, fd_table)
        }
        FCHMOD if VirtualFdTable::is_virtual_fd(args.arg0 as u64) => {
            handle_vfs_fchmod(pid, args, fd_table)
        }
        GETDENTS64 if VirtualFdTable::is_virtual_fd(args.arg0 as u64) => {
            handle_vfs_getdents64(pid, args, fd_table)
        }

        // ── Path operations (virtual path only) ──
        STAT if resolve_tracee_path(pid, args.arg0).map(|p| mount_table.is_virtual(&p)).unwrap_or(false) => {
            handle_path_stat(pid, args, mount_table)
        }
        LSTAT if {
            match memory::read_tracee_string(pid, args.arg0, 4096) {
                Ok(s) => mount_table.is_virtual(&s),
                Err(_) => false,
            }
        } => {
            handle_path_stat(pid, args, mount_table)
        }
        ACCESS if {
            match memory::read_tracee_string(pid, args.arg0, 4096) {
                Ok(s) => mount_table.is_virtual(&s),
                Err(_) => false,
            }
        } => {
            handle_path_access(pid, args, mount_table)
        }
        UNLINK if {
            match memory::read_tracee_string(pid, args.arg0, 4096) {
                Ok(s) => mount_table.is_virtual(&s),
                Err(_) => false,
            }
        } => {
            handle_path_remove(pid, args, mount_table)
        }
        MKDIR if {
            match memory::read_tracee_string(pid, args.arg0, 4096) {
                Ok(s) => mount_table.is_virtual(&s),
                Err(_) => false,
            }
        } => {
            handle_path_mkdir(pid, args, mount_table)
        }
        RMDIR if {
            match memory::read_tracee_string(pid, args.arg0, 4096) {
                Ok(s) => mount_table.is_virtual(&s),
                Err(_) => false,
            }
        } => {
            handle_path_remove(pid, args, mount_table)
        }
        RENAME if {
            match (memory::read_tracee_string(pid, args.arg0, 4096),
                   memory::read_tracee_string(pid, args.arg1, 4096))
            {
                (Ok(from), Ok(to)) => mount_table.is_virtual(&from) || mount_table.is_virtual(&to),
                _ => false,
            }
        } => {
            handle_path_rename(pid, args, mount_table)
        }
        SYMLINK if {
            match memory::read_tracee_string(pid, args.arg1, 4096) {
                Ok(s) => mount_table.is_virtual(&s),
                Err(_) => false,
            }
        } => {
            handle_path_symlink(pid, args, mount_table)
        }
        READLINK if {
            match memory::read_tracee_string(pid, args.arg0, 4096) {
                Ok(s) => mount_table.is_virtual(&s),
                Err(_) => false,
            }
        } => {
            handle_path_readlink(pid, args, mount_table)
        }
        CHMOD if {
            match memory::read_tracee_string(pid, args.arg0, 4096) {
                Ok(s) => mount_table.is_virtual(&s),
                Err(_) => false,
            }
        } => {
            handle_path_chmod(pid, args, mount_table)
        }
        TRUNCATE if {
            match memory::read_tracee_string(pid, args.arg0, 4096) {
                Ok(s) => mount_table.is_virtual(&s),
                Err(_) => false,
            }
        } => {
            handle_path_truncate(pid, args, mount_table)
        }

        // ── *at variants ──
        OPENAT => handle_openat(pid, args, mount_table, fd_table),
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

    if !mount_table.is_virtual(&path) {
        return SyscallAction::passthrough();
    }

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    let mode = flags_to_open_mode(flags);

    if flags & libc::O_CREAT != 0 && flags & libc::O_EXCL != 0 {
        if fs.exists(&path).unwrap_or(false) {
            return SyscallAction::skip_error(libc::EEXIST as i64);
        }
    }

    let result = if flags & libc::O_CREAT != 0 {
        fs.create(&path, flags_to_perm(flags))
    } else {
        fs.open_file(&path, mode)
    };

    match result {
        Ok(file) => {
            let vfd = fd_table.alloc_fd();
            fd_table.insert(pid.as_raw() as u32, vfd, VirtualFdEntry::File {
                handle: file,
                path,
                offset: Arc::new(Mutex::new(0)),
                mode,
                flags,
            });
            SyscallAction::skip(vfd as i64)
        }
        Err(e) => {
            SyscallAction::skip_error(vfs_error_to_errno(&e))
        }
    }
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

    if !mount_table.is_virtual(&path) {
        return SyscallAction::passthrough();
    }

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    let mode = flags_to_open_mode(flags);

    if flags & libc::O_CREAT != 0 && flags & libc::O_EXCL != 0 {
        if fs.exists(&path).unwrap_or(false) {
            return SyscallAction::skip_error(libc::EEXIST as i64);
        }
    }

    let result = if flags & libc::O_CREAT != 0 {
        fs.create(&path, flags_to_perm(flags))
    } else {
        fs.open_file(&path, mode)
    };

    match result {
        Ok(file) => {
            let vfd = fd_table.alloc_fd();
            fd_table.insert(pid.as_raw() as u32, vfd, VirtualFdEntry::File {
                handle: file, path, offset: Arc::new(Mutex::new(0)), mode, flags,
            });
            SyscallAction::skip(vfd as i64)
        }
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
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
    let mode = args.arg1 as u32;

    if !mount_table.is_virtual(&path) {
        return SyscallAction::passthrough();
    }

    let fs = match mount_table.resolve(&path) {
        Some(f) => f,
        None => return SyscallAction::passthrough(),
    };

    match fs.create(&path, mode) {
        Ok(file) => {
            let vfd = fd_table.alloc_fd();
            fd_table.insert(pid.as_raw() as u32, vfd, VirtualFdEntry::File {
                handle: file, path, offset: Arc::new(Mutex::new(0)), mode: OpenMode::Write,
                flags: libc::O_CREAT | libc::O_WRONLY | libc::O_TRUNC,
            });
            SyscallAction::skip(vfd as i64)
        }
        Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
    }
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
        VirtualFdEntry::File { handle, offset, .. } => {
            let buf = match memory::read_tracee_buf(pid, buf_addr, count) {
                Ok(b) => b,
                Err(_) => return SyscallAction::skip_error(libc::EFAULT as i64),
            };
            let off = *offset.lock().unwrap();
            match handle.write_at(&buf, off) {
                Ok(n) => {
                    *offset.lock().unwrap() += n as u64;
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
                Ok(n) => {
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
        VirtualFdEntry::File { handle, offset: file_offset, .. } => {
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
        VirtualFdEntry::File { handle, .. } => {
            match handle.metadata() {
                Ok(meta) => {
                    match memory::write_tracee_stat(pid, buf_addr, &meta) {
                        Ok(()) => SyscallAction::skip(0),
                        Err(_) => SyscallAction::skip_error(libc::EFAULT as i64),
                    }
                }
                Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
            }
        }
        VirtualFdEntry::Directory { handle, .. } => {
            match handle.metadata() {
                Ok(meta) => {
                    match memory::write_tracee_stat(pid, buf_addr, &meta) {
                        Ok(()) => SyscallAction::skip(0),
                        Err(_) => SyscallAction::skip_error(libc::EFAULT as i64),
                    }
                }
                Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
            }
        }
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
        VirtualFdEntry::File { handle, .. } => {
            match handle.truncate(length) {
                Ok(()) => SyscallAction::skip(0),
                Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
            }
        }
        _ => SyscallAction::skip_error(libc::EBADF as i64),
    }
}

fn handle_vfs_fchmod(
    pid: nix::unistd::Pid,
    args: &SyscallArgs,
    fd_table: &VirtualFdTable,
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
        VirtualFdEntry::Directory { handle, .. } => {
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
        Ok(meta) => {
            match memory::write_tracee_stat(pid, buf_addr, &meta) {
                Ok(()) => SyscallAction::skip(0),
                Err(_) => SyscallAction::skip_error(libc::EFAULT as i64),
            }
        }
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
        Ok(file) => {
            match file.truncate(length) {
                Ok(()) => SyscallAction::skip(0),
                Err(e) => SyscallAction::skip_error(vfs_error_to_errno(&e)),
            }
        }
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
        Ok(meta) => {
            match memory::write_tracee_stat(pid, buf_addr, &meta) {
                Ok(()) => SyscallAction::skip(0),
                Err(_) => SyscallAction::skip_error(libc::EFAULT as i64),
            }
        }
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
    let old_fd = args.arg0;
    let new_fd = if args.nr == DUP2 || args.nr == DUP3 {
        args.arg1
    } else {
        // DUP allocates a new FD
        fd_table.alloc_fd()
    };

    if !VirtualFdTable::is_virtual_fd(old_fd) {
        return SyscallAction::passthrough();
    }

    match fd_table.get(pid.as_raw() as u32, old_fd) {
        Some(entry) => {
            fd_table.insert(pid.as_raw() as u32, new_fd, entry.clone());
            SyscallAction::skip(new_fd as i64)
        }
        None => SyscallAction::passthrough(),
    }
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

fn flags_to_perm(flags: i32) -> u32 {
    ((flags >> 16) & 0o777) as u32
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
