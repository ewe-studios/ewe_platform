/// Type-erased VFS types — shared across shim, ptrace, and mount table.
///
/// `DynFs` wraps any `VfsFileSystem` implementation behind a trait object,
/// enabling heterogeneous mount tables and runtime delta store selection.
/// `ErasedFile`/`ErasedSeekableFile`/`ErasedDir` provide the same for
/// individual file/directory handles.

use std::sync::{Arc, Mutex};

use crate::shared::vfs::error::VfsResult;
use crate::shared::vfs::traits::{SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem};
use crate::shared::vfs::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsMetadata};

// ── Minimal blanket impls for Box<dyn ...> ──

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
pub trait ErasedDirOps: Send + Sync {
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

// ── DynFs: type-erased filesystem ──

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
    fn copy(&self, from: &str, to: &str) -> VfsResult<()>;
    fn remove_all(&self, path: &str) -> VfsResult<()>;
    fn mkdir_all(&self, path: &str) -> VfsResult<()>;
    fn inode(&self, path: &str) -> VfsResult<u64>;
    fn path_by_inode(&self, ino: u64) -> VfsResult<String>;
    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata>;
}

/// Type-erased wrapper for any `VfsFileSystem`.
pub struct DynFs {
    inner: Arc<dyn DynFsOps>,
}

impl Clone for DynFs {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
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
    pub fn copy(&self, from: &str, to: &str) -> VfsResult<()> { self.inner.copy(from, to) }
    /// Recursive delete — the erased counterpart of
    /// [`VfsFileSystem::remove_all`], which `remove` alone cannot express.
    pub fn remove_all(&self, path: &str) -> VfsResult<()> { self.inner.remove_all(path) }
    /// Recursive create — the erased counterpart of
    /// [`VfsFileSystem::mkdir_all`], which `mkdir` alone cannot express.
    pub fn mkdir_all(&self, path: &str) -> VfsResult<()> { self.inner.mkdir_all(path) }
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
    fn copy(&self, from: &str, to: &str) -> VfsResult<()> { self.0.copy(from, to) }
    fn remove_all(&self, path: &str) -> VfsResult<()> { self.0.remove_all(path) }
    fn mkdir_all(&self, path: &str) -> VfsResult<()> { self.0.mkdir_all(path) }
    fn inode(&self, path: &str) -> VfsResult<u64> { self.0.inode(path) }
    fn path_by_inode(&self, ino: u64) -> VfsResult<String> { self.0.path_by_inode(ino) }
    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata> { self.0.stat_by_inode(ino) }
}
