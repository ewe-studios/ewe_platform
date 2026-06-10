#![cfg(feature = "vfs-r2")]

//! Cloudflare R2 S3-compatible DeltaStore.
//!
//! Uses foundation_db's `R2Store` (`BlobStore`) for HTTP transport.
//!
//! Storage layout:
//! - `vfs/data/{path}` — file content via BlobStore
//! - `vfs/meta/{path}` — VfsMetadata JSON via BlobStore
//! - `vfs/whiteout/{path}` — whiteout tombstone via BlobStore

pub mod types;

pub use types::{KeyLayout, KeyLayoutAdapter, PathKeyLayout, R2FsConfig, R2ObjectMeta};

use foundation_core::valtron::collect_one;
use foundation_db::R2Store;
use foundation_db::core::storage_provider::BlobStore;

use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::traits::{DeltaStore, VfsFile, VfsFileSystem};
use crate::shared::vfs::types::{
    OpenMode, VfsCapabilities, VfsDirEntry, VfsEntryState, VfsFileType, VfsMetadata,
};

/// R2Delta — Cloudflare R2 S3-compatible DeltaStore.
pub struct R2Delta {
    store: R2Store,
}

impl R2Delta {
    pub fn from_env() -> VfsResult<Self> {
        let account_id = std::env::var("CF_ACCOUNT_ID").map_err(|e| VfsError::Backend { message: format!("CF_ACCOUNT_ID not set: {e}") })?;
        let api_token = std::env::var("CF_API_TOKEN").map_err(|e| VfsError::Backend { message: format!("CF_API_TOKEN not set: {e}") })?;
        let bucket = std::env::var("CF_R2_BUCKET").map_err(|e| VfsError::Backend { message: format!("CF_R2_BUCKET not set: {e}") })?;
        let store = R2Store::new_blob(&api_token, &account_id, &bucket, "vfs");
        Ok(Self { store })
    }

    pub fn new(api_token: &str, account_id: &str, bucket: &str) -> VfsResult<Self> {
        let store = R2Store::new_blob(api_token, account_id, bucket, "vfs");
        Ok(Self { store })
    }

    fn data_key(path: &str) -> String { format!("vfs/data/{}", path.trim_start_matches('/')) }
    fn meta_key(path: &str) -> String { format!("vfs/meta/{}", path.trim_start_matches('/')) }
    fn whiteout_key(path: &str) -> String { format!("vfs/whiteout/{}", path.trim_start_matches('/')) }

    fn store_meta(&self, path: &str, meta: &VfsMetadata) -> VfsResult<()> {
        let bytes = serde_json::to_vec(meta).map_err(|e| VfsError::Backend { message: format!("meta serialize: {e}") })?;
        let stream = self.store.put_blob(&Self::meta_key(path), &bytes).map_err(serr)?;
        collect_one(stream);
        Ok(())
    }

    fn get_meta(&self, path: &str) -> VfsResult<Option<VfsMetadata>> {
        let stream = self.store.get_blob(&Self::meta_key(path)).map_err(serr)?;
        let data: Option<Vec<u8>> = collect_blob(stream)?;
        match data {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes).map_err(|e| VfsError::Backend { message: format!("meta deserialize: {e}") })?)),
            None => Ok(None),
        }
    }

    fn clone_arc(&self) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self { store: self.store.clone() })
    }
}

/// Simple file wrapping content in memory, persisted to R2.
pub struct R2File {
    content: std::sync::Arc<std::sync::RwLock<Vec<u8>>>,
    delta: std::sync::Arc<R2Delta>,
    path: String,
    mode: OpenMode,
}

impl crate::shared::vfs::traits::VfsFile for R2File {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let c = self.content.read().unwrap();
        let off = offset as usize;
        if off >= c.len() { return Ok(0); }
        let n = buf.len().min(c.len() - off);
        buf[..n].copy_from_slice(&c[off..off + n]);
        Ok(n)
    }
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        if self.mode == OpenMode::Read { return Err(VfsError::ReadOnly.into()); }
        let mut c = self.content.write().unwrap();
        let end = offset as usize + buf.len();
        if end > c.len() { c.resize(end, 0); }
        c[offset as usize..end].copy_from_slice(buf);
        let n = buf.len();
        drop(c);
        let key = R2Delta::data_key(&self.path);
        let data = { self.content.read().unwrap().clone() };
        let stream = self.delta.store.put_blob(&key, &data).map_err(serr)?;
        collect_one(stream);
        Ok(n)
    }
    fn sync_data(&self) -> VfsResult<()> { Ok(()) }
    fn size(&self) -> VfsResult<u64> { Ok(self.content.read().unwrap().len() as u64) }
    fn truncate(&self, size: u64) -> VfsResult<()> {
        self.content.write().unwrap().truncate(size as usize);
        Ok(())
    }
    fn metadata(&self) -> VfsResult<VfsMetadata> { self.delta.stat(&self.path) }
}

pub struct R2SeekableFile { file: R2File, pos: std::sync::Arc<std::sync::RwLock<u64>> }

impl crate::shared::vfs::traits::VfsFile for R2SeekableFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> { self.file.read_at(buf, offset) }
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> { self.file.write_at(buf, offset) }
    fn sync_data(&self) -> VfsResult<()> { Ok(()) }
    fn size(&self) -> VfsResult<u64> { self.file.size() }
    fn truncate(&self, size: u64) -> VfsResult<()> { self.file.truncate(size) }
    fn metadata(&self) -> VfsResult<VfsMetadata> { self.file.metadata() }
}

impl crate::shared::vfs::traits::SeekableVfsFile for R2SeekableFile {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        let pos = *self.pos.read().unwrap(); let n = self.file.read_at(buf, pos)?;
        *self.pos.write().unwrap() = pos + n as u64; Ok(n)
    }
    fn write(&mut self, buf: &[u8]) -> VfsResult<usize> {
        let pos = *self.pos.read().unwrap(); let n = self.file.write_at(buf, pos)?;
        *self.pos.write().unwrap() = pos + n as u64; Ok(n)
    }
    fn seek(&mut self, pos: std::io::SeekFrom) -> VfsResult<u64> {
        let cur = *self.pos.read().unwrap(); let size = self.file.size()?;
        let new_pos = match pos { std::io::SeekFrom::Start(p) => p, std::io::SeekFrom::End(p) => (size as i64 + p) as u64, std::io::SeekFrom::Current(p) => (cur as i64 + p) as u64 };
        *self.pos.write().unwrap() = new_pos; Ok(new_pos)
    }
    fn position(&self) -> u64 { *self.pos.read().unwrap() }
}

pub struct R2Directory { path: String, delta: std::sync::Arc<R2Delta> }

impl crate::shared::vfs::traits::VfsDirectory for R2Directory {
    type File = R2File; type SeekableFile = R2SeekableFile;
    fn path(&self) -> &str { &self.path }
    fn metadata(&self) -> VfsResult<VfsMetadata> { self.delta.stat(&self.path) }
    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> { Ok(Vec::new()) }
    fn get_entry(&self, _name: &str) -> VfsResult<Option<VfsDirEntry>> { Ok(None) }
    fn create_file(&self, name: &str, mode: u32) -> VfsResult<Self::File> {
        let p = if self.path.ends_with('/') { format!("{}{}", self.path, name) } else { format!("{}/{}", self.path, name) };
        self.delta.create(&p, mode)
    }
    fn create_dir(&self, name: &str) -> VfsResult<Box<dyn crate::shared::vfs::traits::VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        let p = if self.path.ends_with('/') { format!("{}{}", self.path, name) } else { format!("{}/{}", self.path, name) };
        self.delta.mkdir(&p)?; Ok(Box::new(R2Directory { path: p, delta: self.delta.clone() }))
    }
    fn remove_entry(&self, name: &str) -> VfsResult<()> {
        let p = if self.path.ends_with('/') { format!("{}{}", self.path, name) } else { format!("{}/{}", self.path, name) };
        self.delta.remove(&p)
    }
    fn rename_entry(&self, old: &str, new: &str) -> VfsResult<()> {
        let f = if self.path.ends_with('/') { format!("{}{}", self.path, old) } else { format!("{}/{}", self.path, old) };
        let t = if self.path.ends_with('/') { format!("{}{}", self.path, new) } else { format!("{}/{}", self.path, new) };
        self.delta.rename(&f, &t)
    }
    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let f = if path.starts_with('/') { path.to_string() } else if self.path.ends_with('/') { format!("{}{}", self.path, path) } else { format!("{}/{}", self.path, path) };
        self.delta.open(&f, mode)
    }
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> { self.delta.open_seekable(path, mode) }
    fn open_directory(&self, path: &str) -> VfsResult<Box<dyn crate::shared::vfs::traits::VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        let f = if path.starts_with('/') { path.to_string() } else if self.path.ends_with('/') { format!("{}{}", self.path, path) } else { format!("{}/{}", self.path, path) };
        self.delta.open_directory(&f).map(|d| Box::new(d) as Box<dyn crate::shared::vfs::traits::VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>)
    }
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let f = if path.starts_with('/') { path.to_string() } else if self.path.ends_with('/') { format!("{}{}", self.path, path) } else { format!("{}/{}", self.path, path) };
        self.delta.stat(&f)
    }
    fn exists(&self, path: &str) -> VfsResult<bool> {
        let f = if path.starts_with('/') { path.to_string() } else if self.path.ends_with('/') { format!("{}{}", self.path, path) } else { format!("{}/{}", self.path, path) };
        self.delta.exists(&f)
    }
}

impl VfsFileSystem for R2Delta {
    type File = R2File; type SeekableFile = R2SeekableFile; type Directory = R2Directory;
    fn capabilities(&self) -> VfsCapabilities { VfsCapabilities { seekable: true, symlinks: false, permissions_enforced: false, event_emission: false, persistent: true } }
    fn create(&self, path: &str, _mode: u32) -> VfsResult<Self::File> {
        let s = self.store.put_blob(&Self::data_key(path), &[]).map_err(serr)?; drain_ok(s)?;
        self.store_meta(path, &new_meta(path, 0, VfsFileType::Regular))?;
        Ok(R2File { content: std::sync::Arc::new(std::sync::RwLock::new(Vec::new())), delta: self.clone_arc(), path: path.to_string(), mode: OpenMode::ReadWrite })
    }
    fn mkdir(&self, path: &str) -> VfsResult<()> { self.store_meta(path, &new_meta(path, 0, VfsFileType::Directory)) }
    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let s = self.store.get_blob(&Self::data_key(path)).map_err(serr)?;
        let data: Option<Vec<u8>> = collect_blob(s)?;
        Ok(R2File { content: std::sync::Arc::new(std::sync::RwLock::new(data.unwrap_or_default())), delta: self.clone_arc(), path: path.to_string(), mode })
    }
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        Ok(R2SeekableFile { file: self.open(path, mode)?, pos: std::sync::Arc::new(std::sync::RwLock::new(0)) })
    }
    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        self.get_meta(path)?; Ok(R2Directory { path: path.to_string(), delta: self.clone_arc() })
    }
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        if let Some(m) = self.get_meta(path)? { return Ok(m); }
        let s = self.store.get_blob(&Self::data_key(path)).map_err(serr)?;
        let d: Option<Vec<u8>> = collect_blob(s)?;
        match d { Some(c) => Ok(new_meta(path, c.len() as u64, VfsFileType::Regular)), None => Err(VfsError::Backend { message: format!("not found: {path}") }.into()) }
    }
    fn exists(&self, path: &str) -> VfsResult<bool> {
        if whiteout_exists(self, path)? { return Ok(false); }
        if self.get_meta(path)?.is_some() { return Ok(true); }
        let s = self.store.blob_exists(&Self::data_key(path)).map_err(serr)?;
        if collect_bool(s).unwrap_or(false) { return Ok(true); }
        let s = self.store.blob_exists(&Self::meta_key(path)).map_err(serr)?;
        Ok(collect_bool(s).unwrap_or(false))
    }
    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> {
        if let Some(mut m) = self.get_meta(path)? { m.permissions = mode; self.store_meta(path, &m) }
        else { Err(VfsError::Backend { message: format!("not found: {path}") }.into()) }
    }
    fn symlink(&self, _t: &str, _l: &str) -> VfsResult<()> { Err(VfsError::Backend { message: "unsupported".into() }.into()) }
    fn readlink(&self, _p: &str) -> VfsResult<String> { Err(VfsError::Backend { message: "unsupported".into() }.into()) }
    fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        let s = self.store.get_blob(&Self::data_key(from)).map_err(serr)?; let d: Option<Vec<u8>> = collect_blob(s)?;
        if let Some(d) = d { let s = self.store.put_blob(&Self::data_key(to), &d).map_err(serr)?; drain_ok(s)?; }
        if let Some(m) = self.get_meta(from)? { self.store_meta(to, &m)?; }
        let s = self.store.delete_blob(&Self::data_key(from)).map_err(serr)?; drain_ok(s)?;
        let s = self.store.delete_blob(&Self::meta_key(from)).map_err(serr)?; drain_ok(s)?;
        Ok(())
    }
    fn remove(&self, path: &str) -> VfsResult<()> {
        let s = self.store.put_blob(&Self::whiteout_key(path), b"1").map_err(serr)?; drain_ok(s)?;
        Ok(())
    }
    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> {
        let s = self.store.get_blob(&Self::data_key(path)).map_err(serr)?;
        let d: Option<Vec<u8>> = collect_blob(s)?;
        d.ok_or_else(|| VfsError::Backend { message: format!("not found: {path}") }.into())
    }
    fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()> {
        let s = self.store.put_blob(&Self::data_key(path), data).map_err(serr)?; drain_ok(s)?;
        let mut m = self.get_meta(path)?.unwrap_or_else(|| new_meta(path, data.len() as u64, VfsFileType::Regular));
        m.size = data.len() as u64; m.modified = Some(std::time::SystemTime::now());
        self.store_meta(path, &m)
    }
    fn inode(&self, path: &str) -> VfsResult<u64> { Ok(path_hash(path)) }
    fn path_by_inode(&self, _ino: u64) -> VfsResult<String> { Err(VfsError::Backend { message: "unsupported".into() }.into()) }
    fn stat_by_inode(&self, _ino: u64) -> VfsResult<VfsMetadata> { Err(VfsError::Backend { message: "unsupported".into() }.into()) }
    fn copy(&self, from: &str, to: &str) -> VfsResult<()> {
        let s = self.store.get_blob(&Self::data_key(from)).map_err(serr)?; let d: Option<Vec<u8>> = collect_blob(s)?;
        if let Some(d) = d { let s = self.store.put_blob(&Self::data_key(to), &d).map_err(serr)?; drain_ok(s)?; }
        if let Some(m) = self.get_meta(from)? { self.store_meta(to, &m)?; }
        Ok(())
    }
    fn remove_all(&self, path: &str) -> VfsResult<()> { self.remove(path) }
    fn mkdir_all(&self, path: &str) -> VfsResult<()> {
        let mut cur = String::new();
        for p in path.trim_start_matches('/').split('/') {
            if p.is_empty() { continue; }
            if cur.is_empty() { cur = format!("/{p}"); } else { cur = format!("{cur}/{p}"); }
            if self.get_meta(&cur)?.is_none() { let _ = self.mkdir(&cur); }
        }
        Ok(())
    }
}

impl DeltaStore for R2Delta {
    fn add_whiteout(&self, path: &str, _version: u64) -> VfsResult<()> { self.remove(path) }
    fn is_whiteout(&self, path: &str) -> VfsResult<Option<u64>> { if whiteout_exists(self, path)? { Ok(Some(0)) } else { Ok(None) } }
    fn remove_whiteout(&self, path: &str) -> VfsResult<()> {
        let s = self.store.delete_blob(&Self::whiteout_key(path)).map_err(serr)?; drain_ok(s)?; Ok(())
    }
    fn list_whiteouts(&self, _dir: &str) -> VfsResult<Vec<(String, u64)>> { Ok(Vec::new()) }
    fn flush(&self) -> VfsResult<()> { Ok(()) }
    fn reset(&self) -> VfsResult<()> { Err(VfsError::Backend { message: "unsupported".into() }.into()) }
}

fn new_meta(path: &str, size: u64, ft: VfsFileType) -> VfsMetadata {
    VfsMetadata { inode: path_hash(path), size, file_type: ft, permissions: if ft == VfsFileType::Directory { 0o755 } else { 0o644 }, owner: (0, 0), created: Some(std::time::SystemTime::now()), modified: Some(std::time::SystemTime::now()), accessed: Some(std::time::SystemTime::now()), checksum: crate::shared::vfs::types::Checksum::None, version: 0, state: VfsEntryState::Ready }
}
fn path_hash(path: &str) -> u64 { use std::collections::hash_map::DefaultHasher; use std::hash::{Hash, Hasher}; let mut h = DefaultHasher::new(); path.hash(&mut h); h.finish() }
fn whiteout_exists(d: &R2Delta, p: &str) -> VfsResult<bool> {
    let s = d.store.blob_exists(&R2Delta::whiteout_key(p)).map_err(serr)?;
    Ok(collect_bool(s).unwrap_or(false))
}
fn drain_ok(stream: impl Iterator<Item = foundation_core::valtron::Stream<Result<(), foundation_db::StorageError>, ()>>) -> VfsResult<()> {
    match collect_one(stream) {
        Some(Ok(())) => Ok(()),
        Some(Err(e)) => Err(serr(e).into()),
        None => Ok(()),
    }
}
fn collect_bool(stream: impl Iterator<Item = foundation_core::valtron::Stream<Result<bool, foundation_db::StorageError>, ()>>) -> VfsResult<bool> {
    match collect_one(stream) {
        Some(Ok(b)) => Ok(b),
        Some(Err(e)) => Err(serr(e).into()),
        None => Ok(false),
    }
}
fn collect_blob(stream: impl Iterator<Item = foundation_core::valtron::Stream<Result<Option<Vec<u8>>, foundation_db::StorageError>, ()>>) -> VfsResult<Option<Vec<u8>>> {
    match collect_one(stream) {
        Some(Ok(data)) => Ok(data),
        Some(Err(e)) => Err(serr(e).into()),
        None => Ok(None),
    }
}
fn serr(e: foundation_db::StorageError) -> VfsError { VfsError::Backend { message: e.to_string() } }
