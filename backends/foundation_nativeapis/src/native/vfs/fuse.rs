use std::collections::HashMap;
use std::ffi::OsStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use fuser::{
    BackgroundSession, FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyCreate,
    ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, ReplyStatfs, ReplyWrite, Request,
    Session,
};

use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::traits::{VfsDirectory, VfsFile, VfsFileSystem};
use crate::shared::vfs::types::{OpenMode, VfsDirEntry, VfsFileType, VfsMetadata};

pub const ROOT_INO: u64 = 1;

pub fn vfs_error_to_errno(e: &VfsError) -> i32 {
    match e {
        VfsError::NotFound { .. } => libc::ENOENT,
        VfsError::AlreadyExists { .. } => libc::EEXIST,
        VfsError::PermissionDenied { .. } => libc::EACCES,
        VfsError::NotAFile { .. } => libc::EISDIR,
        VfsError::NotADirectory { .. } => libc::ENOTDIR,
        VfsError::Unsupported { .. } => libc::ENOSYS,
        VfsError::Io { .. } => libc::EIO,
        VfsError::InvalidPath { .. } => libc::EINVAL,
        VfsError::ReadOnly => libc::EROFS,
        VfsError::EntryPending { .. } => libc::EAGAIN,
        VfsError::SymlinkLoop { .. } => libc::ELOOP,
        VfsError::DirectoryNotEmpty { .. } => libc::ENOTEMPTY,
        VfsError::NotASymlink { .. } => libc::EINVAL,
        VfsError::Backend { .. } => libc::EIO,
    }
}

fn reply_err_from_vfs<E>(e: &foundation_errstacks::ErrorTrace<VfsError>, reply: E)
where
    E: FuseReplyError,
{
    let errno = vfs_error_to_errno(e.current_context());
    reply.error(errno);
}

trait FuseReplyError {
    fn error(self, errno: i32);
}

impl FuseReplyError for ReplyEntry {
    fn error(self, errno: i32) {
        self.error(errno);
    }
}

impl FuseReplyError for ReplyAttr {
    fn error(self, errno: i32) {
        self.error(errno);
    }
}

impl FuseReplyError for ReplyData {
    fn error(self, errno: i32) {
        self.error(errno);
    }
}

impl FuseReplyError for ReplyEmpty {
    fn error(self, errno: i32) {
        self.error(errno);
    }
}

impl FuseReplyError for ReplyOpen {
    fn error(self, errno: i32) {
        self.error(errno);
    }
}

impl FuseReplyError for ReplyWrite {
    fn error(self, errno: i32) {
        self.error(errno);
    }
}

impl FuseReplyError for ReplyDirectory {
    fn error(self, errno: i32) {
        self.error(errno);
    }
}

impl FuseReplyError for ReplyCreate {
    fn error(self, errno: i32) {
        self.error(errno);
    }
}

impl FuseReplyError for ReplyStatfs {
    fn error(self, errno: i32) {
        self.error(errno);
    }
}

struct OpenFileHandle {
    file: Box<dyn VfsFile>,
}

struct OpenDirHandle {
    cached_entries: Vec<VfsDirEntry>,
}

#[derive(Debug, Clone)]
pub struct FuseMountOptions {
    pub attr_timeout: Duration,
    pub entry_timeout: Duration,
    pub auto_unmount: bool,
    pub allow_other: bool,
}

impl Default for FuseMountOptions {
    fn default() -> Self {
        Self {
            attr_timeout: Duration::from_secs(1),
            entry_timeout: Duration::from_secs(1),
            auto_unmount: true,
            allow_other: false,
        }
    }
}

pub struct FuseMount<F: VfsFileSystem> {
    fs: Arc<F>,
    options: FuseMountOptions,
    file_handles: RwLock<HashMap<u64, OpenFileHandle>>,
    dir_handles: RwLock<HashMap<u64, OpenDirHandle>>,
    next_fh: AtomicU64,
}

impl<F: VfsFileSystem> std::fmt::Debug for FuseMount<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FuseMount")
            .field("options", &self.options)
            .field("next_fh", &self.next_fh.load(Ordering::Relaxed))
            .finish()
    }
}

impl<F: VfsFileSystem + 'static> FuseMount<F> {
    pub fn new(fs: F, options: FuseMountOptions) -> Self {
        Self {
            fs: Arc::new(fs),
            options,
            file_handles: RwLock::new(HashMap::new()),
            dir_handles: RwLock::new(HashMap::new()),
            next_fh: AtomicU64::new(1),
        }
    }

    pub fn mount(self, mountpoint: &str) -> VfsResult<Session<Self>> {
        let mount_options = self.build_mount_options();
        Session::new(self, std::path::Path::new(mountpoint), &mount_options).map_err(|e| {
            foundation_errstacks::ErrorTrace::new(VfsError::Io { source: e })
        })
    }

    pub fn mount_background(self, mountpoint: &str) -> VfsResult<BackgroundSession>
    where
        F: Send,
    {
        let session = self.mount(mountpoint)?;
        session.spawn().map_err(|e| {
            foundation_errstacks::ErrorTrace::new(VfsError::Io { source: e })
        })
    }

    fn build_mount_options(&self) -> Vec<MountOption> {
        let mut opts = vec![
            MountOption::FSName("vfs-fuse".to_string()),
            MountOption::DefaultPermissions,
        ];
        if self.options.auto_unmount {
            opts.push(MountOption::AutoUnmount);
        }
        if self.options.allow_other {
            opts.push(MountOption::AllowOther);
        }
        opts
    }

    pub fn alloc_fh(&self) -> u64 {
        self.next_fh.fetch_add(1, Ordering::Relaxed)
    }

    pub fn child_path(parent: &str, name: &str) -> String {
        if parent == "/" {
            format!("/{name}")
        } else {
            format!("{parent}/{name}")
        }
    }

    pub fn metadata_to_attr(&self, meta: &VfsMetadata) -> FileAttr {
        let kind = match meta.file_type {
            VfsFileType::Regular => FileType::RegularFile,
            VfsFileType::Directory => FileType::Directory,
            VfsFileType::Symlink => FileType::Symlink,
        };
        let now = SystemTime::now();
        FileAttr {
            ino: meta.inode,
            size: meta.size,
            blocks: (meta.size + 511) / 512,
            atime: meta.accessed.unwrap_or(now),
            mtime: meta.modified.unwrap_or(now),
            ctime: meta.modified.unwrap_or(now),
            crtime: meta.created.unwrap_or(now),
            kind,
            perm: meta.permissions as u16,
            nlink: if meta.file_type == VfsFileType::Directory {
                2
            } else {
                1
            },
            uid: meta.owner.0,
            gid: meta.owner.1,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        }
    }
}

impl<F: VfsFileSystem + 'static> Filesystem for FuseMount<F> {
    fn init(
        &mut self,
        _req: &Request<'_>,
        _config: &mut fuser::KernelConfig,
    ) -> Result<(), libc::c_int> {
        tracing::info!("FUSE filesystem initialized");
        Ok(())
    }

    fn destroy(&mut self) {
        tracing::info!("FUSE filesystem destroyed");
    }

    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let parent_path = match self.fs.path_by_inode(parent) {
            Ok(p) => p,
            Err(e) => {
                reply_err_from_vfs(&e, reply);
                return;
            }
        };

        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let child = Self::child_path(&parent_path, name_str);

        match self.fs.stat(&child) {
            Ok(meta) => {
                let attr = self.metadata_to_attr(&meta);
                reply.entry(&self.options.entry_timeout, &attr, 0);
            }
            Err(e) => reply_err_from_vfs(&e, reply),
        }
    }

    fn forget(&mut self, _req: &Request<'_>, _ino: u64, _nlookup: u64) {
        // VFS owns inode lifecycle — nothing to do here.
    }

    fn getattr(&mut self, _req: &Request<'_>, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        match self.fs.stat_by_inode(ino) {
            Ok(meta) => {
                let attr = self.metadata_to_attr(&meta);
                reply.attr(&self.options.attr_timeout, &attr);
            }
            Err(_) => {
                match self.fs.path_by_inode(ino) {
                    Ok(path) => match self.fs.stat(&path) {
                        Ok(meta) => {
                            let attr = self.metadata_to_attr(&meta);
                            reply.attr(&self.options.attr_timeout, &attr);
                        }
                        Err(e) => reply_err_from_vfs(&e, reply),
                    },
                    Err(e) => reply_err_from_vfs(&e, reply),
                }
            }
        }
    }

    fn setattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>,
        _mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        let path = match self.fs.path_by_inode(ino) {
            Ok(p) => p,
            Err(e) => {
                reply_err_from_vfs(&e, reply);
                return;
            }
        };

        if let Some(mode) = mode {
            if let Err(e) = self.fs.chmod(&path, mode) {
                reply_err_from_vfs(&e, reply);
                return;
            }
        }

        if let Some(new_size) = size {
            match self.fs.open(&path, OpenMode::Write) {
                Ok(file) => {
                    if let Err(e) = file.truncate(new_size) {
                        reply_err_from_vfs(&e, reply);
                        return;
                    }
                }
                Err(e) => {
                    reply_err_from_vfs(&e, reply);
                    return;
                }
            }
        }

        match self.fs.stat(&path) {
            Ok(meta) => {
                let attr = self.metadata_to_attr(&meta);
                reply.attr(&self.options.attr_timeout, &attr);
            }
            Err(e) => reply_err_from_vfs(&e, reply),
        }
    }

    fn readlink(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyData) {
        let path = match self.fs.path_by_inode(ino) {
            Ok(p) => p,
            Err(e) => {
                reply_err_from_vfs(&e, reply);
                return;
            }
        };

        match self.fs.readlink(&path) {
            Ok(target) => reply.data(target.as_bytes()),
            Err(e) => reply_err_from_vfs(&e, reply),
        }
    }

    fn symlink(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        link_name: &OsStr,
        target: &std::path::Path,
        reply: ReplyEntry,
    ) {
        let parent_path = match self.fs.path_by_inode(parent) {
            Ok(p) => p,
            Err(e) => {
                reply_err_from_vfs(&e, reply);
                return;
            }
        };

        let link_name_str = match link_name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let target_str = match target.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let link_path = Self::child_path(&parent_path, link_name_str);

        if let Err(e) = self.fs.symlink(target_str, &link_path) {
            reply_err_from_vfs(&e, reply);
            return;
        }

        match self.fs.stat(&link_path) {
            Ok(meta) => {
                let attr = self.metadata_to_attr(&meta);
                reply.entry(&self.options.entry_timeout, &attr, 0);
            }
            Err(e) => reply_err_from_vfs(&e, reply),
        }
    }

    fn mkdir(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let parent_path = match self.fs.path_by_inode(parent) {
            Ok(p) => p,
            Err(e) => {
                reply_err_from_vfs(&e, reply);
                return;
            }
        };

        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let child = Self::child_path(&parent_path, name_str);

        if let Err(e) = self.fs.mkdir(&child) {
            reply_err_from_vfs(&e, reply);
            return;
        }

        match self.fs.stat(&child) {
            Ok(meta) => {
                let attr = self.metadata_to_attr(&meta);
                reply.entry(&self.options.entry_timeout, &attr, 0);
            }
            Err(e) => reply_err_from_vfs(&e, reply),
        }
    }

    fn unlink(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let parent_path = match self.fs.path_by_inode(parent) {
            Ok(p) => p,
            Err(e) => {
                reply_err_from_vfs(&e, reply);
                return;
            }
        };

        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let child = Self::child_path(&parent_path, name_str);

        match self.fs.remove(&child) {
            Ok(()) => reply.ok(),
            Err(e) => reply_err_from_vfs(&e, reply),
        }
    }

    fn rmdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let parent_path = match self.fs.path_by_inode(parent) {
            Ok(p) => p,
            Err(e) => {
                reply_err_from_vfs(&e, reply);
                return;
            }
        };

        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let child = Self::child_path(&parent_path, name_str);

        match self.fs.remove(&child) {
            Ok(()) => reply.ok(),
            Err(e) => reply_err_from_vfs(&e, reply),
        }
    }

    fn rename(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
        _flags: u32,
        reply: ReplyEmpty,
    ) {
        let parent_path = match self.fs.path_by_inode(parent) {
            Ok(p) => p,
            Err(e) => {
                reply_err_from_vfs(&e, reply);
                return;
            }
        };
        let new_parent_path = match self.fs.path_by_inode(newparent) {
            Ok(p) => p,
            Err(e) => {
                reply_err_from_vfs(&e, reply);
                return;
            }
        };

        let (Some(name_str), Some(newname_str)) = (name.to_str(), newname.to_str()) else {
            reply.error(libc::EINVAL);
            return;
        };

        let old_path = Self::child_path(&parent_path, name_str);
        let new_path = Self::child_path(&new_parent_path, newname_str);

        match self.fs.rename(&old_path, &new_path) {
            Ok(()) => reply.ok(),
            Err(e) => reply_err_from_vfs(&e, reply),
        }
    }

    fn open(&mut self, _req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        let path = match self.fs.path_by_inode(ino) {
            Ok(p) => p,
            Err(e) => {
                reply_err_from_vfs(&e, reply);
                return;
            }
        };

        let mode = flags_to_open_mode(flags);

        match self.fs.open(&path, mode) {
            Ok(file) => {
                let fh = self.alloc_fh();
                self.file_handles
                    .write()
                    .unwrap()
                    .insert(fh, OpenFileHandle { file: Box::new(file) });
                reply.opened(fh, 0);
            }
            Err(e) => reply_err_from_vfs(&e, reply),
        }
    }

    fn read(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        let handles = self.file_handles.read().unwrap();
        let Some(handle) = handles.get(&fh) else {
            reply.error(libc::EBADF);
            return;
        };

        let mut buf = vec![0u8; size as usize];
        match handle.file.read_at(&mut buf, offset as u64) {
            Ok(n) => reply.data(&buf[..n]),
            Err(e) => reply_err_from_vfs(&e, reply),
        }
    }

    fn write(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        let handles = self.file_handles.read().unwrap();
        let Some(handle) = handles.get(&fh) else {
            reply.error(libc::EBADF);
            return;
        };

        match handle.file.write_at(data, offset as u64) {
            Ok(n) => reply.written(n as u32),
            Err(e) => reply_err_from_vfs(&e, reply),
        }
    }

    fn release(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        self.file_handles.write().unwrap().remove(&fh);
        reply.ok();
    }

    fn create(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        _flags: i32,
        reply: ReplyCreate,
    ) {
        let parent_path = match self.fs.path_by_inode(parent) {
            Ok(p) => p,
            Err(e) => {
                reply_err_from_vfs(&e, reply);
                return;
            }
        };

        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let child = Self::child_path(&parent_path, name_str);

        match self.fs.create(&child, mode) {
            Ok(file) => {
                let fh = self.alloc_fh();
                self.file_handles
                    .write()
                    .unwrap()
                    .insert(fh, OpenFileHandle { file: Box::new(file) });

                match self.fs.stat(&child) {
                    Ok(meta) => {
                        let attr = self.metadata_to_attr(&meta);
                        reply.created(&self.options.entry_timeout, &attr, 0, fh, 0);
                    }
                    Err(e) => reply_err_from_vfs(&e, reply),
                }
            }
            Err(e) => reply_err_from_vfs(&e, reply),
        }
    }

    fn opendir(&mut self, _req: &Request<'_>, ino: u64, _flags: i32, reply: ReplyOpen) {
        let path = match self.fs.path_by_inode(ino) {
            Ok(p) => p,
            Err(e) => {
                reply_err_from_vfs(&e, reply);
                return;
            }
        };

        match self.fs.open_directory(&path) {
            Ok(dir) => match dir.list() {
                Ok(entries) => {
                    let fh = self.alloc_fh();
                    self.dir_handles
                        .write()
                        .unwrap()
                        .insert(fh, OpenDirHandle { cached_entries: entries });
                    reply.opened(fh, 0);
                }
                Err(e) => reply_err_from_vfs(&e, reply),
            },
            Err(e) => reply_err_from_vfs(&e, reply),
        }
    }

    fn readdir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let dir_handles = self.dir_handles.read().unwrap();
        let Some(handle) = dir_handles.get(&fh) else {
            reply.error(libc::EBADF);
            return;
        };

        let mut idx = offset as usize;

        // "." entry
        if idx == 0 {
            if reply.add(ino, 1, FileType::Directory, ".") {
                reply.ok();
                return;
            }
            idx = 1;
        }

        // ".." entry
        if idx == 1 {
            let parent_ino = if ino == ROOT_INO {
                ROOT_INO
            } else {
                match self.fs.path_by_inode(ino) {
                    Ok(path) => {
                        let parent = path
                            .rfind('/')
                            .map(|pos| &path[..pos])
                            .unwrap_or("/");
                        let parent = if parent.is_empty() { "/" } else { parent };
                        self.fs.inode(parent).unwrap_or(ROOT_INO)
                    }
                    Err(_) => ROOT_INO,
                }
            };
            if reply.add(parent_ino, 2, FileType::Directory, "..") {
                reply.ok();
                return;
            }
            idx = 2;
        }

        let entry_offset = idx - 2;
        for (i, entry) in handle.cached_entries.iter().enumerate().skip(entry_offset) {
            let fuse_ft = match entry.file_type {
                VfsFileType::Regular => FileType::RegularFile,
                VfsFileType::Directory => FileType::Directory,
                VfsFileType::Symlink => FileType::Symlink,
            };
            let next_offset = (i + 3) as i64;
            if reply.add(entry.inode, next_offset, fuse_ft, &entry.name) {
                break;
            }
        }

        reply.ok();
    }

    fn releasedir(&mut self, _req: &Request<'_>, _ino: u64, fh: u64, _flags: i32, reply: ReplyEmpty) {
        self.dir_handles.write().unwrap().remove(&fh);
        reply.ok();
    }

    fn statfs(&mut self, _req: &Request<'_>, _ino: u64, reply: ReplyStatfs) {
        reply.statfs(0, 0, 0, 0, 0, 4096, 255, 0);
    }
}

pub fn flags_to_open_mode(flags: i32) -> OpenMode {
    let access = flags & libc::O_ACCMODE;
    match access {
        libc::O_RDONLY => OpenMode::Read,
        libc::O_WRONLY => OpenMode::Write,
        libc::O_RDWR => OpenMode::ReadWrite,
        _ => OpenMode::Read,
    }
}
