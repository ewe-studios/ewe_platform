#![cfg(feature = "vfs-nfs")]

//! NFS v3 loopback server exposing any VfsFileSystem as an NFS mount.
//!
//! Uses the `nfsserve` crate for NFS protocol handling.
//! Mount on Linux: `mount -t nfs localhost:/ /mnt -o port=PORT,mountport=PORT,nolocks,tcp`
//! Mount on macOS: `mount_nfs -o resvport,port=PORT,mountport=PORT,nolocks,tcp localhost:/ /mnt`

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

use nfsserve::nfs::{
    attrstat3, createhook3, createverf3, entry3, fattr3, fileid3, filename3, fileid3,
    nfsstat3, post_op_attr, post_op_fh3, pre_op_attr, readlink3res, readlink3resok,
    readdir3args, readdir3res, readdir3resok, remove3args, rename3args, rename3res,
    sattr3, sattrguard3, setattrs, symlinkdata3, write3args, write3res, write3resok,
    specdata3, nfstime3, FSF3_LINK, FSF3_SYMLINK, FSF3_HOMOGENEOUS, FSF3_CANSETTIME,
};
use nfsserve::xdr::XdrError;
use nfsserve::vfs::{NFSFileSystem, VFSCapabilities, LookupRes};

use crate::shared::vfs::traits::VfsFileSystem;
use crate::shared::vfs::types::{OpenMode, VfsFileType, VfsMetadata};

/// NFS file handle — encodes an inode number.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct NfsHandle {
    ino: u64,
}

impl nfsserve::xdr::XDR for NfsHandle {
    fn serialize<W: std::io::Write>(&self, _: &mut W) -> Result<(), XdrError> {
        Ok(())
    }
    fn deserialize<R: std::io::Read>(_: &mut R) -> Result<Self, XdrError> {
        Ok(NfsHandle { ino: 0 })
    }
}

impl From<u64> for NfsHandle {
    fn from(ino: u64) -> Self { NfsHandle { ino } }
}

impl From<NfsHandle> for fileid3 {
    fn from(h: NfsHandle) -> Self { h.ino }
}

/// VfsNfs — NFS v3 server backed by any VfsFileSystem.
pub struct VfsNfs<F: VfsFileSystem + 'static> {
    fs: Arc<F>,
    handles: RwLock<HashMap<u64, String>>, // ino → path
    next_handle: std::sync::atomic::AtomicU64,
}

impl<F: VfsFileSystem + 'static> VfsNfs<F> {
    pub fn new(fs: F) -> Self {
        let fs = Arc::new(fs);
        let mut handles = HashMap::new();
        handles.insert(1, "/".to_string());
        Self {
            fs,
            handles: RwLock::new(handles),
            next_handle: std::sync::atomic::AtomicU64::new(2),
        }
    }

    fn resolve_path(&self, handle: NfsHandle) -> Option<String> {
        self.handles.read().unwrap().get(&handle.ino).cloned()
    }

    fn register_path(&self, ino: u64, path: String) {
        self.handles.write().unwrap().insert(ino, path);
    }

    fn to_fattr(&self, meta: &VfsMetadata) -> fattr3 {
        let ftype = match meta.file_type {
            VfsFileType::Regular => nfsserve::nfs::ftype3::NF3REG,
            VfsFileType::Directory => nfsserve::nfs::ftype3::NF3DIR,
            VfsFileType::Symlink => nfsserve::nfs::ftype3::NF3LNK,
        };
        fattr3 {
            ftype,
            mode: meta.permissions,
            nlink: 1,
            uid: meta.owner.0,
            gid: meta.owner.1,
            size: meta.size,
            used: meta.size,
            rdev: specdata3 { specdata1: 0, specdata2: 0 },
            fsid: 1,
            fileid: meta.inode,
            atime: nfstime3 { seconds: 0, nseconds: 0 },
            mtime: nfstime3 { seconds: 0, nseconds: 0 },
            ctime: nfstime3 { seconds: 0, nseconds: 0 },
        }
    }
}

#[async_trait::async_trait]
impl<F: VfsFileSystem + Send + Sync + 'static> NFSFileSystem<NfsHandle> for VfsNfs<F> {
    fn capabilities(&self) -> VFSCapabilities {
        VFSCapabilities::ReadWrite
    }

    fn fs_info(&self) -> nfsserve::nfs::fsinfo3 {
        nfsserve::nfs::fsinfo3 {
            rtmax: 1048576, rtpref: 1048576, rtmult: 4096,
            wtmax: 1048576, wtpref: 1048576, wtmult: 4096,
            dtpref: 4096,
            maxfilesize: 0xffffffffffffffff,
            time_delta: nfstime3 { seconds: 1, nseconds: 0 },
            properties: FSF3_LINK | FSF3_SYMLINK | FSF3_HOMOGENEOUS | FSF3_CANSETTIME,
        }
    }

    async fn root_dir(&self) -> Result<NfsHandle, nfsstat3> {
        Ok(NfsHandle { ino: 1 })
    }

    async fn lookup(&self, parent: NfsHandle, name: filename3) -> Result<LookupRes<NfsHandle>, nfsstat3> {
        let parent_path = self.resolve_path(parent).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let child_path = if parent_path.ends_with('/') {
            format!("{}{}", parent_path, String::from_utf8_lossy(&name))
        } else {
            format!("{}/{}", parent_path, String::from_utf8_lossy(&name))
        };
        match self.fs.stat(&child_path) {
            Ok(meta) => {
                self.register_path(meta.inode, child_path.clone());
                Ok(LookupRes {
                    handle: NfsHandle { ino: meta.inode },
                    post_attr: Some(self.to_fattr(&meta)),
                })
            }
            Err(_) => Err(nfsstat3::NFS3ERR_NOENT),
        }
    }

    async fn getattr(&self, handle: NfsHandle) -> Result<fattr3, nfsstat3> {
        let path = self.resolve_path(handle).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let meta = self.fs.stat(&path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        Ok(self.to_fattr(&meta))
    }

    async fn readlink(&self, handle: NfsHandle) -> Result<readlink3res, nfsstat3> {
        let path = self.resolve_path(handle).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let target = self.fs.readlink(&path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        Ok(readlink3res::Ok(readlink3resok { data: target.into_bytes() }))
    }

    async fn read(
        &self, handle: NfsHandle, offset: u64, count: u32,
    ) -> Result<nfsserve::nfs::read3res, nfsstat3> {
        let path = self.resolve_path(handle).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let file = self.fs.open(&path, OpenMode::Read).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let mut buf = vec![0u8; count as usize];
        let n = file.read_at(&mut buf, offset).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        buf.truncate(n);
        Ok(nfsserve::nfs::read3res::Ok(nfsserve::nfs::read3resok {
            file_attributes: post_op_attr { attributes_follow: false, attributes: None },
            count: n as u32,
            eof: true,
            data: buf,
        }))
    }

    async fn write(&self, handle: NfsHandle, offset: u64, data: Vec<u8>, stable: nfsserve::nfs::stable_how) -> Result<write3res, nfsstat3> {
        let path = self.resolve_path(handle).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let file = self.fs.open(&path, OpenMode::Write).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let n = file.write_at(&data, offset).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        Ok(write3res::Ok(write3resok {
            file_attributes: post_op_attr { attributes_follow: false, attributes: None },
            count: n as u32,
            committed: match stable {
                nfsserve::nfs::stable_how::FILE_SYNC => nfsserve::nfs::stable_how::FILE_SYNC,
                _ => nfsserve::nfs::stable_how::UNSTABLE,
            },
            verf: createverf3 { data: [0; 8] },
        }))
    }

    async fn create(&self, parent: NfsHandle, name: filename3, attrs: sattr3) -> Result<nfsserve::nfs::create3res, nfsstat3> {
        let parent_path = self.resolve_path(parent).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let path = if parent_path.ends_with('/') {
            format!("{}{}", parent_path, String::from_utf8_lossy(&name))
        } else {
            format!("{}/{}", parent_path, String::from_utf8_lossy(&name))
        };
        let mode = attrs.mode.unwrap_or(0o644);
        let file = self.fs.create(&path, mode).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let meta = file.metadata().map_err(|_| nfsstat3::NFS3ERR_IO)?;
        self.register_path(meta.inode, path.clone());
        Ok(nfsserve::nfs::create3res::Ok(nfsserve::nfs::create3resok {
            handle: NfsHandle { ino: meta.inode },
            post_op_dir_attributes: post_op_attr { attributes_follow: false, attributes: None },
            post_op_file_attributes: post_op_attr { attributes_follow: true, attributes: Some(self.to_fattr(&meta)) },
        }))
    }

    async fn mkdir(&self, parent: NfsHandle, name: filename3, attrs: sattr3) -> Result<nfsserve::nfs::mkdir3res, nfsstat3> {
        let parent_path = self.resolve_path(parent).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let path = if parent_path.ends_with('/') {
            format!("{}{}", parent_path, String::from_utf8_lossy(&name))
        } else {
            format!("{}/{}", parent_path, String::from_utf8_lossy(&name))
        };
        self.fs.mkdir(&path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let meta = self.fs.stat(&path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        self.register_path(meta.inode, path.clone());
        Ok(nfsserve::nfs::mkdir3res::Ok(nfsserve::nfs::mkdir3resok {
            handle: NfsHandle { ino: meta.inode },
            post_op_dir_attributes: post_op_attr { attributes_follow: true, attributes: Some(self.to_fattr(&meta)) },
            post_op_parent_attributes: post_op_attr { attributes_follow: false, attributes: None },
        }))
    }

    async fn remove(&self, parent: NfsHandle, name: filename3) -> Result<remove3args, nfsstat3> {
        let parent_path = self.resolve_path(parent).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let path = if parent_path.ends_with('/') {
            format!("{}{}", parent_path, String::from_utf8_lossy(&name))
        } else {
            format!("{}/{}", parent_path, String::from_utf8_lossy(&name))
        };
        self.fs.remove(&path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        Ok(remove3args { dir_attributes: post_op_attr { attributes_follow: false, attributes: None } })
    }

    async fn rename(&self, from_parent: NfsHandle, from_name: filename3, to_parent: NfsHandle, to_name: filename3) -> Result<rename3res, nfsstat3> {
        let from_parent_path = self.resolve_path(from_parent).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let from = if from_parent_path.ends_with('/') {
            format!("{}{}", from_parent_path, String::from_utf8_lossy(&from_name))
        } else {
            format!("{}/{}", from_parent_path, String::from_utf8_lossy(&from_name))
        };
        let to_parent_path = self.resolve_path(to_parent).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let to = if to_parent_path.ends_with('/') {
            format!("{}{}", to_parent_path, String::from_utf8_lossy(&to_name))
        } else {
            format!("{}/{}", to_parent_path, String::from_utf8_lossy(&to_name))
        };
        self.fs.rename(&from, &to).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        Ok(rename3res::Ok(nfsserve::nfs::rename3resok {
            from_dir_attributes: post_op_attr { attributes_follow: false, attributes: None },
            to_dir_attributes: post_op_attr { attributes_follow: false, attributes: None },
        }))
    }

    async fn symlink(&self, parent: NfsHandle, name: filename3, symlink: symlinkdata3) -> Result<nfsserve::nfs::symlink3res, nfsstat3> {
        let parent_path = self.resolve_path(parent).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let path = if parent_path.ends_with('/') {
            format!("{}{}", parent_path, String::from_utf8_lossy(&name))
        } else {
            format!("{}/{}", parent_path, String::from_utf8_lossy(&name))
        };
        let target = String::from_utf8_lossy(&symlink.symlink_data);
        self.fs.symlink(&target, &path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let meta = self.fs.stat(&path).ok();
        Ok(nfsserve::nfs::symlink3res::Ok(nfsserve::nfs::symlink3resok {
            handle: NfsHandle { ino: meta.as_ref().map(|m| m.inode).unwrap_or(0) },
            post_op_dir_attributes: post_op_attr { attributes_follow: false, attributes: None },
        }))
    }

    async fn readdir(&self, dir: NfsHandle, cookie: u64, cookieverf: createverf3, count: u32) -> Result<readdir3res, nfsstat3> {
        let path = self.resolve_path(dir).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let dir_handle = self.fs.open_directory(&path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let entries = dir_handle.list().map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let mut nfs_entries = Vec::new();
        for (i, entry) in entries.iter().enumerate().skip(cookie as usize) {
            self.register_path(entry.inode, {
                if path.ends_with('/') { format!("{}{}", path, entry.name) } else { format!("{}/{}", path, entry.name) }
            });
            nfs_entries.push(entry3 {
                fileid: entry.inode,
                name: entry.name.clone().into_bytes(),
                cookie: (cookie + i as u64 + 1),
                name_handle: post_op_fh3 { handle_follows: true, handle: Some(NfsHandle { ino: entry.inode }) },
            });
        }
        if nfs_entries.len() >= count as usize {
            nfs_entries.pop();
        }
        let eof = nfs_entries.len() < entries.len();
        Ok(readdir3res::Ok(readdir3resok {
            cookieverf,
            entries: nfs_entries,
            reply_eof: eof,
            dir_attributes: post_op_attr { attributes_follow: false, attributes: None },
        }))
    }

    async fn setattr(&self, handle: NfsHandle, attrs: sattr3, guard: sattrguard3) -> Result<attrstat3, nfsstat3> {
        let path = self.resolve_path(handle).ok_or(nfsstat3::NFS3ERR_STALE)?;
        if let Some(mode) = attrs.mode {
            self.fs.chmod(&path, mode).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        }
        let meta = self.fs.stat(&path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        Ok(attrstat3 { status: nfsstat3::NFS3_OK, obj_attributes: post_op_attr { attributes_follow: true, attributes: Some(self.to_fattr(&meta)) } })
    }
}
