/// Ptrace/Reverie Interceptor — dual-backend syscall interception for transparent VFS sandboxing.
///
/// Intercepts filesystem syscalls at the kernel boundary via `ptrace(2)` and routes them
/// through the `VfsFileSystem`. Non-filesystem syscalls pass through unmodified.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use nix::libc;

use foundation_errstacks::ErrorTrace;

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

// ── Minimal blanket impls for Box<dyn ...> ──
// These are needed only within this module to handle type-erased return values
// from VfsDirectory methods (create_dir, open_directory return Box<dyn VfsDirectory>).

impl<T: VfsFile + ?Sized> VfsFile for Box<T> {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> { (**self).read_at(buf, offset) }
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> { (**self).write_at(buf, offset) }
    fn sync_data(&self) -> VfsResult<()> { (**self).sync_data() }
    fn size(&self) -> VfsResult<u64> { (**self).size() }
    fn truncate(&self, size: u64) -> VfsResult<()> { (**self).truncate(size) }
    fn metadata(&self) -> VfsResult<VfsMetadata> { (**self).metadata() }
}

impl<T: SeekableVfsFile + ?Sized> SeekableVfsFile for Box<T> {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> { (**self).read(buf) }
    fn write(&mut self, buf: &[u8]) -> VfsResult<usize> { (**self).write(buf) }
    fn seek(&mut self, pos: std::io::SeekFrom) -> VfsResult<u64> { (**self).seek(pos) }
    fn position(&self) -> u64 { (**self).position() }
}

impl<T: VfsDirectory + ?Sized> VfsDirectory for Box<T> {
    type File = T::File;
    type SeekableFile = T::SeekableFile;
    fn path(&self) -> &str { (**self).path() }
    fn metadata(&self) -> VfsResult<VfsMetadata> { (**self).metadata() }
    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> { (**self).list() }
    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>> { (**self).get_entry(name) }
    fn create_file(&self, name: &str, mode: u32) -> VfsResult<Self::File> { (**self).create_file(name, mode) }
    fn create_dir(&self, name: &str) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> { (**self).create_dir(name) }
    fn remove_entry(&self, name: &str) -> VfsResult<()> { (**self).remove_entry(name) }
    fn rename_entry(&self, old_name: &str, new_name: &str) -> VfsResult<()> { (**self).rename_entry(old_name, new_name) }
    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> { (**self).open(path, mode) }
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> { (**self).open_seekable(path, mode) }
    fn open_directory(&self, path: &str) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> { (**self).open_directory(path) }
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> { (**self).stat(path) }
    fn exists(&self, path: &str) -> VfsResult<bool> { (**self).exists(path) }
}

// ── Erased types ──

/// Type-erased file handle — shareable across fork via `Arc<Mutex<>>`.
pub struct ErasedFile(pub Arc<Mutex<Box<dyn VfsFile + Send + Sync>>>);

impl ErasedFile {
    pub fn new(f: impl VfsFile + Send + Sync + 'static) -> Self {
        Self(Arc::new(Mutex::new(Box::new(f))))
    }
    pub fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        self.0.lock().unwrap().read_at(buf, offset)
    }
    pub fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        self.0.lock().unwrap().write_at(buf, offset)
    }
    pub fn sync_data(&self) -> VfsResult<()> { self.0.lock().unwrap().sync_data() }
    pub fn size(&self) -> VfsResult<u64> { self.0.lock().unwrap().size() }
    pub fn truncate(&self, size: u64) -> VfsResult<()> { self.0.lock().unwrap().truncate(size) }
    pub fn metadata(&self) -> VfsResult<VfsMetadata> { self.0.lock().unwrap().metadata() }
}

/// Type-erased seekable file handle.
pub struct ErasedSeekableFile(pub Arc<Mutex<Box<dyn SeekableVfsFile + Send + Sync>>>);

impl ErasedSeekableFile {
    pub fn new(f: impl SeekableVfsFile + Send + Sync + 'static) -> Self {
        Self(Arc::new(Mutex::new(Box::new(f))))
    }
    pub fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        self.0.lock().unwrap().read_at(buf, offset)
    }
    pub fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        self.0.lock().unwrap().write_at(buf, offset)
    }
    pub fn sync_data(&self) -> VfsResult<()> { self.0.lock().unwrap().sync_data() }
    pub fn size(&self) -> VfsResult<u64> { self.0.lock().unwrap().size() }
    pub fn truncate(&self, size: u64) -> VfsResult<()> { self.0.lock().unwrap().truncate(size) }
    pub fn metadata(&self) -> VfsResult<VfsMetadata> { self.0.lock().unwrap().metadata() }
    pub fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        self.0.lock().unwrap().read(buf)
    }
    pub fn write(&mut self, buf: &[u8]) -> VfsResult<usize> {
        self.0.lock().unwrap().write(buf)
    }
    pub fn seek(&mut self, pos: std::io::SeekFrom) -> VfsResult<u64> {
        self.0.lock().unwrap().seek(pos)
    }
    pub fn position(&self) -> u64 { self.0.lock().unwrap().position() }
}

/// Type-erased directory handle.
pub struct ErasedDir(pub Arc<Mutex<Box<dyn ErasedDirOps>>>);

/// Internal trait — concrete directories are adapted to this.
trait ErasedDirOps: Send + Sync {
    fn path(&self) -> String;
    fn metadata(&self) -> VfsResult<VfsMetadata>;
    fn list(&self) -> VfsResult<Vec<VfsDirEntry>>;
    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>>;
    fn create_file(&self, name: &str, mode: u32) -> VfsResult<ErasedFile>;
    fn create_dir(&self, name: &str) -> VfsResult<ErasedDir>;
    fn remove_entry(&self, name: &str) -> VfsResult<()>;
    fn rename_entry(&self, old_name: &str, new_name: &str) -> VfsResult<()>;
    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<ErasedFile>;
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<ErasedSeekableFile>;
    fn open_directory(&self, path: &str) -> VfsResult<ErasedDir>;
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata>;
    fn exists(&self, path: &str) -> VfsResult<bool>;
}

/// Adapter: wraps any concrete `VfsDirectory` into `ErasedDirOps`.
struct DirOpsAdapter<D>(D);

impl<D: VfsDirectory + Send + Sync + 'static> ErasedDirOps for DirOpsAdapter<D>
where
    D::File: Send + Sync + 'static,
    D::SeekableFile: Send + Sync + 'static,
{
    fn path(&self) -> String { self.0.path().to_string() }
    fn metadata(&self) -> VfsResult<VfsMetadata> { self.0.metadata() }
    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> { self.0.list() }
    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>> { self.0.get_entry(name) }
    fn create_file(&self, name: &str, mode: u32) -> VfsResult<ErasedFile> {
        self.0.create_file(name, mode).map(ErasedFile::new)
    }
    fn create_dir(&self, name: &str) -> VfsResult<ErasedDir> {
        self.0.create_dir(name).map(|d| ErasedDir(Arc::new(Mutex::new(Box::new(DirOpsAdapter(d))))))
    }
    fn remove_entry(&self, name: &str) -> VfsResult<()> { self.0.remove_entry(name) }
    fn rename_entry(&self, old_name: &str, new_name: &str) -> VfsResult<()> {
        self.0.rename_entry(old_name, new_name)
    }
    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<ErasedFile> {
        self.0.open(path, mode).map(ErasedFile::new)
    }
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<ErasedSeekableFile> {
        self.0.open_seekable(path, mode).map(ErasedSeekableFile::new)
    }
    fn open_directory(&self, path: &str) -> VfsResult<ErasedDir> {
        self.0.open_directory(path).map(|d| ErasedDir(Arc::new(Mutex::new(Box::new(DirOpsAdapter(d))))))
    }
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> { self.0.stat(path) }
    fn exists(&self, path: &str) -> VfsResult<bool> { self.0.exists(path) }
}

impl ErasedDir {
    pub fn new<D: VfsDirectory + Send + Sync + 'static>(d: D) -> Self
    where
        D::File: Send + Sync + 'static,
        D::SeekableFile: Send + Sync + 'static,
    {
        Self(Arc::new(Mutex::new(Box::new(DirOpsAdapter(d)))))
    }

    pub fn path(&self) -> String { self.0.lock().unwrap().path() }
    pub fn metadata(&self) -> VfsResult<VfsMetadata> { self.0.lock().unwrap().metadata() }
    pub fn list(&self) -> VfsResult<Vec<VfsDirEntry>> { self.0.lock().unwrap().list() }
    pub fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>> {
        self.0.lock().unwrap().get_entry(name)
    }
    pub fn create_file(&self, name: &str, mode: u32) -> VfsResult<ErasedFile> {
        self.0.lock().unwrap().create_file(name, mode)
    }
    pub fn create_dir(&self, name: &str) -> VfsResult<ErasedDir> {
        self.0.lock().unwrap().create_dir(name)
    }
    pub fn remove_entry(&self, name: &str) -> VfsResult<()> {
        self.0.lock().unwrap().remove_entry(name)
    }
    pub fn rename_entry(&self, old_name: &str, new_name: &str) -> VfsResult<()> {
        self.0.lock().unwrap().rename_entry(old_name, new_name)
    }
    pub fn open(&self, path: &str, mode: OpenMode) -> VfsResult<ErasedFile> {
        self.0.lock().unwrap().open(path, mode)
    }
    pub fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<ErasedSeekableFile> {
        self.0.lock().unwrap().open_seekable(path, mode)
    }
    pub fn open_directory(&self, path: &str) -> VfsResult<ErasedDir> {
        self.0.lock().unwrap().open_directory(path)
    }
    pub fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        self.0.lock().unwrap().stat(path)
    }
    pub fn exists(&self, path: &str) -> VfsResult<bool> {
        self.0.lock().unwrap().exists(path)
    }
}

// ── DynFs: type-erased filesystem for mount table ──

/// Internal trait for type erasure.
trait DynFsOps: Send + Sync {
    fn capabilities(&self) -> VfsCapabilities;
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata>;
    fn exists(&self, path: &str) -> VfsResult<bool>;
    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()>;
    fn symlink(&self, target: &str, link: &str) -> VfsResult<()>;
    fn readlink(&self, path: &str) -> VfsResult<String>;
    fn rename(&self, from: &str, to: &str) -> VfsResult<()>;
    fn remove(&self, path: &str) -> VfsResult<()>;
    fn open_file(&self, path: &str, mode: OpenMode) -> VfsResult<ErasedFile>;
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<ErasedSeekableFile>;
    fn open_dir(&self, path: &str) -> VfsResult<ErasedDir>;
    fn create(&self, path: &str, mode: u32) -> VfsResult<ErasedFile>;
    fn mkdir(&self, path: &str) -> VfsResult<()>;
    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>>;
    fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()>;
    fn inode(&self, path: &str) -> VfsResult<u64>;
    fn path_by_inode(&self, ino: u64) -> VfsResult<String>;
    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata>;
}

/// Type-erased wrapper for any `VfsFileSystem`.
pub struct DynFs {
    inner: Arc<dyn DynFsOps>,
}

impl DynFs {
    pub fn new<F: VfsFileSystem + Send + Sync + 'static>(fs: Arc<F>) -> Self
    where
        F::File: Send + Sync + 'static,
        F::SeekableFile: Send + Sync + 'static,
        F::Directory: Send + Sync + 'static,
    {
        Self { inner: Arc::new(FsOpsAdapter(fs)) }
    }

    pub fn capabilities(&self) -> VfsCapabilities { self.inner.capabilities() }
    pub fn stat(&self, path: &str) -> VfsResult<VfsMetadata> { self.inner.stat(path) }
    pub fn exists(&self, path: &str) -> VfsResult<bool> { self.inner.exists(path) }
    pub fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> { self.inner.chmod(path, mode) }
    pub fn symlink(&self, target: &str, link: &str) -> VfsResult<()> { self.inner.symlink(target, link) }
    pub fn readlink(&self, path: &str) -> VfsResult<String> { self.inner.readlink(path) }
    pub fn rename(&self, from: &str, to: &str) -> VfsResult<()> { self.inner.rename(from, to) }
    pub fn remove(&self, path: &str) -> VfsResult<()> { self.inner.remove(path) }
    pub fn open_file(&self, path: &str, mode: OpenMode) -> VfsResult<ErasedFile> {
        self.inner.open_file(path, mode)
    }
    pub fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<ErasedSeekableFile> {
        self.inner.open_seekable(path, mode)
    }
    pub fn open_dir(&self, path: &str) -> VfsResult<ErasedDir> { self.inner.open_dir(path) }
    pub fn create(&self, path: &str, mode: u32) -> VfsResult<ErasedFile> {
        self.inner.create(path, mode)
    }
    pub fn mkdir(&self, path: &str) -> VfsResult<()> { self.inner.mkdir(path) }
    pub fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> { self.inner.read_file(path) }
    pub fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()> { self.inner.write_file(path, data) }
    pub fn inode(&self, path: &str) -> VfsResult<u64> { self.inner.inode(path) }
    pub fn path_by_inode(&self, ino: u64) -> VfsResult<String> { self.inner.path_by_inode(ino) }
    pub fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata> { self.inner.stat_by_inode(ino) }
}

struct FsOpsAdapter<F: VfsFileSystem>(Arc<F>);

impl<F: VfsFileSystem + Send + Sync + 'static> DynFsOps for FsOpsAdapter<F>
where
    F::File: Send + Sync + 'static,
    F::SeekableFile: Send + Sync + 'static,
    F::Directory: Send + Sync + 'static,
{
    fn capabilities(&self) -> VfsCapabilities { self.0.capabilities() }
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> { self.0.stat(path) }
    fn exists(&self, path: &str) -> VfsResult<bool> { self.0.exists(path) }
    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> { self.0.chmod(path, mode) }
    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> { self.0.symlink(target, link) }
    fn readlink(&self, path: &str) -> VfsResult<String> { self.0.readlink(path) }
    fn rename(&self, from: &str, to: &str) -> VfsResult<()> { self.0.rename(from, to) }
    fn remove(&self, path: &str) -> VfsResult<()> { self.0.remove(path) }
    fn open_file(&self, path: &str, mode: OpenMode) -> VfsResult<ErasedFile> {
        self.0.open(path, mode).map(ErasedFile::new)
    }
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<ErasedSeekableFile> {
        self.0.open_seekable(path, mode).map(ErasedSeekableFile::new)
    }
    fn open_dir(&self, path: &str) -> VfsResult<ErasedDir> {
        self.0.open_directory(path).map(ErasedDir::new)
    }
    fn create(&self, path: &str, mode: u32) -> VfsResult<ErasedFile> {
        self.0.create(path, mode).map(ErasedFile::new)
    }
    fn mkdir(&self, path: &str) -> VfsResult<()> { self.0.mkdir(path) }
    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> { self.0.read_file(path) }
    fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()> { self.0.write_file(path, data) }
    fn inode(&self, path: &str) -> VfsResult<u64> { self.0.inode(path) }
    fn path_by_inode(&self, ino: u64) -> VfsResult<String> { self.0.path_by_inode(ino) }
    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata> { self.0.stat_by_inode(ino) }
}

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
            entries: self.entries.iter().map(|(s, f)| (s.clone(), DynFs { inner: Arc::clone(&f.inner) })).collect(),
            default_fs: self.default_fs.as_ref().map(|f| DynFs { inner: Arc::clone(&f.inner) }),
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
