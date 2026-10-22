#![cfg(feature = "vfs-nfs")]

//! NFS v3 loopback server exposing any AsyncVfsFileSystem as an NFS mount.
//!
//! Uses the `nfsserve` crate for NFS protocol handling.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use nfsserve::nfs::*;
use nfsserve::vfs::{NFSFileSystem, VFSCapabilities, ReadDirResult, DirEntry};

use crate::shared::vfs::async_traits::{AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem};
use crate::shared::vfs::types::{OpenMode, VfsFileType};

struct IdMapper {
    next_id: std::sync::atomic::AtomicU64,
    id_to_path: RwLock<HashMap<u64, String>>,
    path_to_id: RwLock<HashMap<String, u64>>,
}

impl IdMapper {
    fn new() -> Self {
        let m = Self {
            next_id: std::sync::atomic::AtomicU64::new(1),
            id_to_path: RwLock::new(HashMap::new()),
            path_to_id: RwLock::new(HashMap::new()),
        };
        m.register("/", 1);
        m
    }

    fn register(&self, path: &str, id: u64) {
        self.id_to_path.write().unwrap().insert(id, path.to_string());
        self.path_to_id.write().unwrap().insert(path.to_string(), id);
    }

    fn get_id(&self, path: &str) -> Option<u64> {
        self.path_to_id.read().unwrap().get(path).copied()
    }

    fn get_path(&self, id: u64) -> Option<String> {
        self.id_to_path.read().unwrap().get(&id).cloned()
    }

    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}

fn to_nfs_ftype(ft: VfsFileType) -> ftype3 {
    match ft {
        VfsFileType::Regular => ftype3::NF3REG,
        VfsFileType::Directory => ftype3::NF3DIR,
        VfsFileType::Symlink => ftype3::NF3LNK,
    }
}

fn now_ns() -> nfstime3 {
    nfstime3 { seconds: 0, nseconds: 0 }
}

fn child_of(dir_path: &str, name: &str) -> String {
    if dir_path.ends_with('/') {
        format!("{dir_path}{name}")
    } else {
        format!("{dir_path}/{name}")
    }
}

async fn to_fattr3(fs: &impl AsyncVfsFileSystem, path: &str, id: u64) -> Result<fattr3, nfsstat3> {
    let meta = fs.stat_async(path.to_string()).await.map_err(|_| nfsstat3::NFS3ERR_IO)?;
    Ok(fattr3 {
        ftype: to_nfs_ftype(meta.file_type),
        mode: meta.permissions,
        nlink: 1,
        uid: meta.owner.0,
        gid: meta.owner.1,
        size: meta.size,
        used: meta.size,
        rdev: specdata3 { specdata1: 0, specdata2: 0 },
        fsid: 1,
        fileid: id,
        atime: now_ns(),
        mtime: now_ns(),
        ctime: now_ns(),
    })
}

/// VfsNfs — NFS v3 server backed by any AsyncVfsFileSystem.
pub struct VfsNfs<F: AsyncVfsFileSystem + 'static> {
    fs: F,
    mapper: Arc<IdMapper>,
}

impl<F: AsyncVfsFileSystem> VfsNfs<F> {
    pub fn new(fs: F) -> Self {
        Self { fs, mapper: Arc::new(IdMapper::new()) }
    }
}

#[async_trait::async_trait]
impl<F: AsyncVfsFileSystem> NFSFileSystem for VfsNfs<F> {
    fn capabilities(&self) -> VFSCapabilities {
        VFSCapabilities::ReadWrite
    }

    fn root_dir(&self) -> fileid3 { 1 }

    async fn lookup(&self, dirid: fileid3, filename: &filename3) -> Result<fileid3, nfsstat3> {
        let dir_path = self.mapper.get_path(dirid).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let name = String::from_utf8_lossy(filename).to_string();
        let child_path = child_of(&dir_path, &name);
        if self.fs.exists_async(child_path.clone()).await.unwrap_or(false) {
            let id = self.mapper.get_id(&child_path).unwrap_or_else(|| {
                let id = self.mapper.next_id();
                self.mapper.register(&child_path, id);
                id
            });
            Ok(id)
        } else {
            Err(nfsstat3::NFS3ERR_NOENT)
        }
    }

    async fn getattr(&self, id: fileid3) -> Result<fattr3, nfsstat3> {
        let path = self.mapper.get_path(id).ok_or(nfsstat3::NFS3ERR_STALE)?;
        to_fattr3(&self.fs, &path, id).await
    }

    async fn setattr(&self, id: fileid3, setattr: sattr3) -> Result<fattr3, nfsstat3> {
        let path = self.mapper.get_path(id).ok_or(nfsstat3::NFS3ERR_STALE)?;
        if let set_mode3::mode(m) = setattr.mode {
            self.fs.chmod_async(path.clone(), m).await.map_err(|_| nfsstat3::NFS3ERR_IO)?;
        }
        to_fattr3(&self.fs, &path, id).await
    }

    async fn read(&self, id: fileid3, offset: u64, count: u32) -> Result<(Vec<u8>, bool), nfsstat3> {
        let path = self.mapper.get_path(id).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let file = self.fs.open_async(path.clone(), OpenMode::Read).await.map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let size = file.size_async().await.map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let to_read = std::cmp::min(count as usize, (size.saturating_sub(offset)) as usize);
        let buf = file.read_at_async(to_read, offset).await.map_err(|_| nfsstat3::NFS3ERR_IO)?;
        Ok((buf, offset + to_read as u64 >= size))
    }

    async fn write(&self, id: fileid3, offset: u64, data: &[u8]) -> Result<fattr3, nfsstat3> {
        let path = self.mapper.get_path(id).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let file = self.fs.open_async(path.clone(), OpenMode::ReadWrite).await.map_err(|_| nfsstat3::NFS3ERR_IO)?;
        file.write_at_async(data.to_vec(), offset).await.map_err(|_| nfsstat3::NFS3ERR_IO)?;
        to_fattr3(&self.fs, &path, id).await
    }

    async fn create(&self, dirid: fileid3, filename: &filename3, _attr: sattr3) -> Result<(fileid3, fattr3), nfsstat3> {
        let dir_path = self.mapper.get_path(dirid).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let name = String::from_utf8_lossy(filename).to_string();
        let child_path = child_of(&dir_path, &name);
        self.fs.create_async(child_path.clone(), 0o644).await.map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let id = self.mapper.next_id();
        self.mapper.register(&child_path, id);
        Ok((id, to_fattr3(&self.fs, &child_path, id).await?))
    }

    async fn create_exclusive(&self, dirid: fileid3, filename: &filename3) -> Result<fileid3, nfsstat3> {
        let dir_path = self.mapper.get_path(dirid).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let name = String::from_utf8_lossy(filename).to_string();
        let child_path = child_of(&dir_path, &name);
        if self.fs.exists_async(child_path.clone()).await.unwrap_or(false) {
            return Err(nfsstat3::NFS3ERR_EXIST);
        }
        self.fs.create_async(child_path.clone(), 0o644).await.map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let id = self.mapper.next_id();
        self.mapper.register(&child_path, id);
        Ok(id)
    }

    async fn mkdir(&self, dirid: fileid3, dirname: &filename3) -> Result<(fileid3, fattr3), nfsstat3> {
        let dir_path = self.mapper.get_path(dirid).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let name = String::from_utf8_lossy(dirname).to_string();
        let child_path = child_of(&dir_path, &name);
        self.fs.mkdir_async(child_path.clone()).await.map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let id = self.mapper.next_id();
        self.mapper.register(&child_path, id);
        Ok((id, to_fattr3(&self.fs, &child_path, id).await?))
    }

    async fn remove(&self, dirid: fileid3, filename: &filename3) -> Result<(), nfsstat3> {
        let dir_path = self.mapper.get_path(dirid).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let name = String::from_utf8_lossy(filename).to_string();
        let child_path = child_of(&dir_path, &name);
        self.fs.remove_async(child_path.clone()).await.map_err(|_| nfsstat3::NFS3ERR_IO)?;
        if let Some(id) = self.mapper.get_id(&child_path) {
            self.mapper.id_to_path.write().unwrap().remove(&id);
        }
        self.mapper.path_to_id.write().unwrap().remove(&child_path);
        Ok(())
    }

    async fn rename(&self, from_dirid: fileid3, from_name: &filename3, to_dirid: fileid3, to_name: &filename3) -> Result<(), nfsstat3> {
        let from_dir = self.mapper.get_path(from_dirid).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let to_dir = self.mapper.get_path(to_dirid).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let from_path = child_of(&from_dir, &String::from_utf8_lossy(from_name));
        let to_path = child_of(&to_dir, &String::from_utf8_lossy(to_name));
        self.fs.rename_async(from_path.clone(), to_path.clone()).await.map_err(|_| nfsstat3::NFS3ERR_IO)?;
        if let Some(id) = self.mapper.get_id(&from_path) {
            self.mapper.register(&to_path, id);
            self.mapper.path_to_id.write().unwrap().remove(&from_path);
        }
        Ok(())
    }

    async fn readdir(&self, dirid: fileid3, _start_after: fileid3, max_entries: usize) -> Result<ReadDirResult, nfsstat3> {
        let dir_path = self.mapper.get_path(dirid).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let dir = self.fs.open_directory_async(dir_path.clone()).await.map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let entries = dir.list_async().await.map_err(|_| nfsstat3::NFS3ERR_IO)?;

        let mut result = Vec::new();
        let mut eof = true;

        for entry in entries.iter().take(max_entries) {
            let child_path = child_of(&dir_path, &entry.name);
            let id = self.mapper.get_id(&child_path).unwrap_or_else(|| {
                let id = self.mapper.next_id();
                self.mapper.register(&child_path, id);
                id
            });
            let attr = to_fattr3(&self.fs, &child_path, id).await?;
            result.push(DirEntry {
                fileid: id,
                name: nfsstring(entry.name.clone().into_bytes()),
                attr,
            });
        }

        if entries.len() > max_entries {
            eof = false;
        }

        Ok(ReadDirResult { entries: result, end: eof })
    }

    async fn symlink(&self, dirid: fileid3, linkname: &filename3, symlink: &nfspath3, _attr: &sattr3) -> Result<(fileid3, fattr3), nfsstat3> {
        let dir_path = self.mapper.get_path(dirid).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let name = String::from_utf8_lossy(linkname).to_string();
        let child_path = child_of(&dir_path, &name);
        let target = String::from_utf8_lossy(symlink).to_string();
        self.fs.symlink_async(target.clone(), child_path.clone()).await.map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let id = self.mapper.next_id();
        self.mapper.register(&child_path, id);
        let attr = to_fattr3(&self.fs, &child_path, id).await?;
        Ok((id, attr))
    }

    async fn readlink(&self, id: fileid3) -> Result<nfspath3, nfsstat3> {
        let path = self.mapper.get_path(id).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let target = self.fs.readlink_async(path.clone()).await.map_err(|_| nfsstat3::NFS3ERR_IO)?;
        Ok(target.into_bytes().into())
    }

    async fn fsinfo(&self, _root_fileid: fileid3) -> Result<fsinfo3, nfsstat3> {
        Ok(fsinfo3 {
            obj_attributes: post_op_attr::Void,
            rtmax: 1048576, rtpref: 1048576, rtmult: 4096,
            wtmax: 1048576, wtpref: 1048576, wtmult: 4096,
            dtpref: 4096,
            maxfilesize: 0xffffffffffffffff,
            time_delta: nfstime3 { seconds: 1, nseconds: 0 },
            properties: 0x0001 | 0x0002 | 0x0008 | 0x0010, // FSF3_LINK | SYMLINK | HOMOGENEOUS | CANSETTIME
        })
    }

    fn id_to_fh(&self, id: fileid3) -> nfs_fh3 {
        nfs_fh3 { data: id.to_le_bytes().to_vec() }
    }

    fn fh_to_id(&self, fh: &nfs_fh3) -> Result<fileid3, nfsstat3> {
        if fh.data.len() == 8 {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&fh.data);
            Ok(u64::from_le_bytes(bytes))
        } else {
            Err(nfsstat3::NFS3ERR_BADHANDLE)
        }
    }

    async fn path_to_id(&self, path: &[u8]) -> Result<fileid3, nfsstat3> {
        let path_str = String::from_utf8_lossy(path).to_string();
        if let Some(id) = self.mapper.get_id(&path_str) {
            Ok(id)
        } else if self.fs.exists_async(path_str.clone()).await.unwrap_or(false) {
            let id = self.mapper.next_id();
            self.mapper.register(&path_str, id);
            Ok(id)
        } else {
            Err(nfsstat3::NFS3ERR_NOENT)
        }
    }

    fn serverid(&self) -> cookieverf3 {
        [0; 8]
    }
}
