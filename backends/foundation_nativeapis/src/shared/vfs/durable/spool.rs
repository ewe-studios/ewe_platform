//! The local spool.
//!
//! An append writes to the spool first and flushes it to the disk before the
//! virtual file system (VFS) returns `Accepted`. The batcher later reads the
//! spool, puts one object per batch and publishes the head. The spool is the
//! only place that holds bytes that are not yet in the object store, so a
//! crash loses no acknowledged byte.
//!
//! # Directory layout
//!
//! | File | Holds |
//! |---|---|
//! | `<spool>/<stream hex>/pending` | bytes appended since the last publish |
//! | `<spool>/<stream hex>/state` | append count and the last access time |
//! | `<spool>/<stream hex>/overlay` | the metadata record of the last append |
//! | `<spool>/<stream hex>/inflight` | the keys of a batch whose publish may have landed |
//!
//! A head manifest overlay uses the same format as the head
//! ([`super::head`]). The pending length always comes from the file length, so
//! a crash between the data write and the state write cannot lose bytes.

use super::head::HeadManifest;
use core::fmt::Write as _;
use foundation_core::durable::{StreamName, VfsMeta};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// A stream's unpublished state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpoolState {
    /// The number of appends since the last publish.
    pub appends: u64,
    /// The first append time in UNIX milliseconds, or `0` when no byte waits.
    pub first_appended_ms: i64,
    /// The last access time set by a touch or an append, or `None`.
    pub last_access_ms: Option<i64>,
}

/// A batch whose publish may have reached the head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InflightBatch {
    /// The published snapshot length.
    pub snapshot_len: u64,
    /// The number of appends in the snapshot.
    pub appends: u64,
    /// The object keys and lengths of the batch, in stream order.
    pub blocks: Vec<(u64, String)>,
}

/// The local spool.
#[derive(Debug)]
pub struct Spool {
    root: PathBuf,
}

impl Spool {
    /// Open or create the spool under `root`.
    ///
    /// # Errors
    ///
    /// Returns an input/output error when the directory cannot be created.
    pub fn new(root: impl AsRef<Path>) -> std::io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    /// Return the spool root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn dir(&self, name: &StreamName) -> PathBuf {
        self.root.join(hex(name.as_str().as_bytes()))
    }

    /// Return the number of pending bytes.
    ///
    /// # Errors
    ///
    /// Returns an input/output error when the pending file cannot be read.
    pub fn pending_len(&self, name: &StreamName) -> std::io::Result<u64> {
        match fs::metadata(self.dir(name).join("pending")) {
            Ok(metadata) => Ok(metadata.len()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(error) => Err(error),
        }
    }

    /// Read the spool state. A stream with no spool returns the default.
    ///
    /// # Errors
    ///
    /// Returns an input/output error when the state file is malformed.
    pub fn load(&self, name: &StreamName) -> std::io::Result<SpoolState> {
        let path = self.dir(name).join("state");
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(SpoolState::default());
            }
            Err(error) => return Err(error),
        };
        let mut state = SpoolState::default();
        for line in text.lines() {
            let mut parts = line.split('\t');
            let Some(key) = parts.next() else { continue };
            let value = parts.next();
            match key {
                "appends" => {
                    state.appends = value
                        .and_then(|value| value.parse().ok())
                        .ok_or_else(|| bad_state("appends"))?;
                }
                "first_appended_ms" => {
                    state.first_appended_ms = value
                        .and_then(|value| value.parse().ok())
                        .ok_or_else(|| bad_state("first_appended_ms"))?;
                }
                "last_access_ms" => {
                    state.last_access_ms = Some(
                        value
                            .and_then(|value| value.parse().ok())
                            .ok_or_else(|| bad_state("last_access_ms"))?,
                    );
                }
                _ => {}
            }
        }
        Ok(state)
    }

    /// Save the spool state.
    ///
    /// # Errors
    ///
    /// Returns an input/output error when the state cannot be written.
    pub fn save(&self, name: &StreamName, state: &SpoolState) -> std::io::Result<()> {
        let dir = self.dir(name);
        fs::create_dir_all(&dir)?;
        let mut text = format!(
            "appends\t{}\nfirst_appended_ms\t{}\n",
            state.appends, state.first_appended_ms
        );
        if let Some(last_access) = state.last_access_ms {
            let _ = writeln!(text, "last_access_ms\t{last_access}");
        }
        write_atomic(&dir.join("state"), text.as_bytes())
    }

    /// Append bytes for one append and store the metadata overlay.
    ///
    /// # Errors
    ///
    /// Returns an input/output error when the data or the state cannot be
    /// written.
    pub fn append(
        &self,
        name: &StreamName,
        bytes: &[u8],
        now_ms: i64,
        overlay: Option<&VfsMeta>,
    ) -> std::io::Result<SpoolState> {
        let dir = self.dir(name);
        fs::create_dir_all(&dir)?;
        let mut state = self.load(name)?;
        let was_empty = self.pending_len(name)? == 0;
        if !bytes.is_empty() {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join("pending"))?;
            file.write_all(bytes)?;
            file.sync_data()?;
        }
        state.appends = state.appends.saturating_add(1);
        if was_empty {
            state.first_appended_ms = now_ms;
        }
        if let Some(meta) = overlay {
            let manifest = HeadManifest::from_meta(meta);
            let text = manifest.render();
            write_atomic(&dir.join("overlay"), text.as_bytes())?;
            state.last_access_ms = Some(meta.last_access_ms);
        }
        self.save(name, &state)?;
        Ok(state)
    }

    /// Read a range of the pending bytes.
    ///
    /// # Errors
    ///
    /// Returns an input/output error when the pending file cannot be read.
    pub fn read_range(
        &self,
        name: &StreamName,
        start: u64,
        len: usize,
    ) -> std::io::Result<Vec<u8>> {
        let path = self.dir(name).join("pending");
        let mut file = match fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let file_len = file.metadata()?.len();
        if start >= file_len {
            return Ok(Vec::new());
        }
        let available = file_len.saturating_sub(start);
        let want = u64::try_from(len).unwrap_or(u64::MAX).min(available);
        let want = usize::try_from(want).unwrap_or(usize::MAX);
        file.seek(SeekFrom::Start(start))?;
        let mut buffer = vec![0_u8; want];
        let read = file.read(&mut buffer)?;
        buffer.truncate(read);
        Ok(buffer)
    }

    /// Remove the first `len` bytes of the pending data.
    ///
    /// # Errors
    ///
    /// Returns an input/output error when the pending file cannot be rewritten.
    pub fn consume(&self, name: &StreamName, len: u64) -> std::io::Result<()> {
        if len == 0 {
            return Ok(());
        }
        let dir = self.dir(name);
        let remaining = self.read_range(name, len, usize::MAX)?;
        if remaining.is_empty() {
            match fs::remove_file(dir.join("pending")) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        } else {
            write_atomic(&dir.join("pending"), &remaining)?;
        }
        Ok(())
    }

    /// Record the appends that the head now holds and clear the overlay.
    ///
    /// # Errors
    ///
    /// Returns an input/output error when the state cannot be written.
    pub fn settle(
        &self,
        name: &StreamName,
        published_appends: u64,
        clear_overlay: bool,
    ) -> std::io::Result<SpoolState> {
        let mut state = self.load(name)?;
        state.appends = state.appends.saturating_sub(published_appends);
        if self.pending_len(name)? == 0 {
            state.first_appended_ms = 0;
            state.last_access_ms = None;
        }
        if clear_overlay {
            let path = self.dir(name).join("overlay");
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        self.save(name, &state)?;
        Ok(state)
    }

    /// Update the last access time in the pending state.
    ///
    /// # Errors
    ///
    /// Returns an input/output error when the state cannot be written.
    pub fn touch(&self, name: &StreamName, now_ms: i64) -> std::io::Result<SpoolState> {
        let mut state = self.load(name)?;
        state.last_access_ms = Some(now_ms);
        self.save(name, &state)?;
        Ok(state)
    }

    /// Read the metadata overlay.
    ///
    /// # Errors
    ///
    /// Returns an input/output error when the overlay is malformed.
    pub fn overlay(&self, name: &StreamName) -> std::io::Result<Option<VfsMeta>> {
        let path = self.dir(name).join("overlay");
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let manifest =
            HeadManifest::parse(&text).map_err(|error| std::io::Error::other(error.to_string()))?;
        Ok(Some(manifest.to_meta()))
    }

    /// Record a batch whose publish may land.
    ///
    /// # Errors
    ///
    /// Returns an input/output error when the record cannot be written.
    pub fn set_inflight(
        &self,
        name: &StreamName,
        snapshot_len: u64,
        appends: u64,
        blocks: &[(u64, String)],
    ) -> std::io::Result<()> {
        let dir = self.dir(name);
        fs::create_dir_all(&dir)?;
        let mut text = format!("{snapshot_len}\n{appends}\n");
        for (len, key) in blocks {
            let _ = writeln!(text, "{len}\t{key}");
        }
        write_atomic(&dir.join("inflight"), text.as_bytes())
    }

    /// Read the in-flight batch record.
    ///
    /// # Errors
    ///
    /// Returns an input/output error when the record is malformed.
    pub fn inflight(&self, name: &StreamName) -> std::io::Result<Option<InflightBatch>> {
        let path = self.dir(name).join("inflight");
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut lines = text.lines();
        let snapshot_len = lines
            .next()
            .and_then(|line| line.parse().ok())
            .ok_or_else(|| bad_state("inflight length"))?;
        let appends = lines
            .next()
            .and_then(|line| line.parse().ok())
            .ok_or_else(|| bad_state("inflight appends"))?;
        let mut blocks = Vec::new();
        for line in lines {
            if line.is_empty() {
                continue;
            }
            let mut parts = line.split('\t');
            let len = parts
                .next()
                .and_then(|value| value.parse().ok())
                .ok_or_else(|| bad_state("inflight block length"))?;
            let key = parts
                .next()
                .ok_or_else(|| bad_state("inflight block key"))?;
            blocks.push((len, key.to_string()));
        }
        Ok(Some(InflightBatch {
            snapshot_len,
            appends,
            blocks,
        }))
    }

    /// Remove the in-flight record.
    ///
    /// # Errors
    ///
    /// Returns an input/output error when the record cannot be removed.
    pub fn clear_inflight(&self, name: &StreamName) -> std::io::Result<()> {
        match fs::remove_file(self.dir(name).join("inflight")) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// List the streams with pending bytes.
    ///
    /// # Errors
    ///
    /// Returns an input/output error when the spool directory cannot be read.
    pub fn streams(&self) -> std::io::Result<Vec<StreamName>> {
        let mut out = Vec::new();
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(error) => return Err(error),
        };
        for entry in entries.flatten() {
            let Some(name_hex) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let Some(bytes) = unhex(&name_hex) else {
                continue;
            };
            let Ok(text) = String::from_utf8(bytes) else {
                continue;
            };
            if let Ok(name) = StreamName::parse(&text) {
                out.push(name);
            }
        }
        Ok(out)
    }

    /// Remove a stream's spool directory.
    ///
    /// # Errors
    ///
    /// Returns an input/output error when the directory cannot be removed.
    pub fn remove(&self, name: &StreamName) -> std::io::Result<()> {
        match fs::remove_dir_all(self.dir(name)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

fn bad_state(field: &str) -> std::io::Error {
    std::io::Error::other(format!("bad spool state field: {field}"))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temp = path.with_extension("tmp");
    {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&temp, path)?;
    if let Some(parent) = path.parent() {
        // A directory fsync is unavailable on some platforms. The rename is
        // still atomic, so a failed directory fsync is not a data loss.
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
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
