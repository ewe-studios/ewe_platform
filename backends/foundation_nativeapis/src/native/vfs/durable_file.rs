//! A file-backed Durable Streams virtual file system (VFS).
//!
//! Each stream is one directory named by the hexadecimal form of its name.
//! The directory holds an append-only `data` log and a `meta` file. A metadata
//! write goes to a temporary file and is renamed over `meta` after the data
//! write is flushed, so a crash leaves the old metadata with the old data.
//!
//! The owner chose this metadata file on 2026-11-27 in place of a Lightweight
//! Memory-Mapped Database (LMDB): the workspace has no LMDB dependency.

use foundation_core::durable::{
    ContentType, Expiry, ForkInfo, Notifier, NotifierRef, ProducerEpochState, ProducerId,
    StreamName, StreamSeq, VfsEntry, VfsError, VfsFuture, VfsMeta, VfsOffset, WriterState,
};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The default deepest fork chain a read follows. The memory VFS uses the
/// same limit.
pub const DEFAULT_MAX_FORK_DEPTH: u32 = 64;

/// One notifier for a stream.
#[derive(Default)]
struct FileNotifier {
    callbacks: Mutex<Vec<Box<dyn Fn() + Send + 'static>>>,
}

impl Notifier for FileNotifier {
    fn register(&self, cb: Box<dyn Fn() + Send + 'static>) {
        self.callbacks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(cb);
    }

    fn notify(&self) {
        let callbacks = std::mem::take(
            &mut *self
                .callbacks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        for callback in callbacks {
            callback();
        }
    }
}

/// A file-backed VFS.
pub struct FileVfs {
    root: PathBuf,
    max_fork_depth: u32,
    lock: Mutex<()>,
    notifiers: Mutex<HashMap<StreamName, Arc<FileNotifier>>>,
}

impl FileVfs {
    /// Open or create the VFS at `root`.
    ///
    /// # Errors
    /// Returns an input/output error when the directory cannot be created.
    pub fn new(root: impl AsRef<Path>) -> std::io::Result<Self> {
        Self::with_max_fork_depth(root, DEFAULT_MAX_FORK_DEPTH)
    }

    /// Open or create the VFS at `root` with a fork-chain depth limit.
    ///
    /// A read stops with [`VfsError::Conflict`] when the chain is deeper than
    /// `max_fork_depth`. The default is [`DEFAULT_MAX_FORK_DEPTH`].
    ///
    /// # Errors
    /// Returns an input/output error when the directory cannot be created.
    pub fn with_max_fork_depth(
        root: impl AsRef<Path>,
        max_fork_depth: u32,
    ) -> std::io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            max_fork_depth,
            lock: Mutex::new(()),
            notifiers: Mutex::new(HashMap::new()),
        })
    }

    /// Return the fork-chain depth limit.
    #[must_use]
    pub fn max_fork_depth(&self) -> u32 {
        self.max_fork_depth
    }

    /// Notify every waiter for a stream.
    pub fn notify(&self, name: &StreamName) {
        self.notifier(name).notify();
    }

    fn dir(&self, name: &StreamName) -> PathBuf {
        self.root.join(hex(name.as_str().as_bytes()))
    }

    fn notifier(&self, name: &StreamName) -> Arc<FileNotifier> {
        self.notifiers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(name.clone())
            .or_default()
            .clone()
    }

    fn forget_notifier(&self, name: &StreamName) {
        self.notifiers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(name);
    }

    fn read_meta(&self, name: &StreamName) -> Result<VfsMeta, VfsError> {
        let dir = self.dir(name);
        if !dir.is_dir() {
            return Err(VfsError::NotFound);
        }
        let stored = self.read_stored(name)?;
        if stored.soft_deleted {
            return Err(VfsError::Gone);
        }
        Ok(stored.meta())
    }

    fn write_meta(&self, name: &StreamName, stored: &StoredMeta) -> Result<(), VfsError> {
        let dir = self.dir(name);
        let temp = dir.join("meta.tmp");
        let target = dir.join("meta");
        let mut file = fs::File::create(&temp).map_err(io_error)?;
        file.write_all(stored.render().as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(io_error)?;
        fs::rename(&temp, &target).map_err(io_error)
    }

    /// Read the stored metadata. A missing entry returns [`VfsError::NotFound`].
    fn read_stored(&self, name: &StreamName) -> Result<StoredMeta, VfsError> {
        let text = fs::read_to_string(self.dir(name).join("meta")).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                VfsError::NotFound
            } else {
                VfsError::Io(Box::new(error))
            }
        })?;
        StoredMeta::parse(&text)
    }

    /// Append bytes to the stream's own log and return its own length.
    ///
    /// The length counts only the bytes this stream wrote, not the inherited
    /// fork prefix. A caller adds the fork base to get the stream length.
    fn append_own(&self, name: &StreamName, bytes: &[u8]) -> Result<u64, VfsError> {
        let path = self.dir(name).join("data");
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(io_error)?;
        file.write_all(bytes)
            .and_then(|()| file.sync_data())
            .map_err(io_error)?;
        file.metadata().map(|meta| meta.len()).map_err(io_error)
    }

    /// Walk the fork chain and return the bytes at `at`.
    ///
    /// The chain reads a soft-deleted source on purpose. A living fork depends
    /// on that data.
    fn read_chain(
        &self,
        name: &StreamName,
        at: u64,
        limit: usize,
        depth: u32,
    ) -> Result<Vec<u8>, VfsError> {
        if depth > self.max_fork_depth {
            return Err(VfsError::Conflict);
        }
        let stored = self.read_stored(name)?;
        let base = base_len(&stored);
        let mut out = Vec::new();
        if at < base && limit > 0 {
            let take = usize::try_from(base - at).unwrap_or(usize::MAX).min(limit);
            if let Some(fork) = &stored.fork {
                out.extend_from_slice(&self.read_chain(&fork.source, at, take, depth + 1)?);
            }
        }
        if out.len() < limit {
            let own_at = at.saturating_sub(base);
            // The own log holds `len - base` bytes. Bound the buffer by the
            // bytes that remain, so a caller that passes `usize::MAX` as the
            // limit (the core does when no page limit is set) does not ask
            // for an impossible allocation.
            let own_len = stored.len.saturating_sub(base);
            let available = own_len.saturating_sub(own_at);
            let want = (limit - out.len()).min(usize::try_from(available).unwrap_or(usize::MAX));
            let path = self.dir(name).join("data");
            let mut file = fs::File::open(path).map_err(io_error)?;
            file.seek(SeekFrom::Start(own_at)).map_err(io_error)?;
            let mut buffer = vec![0_u8; want];
            let read = file.read(&mut buffer).map_err(io_error)?;
            buffer.truncate(read);
            out.extend_from_slice(&buffer);
        }
        Ok(out)
    }

    /// Remove every soft-deleted entry whose reference count reached zero.
    ///
    /// One pass can free a source that a removed fork pointed to, so repeat
    /// until no candidate remains.
    fn cascade(&self) {
        loop {
            let Ok(entries) = fs::read_dir(&self.root) else {
                return;
            };
            let mut candidates = Vec::new();
            for entry in entries.flatten() {
                let Ok(text) = fs::read_to_string(entry.path().join("meta")) else {
                    continue;
                };
                let Ok(stored) = StoredMeta::parse(&text) else {
                    continue;
                };
                if stored.soft_deleted && stored.ref_count == 0 {
                    candidates.push((entry.path(), stored));
                }
            }
            if candidates.is_empty() {
                return;
            }
            let mut removed = false;
            for (path, stored) in candidates {
                if fs::remove_dir_all(&path).is_err() {
                    continue;
                }
                removed = true;
                if let Some(fork) = stored.fork {
                    if let Ok(mut source) = self.read_stored(&fork.source) {
                        source.ref_count = source.ref_count.saturating_sub(1);
                        let _ = self.write_meta(&fork.source, &source);
                    }
                }
            }
            if !removed {
                return;
            }
        }
    }
}

fn io_error(error: std::io::Error) -> VfsError {
    VfsError::Io(Box::new(error))
}

/// The number of bytes a fork inherits from its source.
fn base_len(stored: &StoredMeta) -> u64 {
    stored.fork.as_ref().map_or(0, |fork| fork.at)
}

fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        text.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    text
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.as_chunks::<2>().0 {
        let high = (chunk[0] as char).to_digit(16)?;
        let low = (chunk[1] as char).to_digit(16)?;
        out.push(u8::try_from((high << 4) | low).ok()?);
    }
    Some(out)
}

#[derive(Clone)]
struct StoredFork {
    source: StreamName,
    at: u64,
}

/// The on-disk metadata record.
#[derive(Clone, Default)]
struct StoredMeta {
    content_type: Option<ContentType>,
    len: u64,
    closed: bool,
    expires: Option<Expiry>,
    last_access_ms: i64,
    trim_from: u64,
    version: u64,
    ref_count: u32,
    soft_deleted: bool,
    fork: Option<StoredFork>,
    producers: BTreeMap<ProducerId, ProducerEpochState>,
    stream_seq: Option<StreamSeq>,
}

impl StoredMeta {
    fn meta(&self) -> VfsMeta {
        VfsMeta {
            content_type: self.content_type.clone().unwrap_or_default(),
            len: self.len,
            closed: self.closed,
            expires: self.expires.clone(),
            writer: WriterState {
                stream_seq: self.stream_seq.clone(),
                producers: self.producers.clone(),
            },
            fork: self.fork.as_ref().map(|fork| ForkInfo {
                source: fork.source.clone(),
                at: fork.at,
            }),
            last_access_ms: self.last_access_ms,
            trim_from: self.trim_from,
            version: self.version,
        }
    }

    fn from_meta(meta: &VfsMeta) -> Self {
        Self {
            content_type: Some(meta.content_type.clone()),
            len: meta.len,
            closed: meta.closed,
            expires: meta.expires.clone(),
            last_access_ms: meta.last_access_ms,
            trim_from: meta.trim_from,
            version: meta.version,
            ref_count: 0,
            soft_deleted: false,
            fork: meta.fork.as_ref().map(|fork| StoredFork {
                source: fork.source.clone(),
                at: fork.at,
            }),
            producers: meta.writer.producers.clone(),
            stream_seq: meta.writer.stream_seq.clone(),
        }
    }

    fn render(&self) -> String {
        use std::fmt::Write as _;

        let mut out = String::new();
        let expires = match &self.expires {
            None => "none".to_string(),
            Some(Expiry::Ttl(ttl)) => format!("ttl:{}", ttl.as_millis()),
            Some(Expiry::Absolute(at)) => format!("abs:{at}"),
        };
        let _ = write!(
            out,
            "content_type\t{}\nlen\t{}\nclosed\t{}\nexpires\t{}\nlast_access_ms\t{}\ntrim_from\t{}\nversion\t{}\nref_count\t{}\nsoft_deleted\t{}\n",
            self.content_type.as_ref().map_or("", ContentType::as_str),
            self.len,
            u8::from(self.closed),
            expires,
            self.last_access_ms,
            self.trim_from,
            self.version,
            self.ref_count,
            u8::from(self.soft_deleted),
        );
        if let Some(fork) = &self.fork {
            let _ = writeln!(
                out,
                "fork\t{}\t{}",
                hex(fork.source.as_str().as_bytes()),
                fork.at
            );
        }
        for (producer, state) in &self.producers {
            let _ = writeln!(
                out,
                "producer\t{}\t{}\t{}",
                hex(producer.as_str().as_bytes()),
                state.epoch,
                state.last_seq
            );
        }
        if let Some(seq) = &self.stream_seq {
            let _ = writeln!(out, "stream_seq\t{}", hex(seq.as_bytes()));
        }
        out
    }

    fn parse(text: &str) -> Result<Self, VfsError> {
        let mut stored = Self::default();
        for line in text.lines() {
            let mut parts = line.split('\t');
            let Some(key) = parts.next() else { continue };
            let bad = || VfsError::Io(Box::new(std::io::Error::other("bad metadata")));
            match key {
                "content_type" => {
                    let value = parts.next().ok_or_else(bad)?;
                    if !value.is_empty() {
                        stored.content_type = Some(ContentType::parse(value).map_err(|_| bad())?);
                    }
                }
                "len" => stored.len = parts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?,
                "closed" => stored.closed = parts.next() == Some("1"),
                "expires" => {
                    let value = parts.next().unwrap_or("none");
                    stored.expires = if value == "none" {
                        None
                    } else if let Some(ms) = value.strip_prefix("ttl:") {
                        Some(Expiry::Ttl(core::time::Duration::from_millis(
                            ms.parse().map_err(|_| bad())?,
                        )))
                    } else if let Some(at) = value.strip_prefix("abs:") {
                        Some(Expiry::Absolute(at.parse().map_err(|_| bad())?))
                    } else {
                        return Err(bad());
                    };
                }
                "last_access_ms" => {
                    stored.last_access_ms =
                        parts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
                }
                "trim_from" => {
                    stored.trim_from = parts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
                }
                "version" => {
                    stored.version = parts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
                }
                "ref_count" => {
                    stored.ref_count = parts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
                }
                "soft_deleted" => stored.soft_deleted = parts.next() == Some("1"),
                "fork" => {
                    let source = parts.next().ok_or_else(bad)?;
                    let at = parts.next().ok_or_else(bad)?;
                    let bytes = unhex(source).ok_or_else(bad)?;
                    let text = String::from_utf8(bytes).map_err(|_| bad())?;
                    stored.fork = Some(StoredFork {
                        source: StreamName::parse(&text).map_err(|_| bad())?,
                        at: at.parse().map_err(|_| bad())?,
                    });
                }
                "producer" => {
                    let id = parts.next().ok_or_else(bad)?;
                    let epoch = parts.next().ok_or_else(bad)?;
                    let last_seq = parts.next().ok_or_else(bad)?;
                    let bytes = unhex(id).ok_or_else(bad)?;
                    let text = String::from_utf8(bytes).map_err(|_| bad())?;
                    stored.producers.insert(
                        ProducerId::parse(&text).map_err(|_| bad())?,
                        ProducerEpochState {
                            epoch: epoch.parse().map_err(|_| bad())?,
                            last_seq: last_seq.parse().map_err(|_| bad())?,
                        },
                    );
                }
                "stream_seq" => {
                    let value = parts.next().ok_or_else(bad)?;
                    stored.stream_seq = Some(StreamSeq::from_bytes(unhex(value).ok_or_else(bad)?));
                }
                _ => {}
            }
        }
        Ok(stored)
    }
}

impl foundation_core::durable::DurableStreamVfs for FileVfs {
    fn create(&self, name: &StreamName, meta: VfsMeta) -> VfsFuture<'_, bool> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = (|| {
            let dir = self.dir(name);
            if dir.is_dir() {
                match self.read_stored(name) {
                    Ok(existing) => {
                        if existing.soft_deleted {
                            return Err(VfsError::Gone);
                        }
                        if existing.content_type.as_ref() != Some(&meta.content_type)
                            || existing.closed != meta.closed
                            || existing.expires != meta.expires
                            || existing.fork.as_ref().map(|f| (&f.source, f.at))
                                != meta.fork.as_ref().map(|f| (&f.source, f.at))
                        {
                            return Err(VfsError::Conflict);
                        }
                        return Ok(false);
                    }
                    // A crash can leave the directory before its metadata. Rebuild it.
                    Err(VfsError::NotFound) => {
                        fs::remove_dir_all(&dir).map_err(io_error)?;
                    }
                    Err(error) => return Err(error),
                }
            }
            let source = match meta.fork.as_ref() {
                Some(fork) => {
                    let stored = self.read_stored(&fork.source)?;
                    if stored.soft_deleted {
                        return Err(VfsError::Gone);
                    }
                    if meta.len > stored.len {
                        return Err(VfsError::Conflict);
                    }
                    Some(stored)
                }
                None => None,
            };
            let mut stored = StoredMeta::from_meta(&meta);
            stored.version = 0;
            fs::create_dir_all(&dir).map_err(io_error)?;
            fs::write(dir.join("data"), []).map_err(io_error)?;
            if let Err(error) = self.write_meta(name, &stored) {
                let _ = fs::remove_dir_all(&dir);
                return Err(error);
            }
            if let (Some(mut source), Some(fork)) = (source, meta.fork.as_ref()) {
                source.ref_count = source.ref_count.saturating_add(1);
                if let Err(error) = self.write_meta(&fork.source, &source) {
                    let _ = fs::remove_dir_all(&dir);
                    return Err(error);
                }
            }
            Ok(true)
        })();
        Box::pin(async move { result })
    }

    fn append(&self, name: &StreamName, bytes: &[u8]) -> VfsFuture<'_, VfsOffset> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = (|| {
            let mut stored = self.read_stored(name)?;
            if stored.soft_deleted {
                return Err(VfsError::Gone);
            }
            if stored.closed {
                return Err(VfsError::Conflict);
            }
            let own = self.append_own(name, bytes)?;
            stored.len = base_len(&stored).saturating_add(own);
            stored.version = stored.version.saturating_add(1);
            self.write_meta(name, &stored)?;
            Ok(VfsOffset(stored.len))
        })();
        if result.is_ok() {
            self.notify(name);
        }
        Box::pin(async move { result })
    }

    fn append_with_meta(
        &self,
        name: &StreamName,
        bytes: &[u8],
        meta: VfsMeta,
        expected_version: u64,
    ) -> VfsFuture<'_, VfsOffset> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = (|| {
            let stored = self.read_stored(name)?;
            if stored.soft_deleted {
                return Err(VfsError::Gone);
            }
            if stored.version != expected_version || stored.closed {
                return Err(VfsError::Conflict);
            }
            let own = self.append_own(name, bytes)?;
            let mut next = StoredMeta::from_meta(&meta);
            next.len = base_len(&stored).saturating_add(own);
            next.version = expected_version.saturating_add(1);
            next.ref_count = stored.ref_count;
            next.soft_deleted = false;
            next.fork.clone_from(&stored.fork);
            next.trim_from = stored.trim_from;
            self.write_meta(name, &next)?;
            Ok(VfsOffset(next.len))
        })();
        if result.is_ok() {
            self.notify(name);
        }
        Box::pin(async move { result })
    }

    fn read_at(&self, name: &StreamName, at: &VfsOffset, limit: usize) -> VfsFuture<'_, Vec<u8>> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = (|| {
            let stored = self.read_stored(name)?;
            if stored.soft_deleted {
                return Err(VfsError::Gone);
            }
            if at.0 < stored.trim_from {
                return Err(VfsError::Trimmed);
            }
            self.read_chain(name, at.0, limit, 0)
        })();
        Box::pin(async move { result })
    }

    fn meta(&self, name: &StreamName) -> VfsFuture<'_, VfsMeta> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = self.read_meta(name);
        Box::pin(async move { result })
    }

    fn touch(&self, name: &StreamName, now_ms: i64) -> VfsFuture<'_, ()> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = (|| {
            let mut stored = self.read_stored(name)?;
            if stored.soft_deleted {
                return Err(VfsError::Gone);
            }
            stored.last_access_ms = now_ms;
            self.write_meta(name, &stored)
        })();
        Box::pin(async move { result })
    }

    fn trim(&self, name: &StreamName, at: VfsOffset) -> VfsFuture<'_, ()> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = (|| {
            let mut stored = self.read_stored(name)?;
            if stored.soft_deleted {
                return Err(VfsError::Gone);
            }
            stored.trim_from = at.0.min(stored.len).max(stored.trim_from);
            stored.version = stored.version.saturating_add(1);
            self.write_meta(name, &stored)
        })();
        Box::pin(async move { result })
    }

    fn close(&self, name: &StreamName) -> VfsFuture<'_, ()> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = (|| {
            let mut stored = self.read_stored(name)?;
            if stored.soft_deleted {
                return Err(VfsError::Gone);
            }
            stored.closed = true;
            stored.version = stored.version.saturating_add(1);
            self.write_meta(name, &stored)
        })();
        if result.is_ok() {
            self.notify(name);
        }
        Box::pin(async move { result })
    }

    fn remove(&self, name: &StreamName) -> VfsFuture<'_, bool> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = (|| {
            let dir = self.dir(name);
            if !dir.is_dir() {
                return Err(VfsError::NotFound);
            }
            let mut stored = self.read_stored(name)?;
            if stored.soft_deleted {
                return Err(VfsError::Gone);
            }
            let soft = stored.ref_count > 0;
            if soft {
                stored.soft_deleted = true;
                stored.version = stored.version.saturating_add(1);
                self.write_meta(name, &stored)?;
            } else {
                let fork = stored.fork.clone();
                fs::remove_dir_all(&dir).map_err(io_error)?;
                if let Some(fork) = fork {
                    if let Ok(mut source) = self.read_stored(&fork.source) {
                        source.ref_count = source.ref_count.saturating_sub(1);
                        let _ = self.write_meta(&fork.source, &source);
                    }
                }
            }
            self.cascade();
            Ok(soft)
        })();
        if result.is_ok() {
            if result.as_ref().is_ok_and(|soft| *soft) {
                self.notify(name);
            } else {
                self.forget_notifier(name);
            }
        }
        Box::pin(async move { result })
    }

    fn list(&self, prefix: &StreamName) -> VfsFuture<'_, Vec<VfsEntry>> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = (|| {
            let mut entries = Vec::new();
            let Ok(dir) = fs::read_dir(&self.root) else {
                return Ok(entries);
            };
            for entry in dir.flatten() {
                let Some(name_hex) = entry.file_name().to_str().map(str::to_string) else {
                    continue;
                };
                let Some(bytes) = unhex(&name_hex) else {
                    continue;
                };
                let Ok(text) = String::from_utf8(bytes) else {
                    continue;
                };
                let Ok(name) = StreamName::parse(&text) else {
                    continue;
                };
                if !name.as_str().starts_with(prefix.as_str()) {
                    continue;
                }
                if let Ok(meta) = self.read_meta(&name) {
                    entries.push(VfsEntry { name, meta });
                }
            }
            Ok(entries)
        })();
        Box::pin(async move { result })
    }

    fn notifier(&self, name: &StreamName) -> Option<NotifierRef> {
        Some(self.notifier(name))
    }
}
