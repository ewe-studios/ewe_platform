/// Ptrace/Reverie Interceptor — dual-backend syscall interception for transparent VFS sandboxing.
///
/// Intercepts filesystem syscalls at the kernel boundary via `ptrace(2)` and routes them
/// through the `VfsFileSystem`. Non-filesystem syscalls pass through unmodified.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use nix::libc;

use foundation_errstacks::ErrorTrace;

// Re-export erased types from shared — ptrace code uses these via `super::ErasedFile` etc.
pub use crate::shared::vfs::dynfs::{DynFs, ErasedDir, ErasedDirOps, ErasedFile, ErasedSeekableFile};

use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::traits::{SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem};
use crate::shared::vfs::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsMetadata};

pub mod memory;
pub mod nix_backend;
pub mod platform;
pub mod syscall_dispatch;

// ── Syscall numbers (x86_64 Linux) ──

pub mod syscall_nr {
    pub const READ: i64 = 0;
    pub const WRITE: i64 = 1;
    pub const OPEN: i64 = 2;
    pub const CLOSE: i64 = 3;
    pub const STAT: i64 = 4;
    pub const FSTAT: i64 = 5;
    pub const LSTAT: i64 = 6;
    pub const LSEEK: i64 = 8;
    pub const PREAD64: i64 = 17;
    pub const PWRITE64: i64 = 18;
    pub const ACCESS: i64 = 21;
    pub const DUP: i64 = 32;
    pub const DUP2: i64 = 33;
    pub const FORK: i64 = 57;
    pub const VFORK: i64 = 58;
    pub const EXECVE: i64 = 59;
    pub const CHMOD: i64 = 90;
    pub const FCHMOD: i64 = 91;
    pub const TRUNCATE: i64 = 76;
    pub const FTRUNCATE: i64 = 77;
    pub const RENAME: i64 = 82;
    pub const MKDIR: i64 = 83;
    pub const RMDIR: i64 = 84;
    pub const CREAT: i64 = 85;
    pub const UNLINK: i64 = 87;
    pub const SYMLINK: i64 = 88;
    pub const READLINK: i64 = 89;
    pub const GETDENTS64: i64 = 217;
    pub const OPENAT: i64 = 257;
    pub const MKDIRAT: i64 = 258;
    pub const FCHMODAT: i64 = 268;
    pub const FACCESSAT: i64 = 269;
    pub const NEWFSTATAT: i64 = 262;
    pub const UNLINKAT: i64 = 263;
    pub const RENAMEAT: i64 = 264;
    pub const RENAMEAT2: i64 = 316;
    pub const READLINKAT: i64 = 267;
    pub const SYMLINKAT: i64 = 266;
    pub const DUP3: i64 = 292;
    pub const OPENAT2: i64 = 437;
    pub const FACCESSAT2: i64 = 439;
    pub const STATX: i64 = 332;
}

// Re-export DynFs from shared — it's also available via crate::shared::vfs::dynfs
// Blanket impls for Box<dyn ...> are defined in shared/vfs/dynfs.rs

// ── SyscallInterceptor trait ──

pub trait SyscallInterceptor: Send + Sync {
    fn spawn(
        &self,
        mount_table: MountTable,
        command: &str,
        args: &[&str],
    ) -> VfsResult<InterceptorHandle>;
}

/// Handle to a traced child process.
pub struct InterceptorHandle {
    pub child_pid: u32,
    join_handle: std::thread::JoinHandle<VfsResult<i32>>,
}

impl InterceptorHandle {
    pub fn new(child_pid: u32, join_handle: std::thread::JoinHandle<VfsResult<i32>>) -> Self {
        Self { child_pid, join_handle }
    }

    pub fn wait(self) -> VfsResult<i32> {
        self.join_handle.join().unwrap_or_else(|_| {
            Err(ErrorTrace::new(VfsError::Backend {
                message: "interceptor thread panicked".into(),
            }))
        })
    }

    pub fn pid(&self) -> u32 { self.child_pid }
}

// ── MountTable ──

#[derive(Default)]
pub struct MountTable {
    entries: Vec<(String, DynFs)>,
    default_fs: Option<DynFs>,
}

impl Clone for MountTable {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.iter().map(|(s, f)| (s.clone(), f.clone())).collect(),
            default_fs: self.default_fs.clone(),
        }
    }
}

impl MountTable {
    pub fn new() -> Self { Self::default() }
    pub fn mount(&mut self, prefix: &str, fs: DynFs) {
        self.entries.push((normalize_prefix(prefix), fs));
    }
    pub fn set_default(&mut self, fs: DynFs) { self.default_fs = Some(fs); }
    pub fn resolve(&self, path: &str) -> Option<&DynFs> {
        for (prefix, fs) in &self.entries {
            if path.starts_with(prefix.as_str()) { return Some(fs); }
        }
        self.default_fs.as_ref()
    }
    pub fn is_virtual(&self, path: &str) -> bool {
        self.entries.iter().any(|(p, _)| path.starts_with(p.as_str()))
    }
}

fn normalize_prefix(prefix: &str) -> String {
    let mut p = prefix.to_string();
    if !p.starts_with('/') { p.insert(0, '/'); }
    if p.len() > 1 { while p.ends_with('/') { p.pop(); } }
    p
}

// ── Virtual FD Table ──

pub const FD_VIRTUAL_BASE: u64 = 10_000;

pub enum VirtualFdEntry {
    File {
        handle: ErasedFile,
        path: String,
        offset: Arc<Mutex<u64>>,
        mode: OpenMode,
        flags: i32,
    },
    Directory {
        handle: ErasedDir,
        path: String,
        dir_offset: Arc<Mutex<u64>>,
    },
}

impl Clone for VirtualFdEntry {
    fn clone(&self) -> Self {
        match self {
            VirtualFdEntry::File { handle, path, offset, mode, flags } => {
                VirtualFdEntry::File {
                    handle: ErasedFile(Arc::clone(&handle.0)),
                    path: path.clone(), offset: Arc::clone(offset), mode: *mode,
                    flags: *flags,
                }
            }
            VirtualFdEntry::Directory { handle, path, dir_offset } => {
                VirtualFdEntry::Directory {
                    handle: ErasedDir(Arc::clone(&handle.0)),
                    path: path.clone(), dir_offset: Arc::clone(dir_offset),
                }
            }
        }
    }
}

pub struct VirtualFdTable {
    next_fd: AtomicU64,
    entries: Mutex<HashMap<(u32, u64), VirtualFdEntry>>,
}

impl VirtualFdTable {
    pub fn new() -> Self {
        Self {
            next_fd: AtomicU64::new(FD_VIRTUAL_BASE),
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub fn alloc_fd(&self) -> u64 { self.next_fd.fetch_add(1, Ordering::Relaxed) }

    pub fn insert(&self, pid: u32, fd: u64, entry: VirtualFdEntry) {
        self.entries.lock().unwrap().insert((pid, fd), entry);
    }

    pub fn get(&self, pid: u32, fd: u64) -> Option<VirtualFdEntry> {
        self.entries.lock().unwrap().get(&(pid, fd)).cloned()
    }

    pub fn remove(&self, pid: u32, fd: u64) -> Option<VirtualFdEntry> {
        self.entries.lock().unwrap().remove(&(pid, fd))
    }

    pub fn is_virtual_fd(fd: u64) -> bool { fd >= FD_VIRTUAL_BASE }

    /// Clone FD entries from parent to child (for fork).
    pub fn clone_for_child(&self, parent_pid: u32, child_pid: u32) {
        let mut entries = self.entries.lock().unwrap();
        let clones: Vec<_> = entries
            .iter()
            .filter(|((pid, _), _)| *pid == parent_pid)
            .map(|((_, fd), entry)| ((child_pid, *fd), entry.clone()))
            .collect();
        for (key, entry) in clones { entries.insert(key, entry); }
    }

    /// Close all virtual FDs for a process (on exit).
    pub fn close_all(&self, pid: u32) {
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|(p, _), _| *p != pid);
    }

    /// Close virtual FDs with O_CLOEXEC set (on exec).
    pub fn close_cloexec(&self, pid: u32) {
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|(p, _), entry| {
            if *p != pid { return true; }
            match entry {
                VirtualFdEntry::File { flags, .. } => *flags & libc::O_CLOEXEC == 0,
                VirtualFdEntry::Directory { .. } => true,
            }
        });
    }
}
