#![cfg(feature = "vfs-nfs")]

//! NFS v3 loopback server exposing any VfsFileSystem as an NFS mount.
//!
//! Uses the `nfsserve` crate for NFS protocol handling.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use nfsserve::nfs::*;
use nfsserve::vfs::{NFSFileSystem, VFSCapabilities, ReadDirResult, DirEntry};

use crate::shared::vfs::traits::{VfsFileSystem, VfsFile, VfsDirectory};
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

fn to_fattr3(fs: &impl VfsFileSystem, path: &str, id: u64) -> Result<fattr3, nfsstat3> {
    let meta = fs.stat(path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
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

/// VfsNfs — NFS v3 server backed by any VfsFileSystem.
pub struct VfsNfs<F: VfsFileSystem + 'static> {
    fs: F,
    mapper: Arc<IdMapper>,
}

impl<F: VfsFileSystem> VfsNfs<F> {
    pub fn new(fs: F) -> Self {
        Self { fs, mapper: Arc::new(IdMapper::new()) }
    }
}

#[async_trait::async_trait]
impl<F: VfsFileSystem + Send + Sync> NFSFileSystem for VfsNfs<F> {
    fn capabilities(&self) -> VFSCapabilities {
        VFSCapabilities::ReadWrite
    }

    fn root_dir(&self) -> fileid3 { 1 }

    async fn lookup(&self, dirid: fileid3, filename: &filename3) -> Result<fileid3, nfsstat3> {
        let dir_path = self.mapper.get_path(dirid).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let name = String::from_utf8_lossy(filename).to_string();
        let child_path = if dir_path.ends_with('/') { format!("{}{}", dir_path, name) } else { format!("{}/{}", dir_path, name) };
        if self.fs.exists(&child_path).unwrap_or(false) {
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
        to_fattr3(&self.fs, &path, id)
    }

    async fn setattr(&self, id: fileid3, setattr: sattr3) -> Result<fattr3, nfsstat3> {
        let path = self.mapper.get_path(id).ok_or(nfsstat3::NFS3ERR_STALE)?;
        if let set_mode3::mode(m) = setattr.mode {
            self.fs.chmod(&path, m).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        }
        to_fattr3(&self.fs, &path, id)
    }

    async fn read(&self, id: fileid3, offset: u64, count: u32) -> Result<(Vec<u8>, bool), nfsstat3> {
        let path = self.mapper.get_path(id).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let file = self.fs.open(&path, OpenMode::Read).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let size = file.size().map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let to_read = std::cmp::min(count as usize, (size.saturating_sub(offset)) as usize);
        let mut buf = vec![0u8; to_read];
        file.read_at(&mut buf, offset).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        Ok((buf, offset + to_read as u64 >= size))
    }

    async fn write(&self, id: fileid3, offset: u64, data: &[u8]) -> Result<fattr3, nfsstat3> {
        let path = self.mapper.get_path(id).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let file = self.fs.open(&path, OpenMode::ReadWrite).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        file.write_at(data, offset).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        to_fattr3(&self.fs, &path, id)
    }

    async fn create(&self, dirid: fileid3, filename: &filename3, _attr: sattr3) -> Result<(fileid3, fattr3), nfsstat3> {
        let dir_path = self.mapper.get_path(dirid).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let name = String::from_utf8_lossy(filename).to_string();
        let child_path = if dir_path.ends_with('/') { format!("{}{}", dir_path, name) } else { format!("{}/{}", dir_path, name) };
        self.fs.create(&child_path, 0o644).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let id = self.mapper.next_id();
        self.mapper.register(&child_path, id);
        Ok((id, to_fattr3(&self.fs, &child_path, id)?))
    }

    async fn create_exclusive(&self, dirid: fileid3, filename: &filename3) -> Result<fileid3, nfsstat3> {
        let dir_path = self.mapper.get_path(dirid).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let name = String::from_utf8_lossy(filename).to_string();
        let child_path = if dir_path.ends_with('/') { format!("{}{}", dir_path, name) } else { format!("{}/{}", dir_path, name) };
        if self.fs.exists(&child_path).unwrap_or(false) {
            return Err(nfsstat3::NFS3ERR_EXIST);
        }
        self.fs.create(&child_path, 0o644).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let id = self.mapper.next_id();
        self.mapper.register(&child_path, id);
        Ok(id)
    }

    async fn mkdir(&self, dirid: fileid3, dirname: &filename3) -> Result<(fileid3, fattr3), nfsstat3> {
        let dir_path = self.mapper.get_path(dirid).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let name = String::from_utf8_lossy(dirname).to_string();
        let child_path = if dir_path.ends_with('/') { format!("{}{}", dir_path, name) } else { format!("{}/{}", dir_path, name) };
        self.fs.mkdir(&child_path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let id = self.mapper.next_id();
        self.mapper.register(&child_path, id);
        Ok((id, to_fattr3(&self.fs, &child_path, id)?))
    }

    async fn remove(&self, dirid: fileid3, filename: &filename3) -> Result<(), nfsstat3> {
        let _ = dirid;
        let name = String::from_utf8_lossy(filename).to_string();
        self.fs.remove(&name).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        if let Some(id) = self.mapper.get_id(&name) {
            self.mapper.id_to_path.write().unwrap().remove(&id);
        }
        self.mapper.path_to_id.write().unwrap().remove(&name);
        Ok(())
    }

    async fn rename(&self, from_dirid: fileid3, from_name: &filename3, _to_dirid: fileid3, to_name: &filename3) -> Result<(), nfsstat3> {
        let _ = from_dirid;
        let from_name = String::from_utf8_lossy(from_name).to_string();
        let to_name = String::from_utf8_lossy(to_name).to_string();
        self.fs.rename(&from_name, &to_name).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        if let Some(id) = self.mapper.get_id(&from_name) {
            self.mapper.register(&to_name, id);
            self.mapper.path_to_id.write().unwrap().remove(&from_name);
        }
        Ok(())
    }

    async fn readdir(&self, dirid: fileid3, _start_after: fileid3, max_entries: usize) -> Result<ReadDirResult, nfsstat3> {
        let dir_path = self.mapper.get_path(dirid).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let dir = self.fs.open_directory(&dir_path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let entries = dir.list().map_err(|_| nfsstat3::NFS3ERR_IO)?;

        let mut result = Vec::new();
        let mut eof = true;

        for entry in entries.iter().take(max_entries) {
            let child_path = if dir_path.ends_with('/') { format!("{}{}", dir_path, entry.name) } else { format!("{}/{}", dir_path, entry.name) };
            let id = self.mapper.get_id(&child_path).unwrap_or_else(|| {
                let id = self.mapper.next_id();
                self.mapper.register(&child_path, id);
                id
            });
            let attr = to_fattr3(&self.fs, &child_path, id)?;
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
        let child_path = if dir_path.ends_with('/') { format!("{}{}", dir_path, name) } else { format!("{}/{}", dir_path, name) };
        let target = String::from_utf8_lossy(symlink).to_string();
        self.fs.symlink(&target, &child_path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
        let id = self.mapper.next_id();
        self.mapper.register(&child_path, id);
        let attr = to_fattr3(&self.fs, &child_path, id)?;
        Ok((id, attr))
    }

    async fn readlink(&self, id: fileid3) -> Result<nfspath3, nfsstat3> {
        let path = self.mapper.get_path(id).ok_or(nfsstat3::NFS3ERR_STALE)?;
        let target = self.fs.readlink(&path).map_err(|_| nfsstat3::NFS3ERR_IO)?;
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
        } else if self.fs.exists(&path_str).unwrap_or(false) {
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
