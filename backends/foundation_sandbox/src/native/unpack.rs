//! The tar reader and applier of an image unpack (feature 12, section 6.6).
//!
//! The slave runs this code in a child process whose root is the target
//! directory. This module is portable Unix code, so the tests apply a tar to a
//! temporary directory without a chroot and without the right to root.
//!
//! The reader supports ustar, PAX extended headers and GNU long names. It
//! applies regular files, directories, symbolic links, hard links, character
//! and block devices, and first-in first-out (FIFO) files. It refuses a GNU
//! sparse file and a PAX global header that sets a path.
//!
//! No entry writes outside the root: an absolute path loses its first `/`, a
//! `..` part is refused, and a symbolic link resolves inside the root, as it
//! does in a `chroot` child.

use std::collections::{HashSet, VecDeque};
use std::ffi::CString;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use crate::shared::unpack::{UnpackOptions, UNPACK_LIMIT_MESSAGE};

/// One tar block in bytes.
const BLOCK: usize = 512;
/// The most symbolic links that one path component resolves through.
const MAX_LINK_DEPTH: usize = 64;
/// The buffer of one file data copy.
const COPY_BUFFER: usize = 64 * 1024;

/// The limits of one unpack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnpackLimits {
    /// The most entries that the reader applies.
    pub max_entries: usize,
    /// The most file data bytes that the reader writes.
    pub max_bytes: u64,
    /// The longest path in bytes.
    pub max_path_bytes: usize,
}

impl Default for UnpackLimits {
    fn default() -> Self {
        Self {
            max_entries: 1_000_000,
            max_bytes: 16 * 1024 * 1024 * 1024,
            // Linux `PATH_MAX`.
            max_path_bytes: 4096,
        }
    }
}

/// What one unpack wrote.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UnpackStats {
    /// The number of applied entries.
    pub entries: u64,
    /// The number of file data bytes.
    pub bytes: u64,
}

/// Why one unpack stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnpackError {
    /// The tar bytes are malformed or use an unsupported form.
    BadRequest {
        /// The entry path, when the reader knows it.
        path: String,
        /// The reason.
        message: String,
    },
    /// One entry could not be applied.
    Entry {
        /// The entry path.
        path: String,
        /// The operating system error number.
        errno: Option<i32>,
        /// The reason.
        message: String,
    },
    /// The unpack passed a limit.
    Limit {
        /// The reason, which starts with [`UNPACK_LIMIT_MESSAGE`].
        message: String,
    },
}

impl UnpackError {
    /// The error of one entry.
    fn entry(path: &str, error: &io::Error) -> Self {
        Self::Entry {
            path: path.to_string(),
            errno: error.raw_os_error(),
            message: error.to_string(),
        }
    }
}

impl std::fmt::Display for UnpackError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadRequest { path, message } => {
                write!(formatter, "bad tar entry {path}: {message}")
            }
            Self::Entry {
                path,
                errno,
                message,
            } => write!(
                formatter,
                "entry {path} failed: {message} (errno {errno:?})"
            ),
            Self::Limit { message } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for UnpackError {}

/// Applies an uncompressed tar stream to `root`.
///
/// `root` is the directory that the caller treats as `/`. The function never
/// follows a path outside it and applies the entries in tar order.
///
/// # Errors
/// [`UnpackError`] for a bad header, a failed entry or a limit.
#[allow(
    clippy::too_many_lines,
    reason = "the tar order is one readable pass: the extended headers, the whiteouts, the limits and one entry"
)]
pub fn unpack<R: Read>(
    reader: R,
    root: &Path,
    options: &UnpackOptions,
    limits: &UnpackLimits,
) -> Result<UnpackStats, UnpackError> {
    let mut reader = TarReader::new(reader);
    let skip: HashSet<String> = options
        .skip
        .iter()
        .map(|path| normalize_rel(path).unwrap_or_default())
        .collect();
    let mut created: HashSet<String> = HashSet::new();
    let mut stats = UnpackStats::default();
    let mut pending = Pending::default();
    while let Some(block) = reader.next_block().map_err(|error| UnpackError::Entry {
        path: String::new(),
        errno: error.raw_os_error(),
        message: error.to_string(),
    })? {
        if is_zero(&block) {
            // The first zero block ends the archive; a second one is padding.
            break;
        }
        let header = Header::parse(&block)?;
        match header.kind {
            HeaderKind::Pax => {
                let value = pending.take_pax(&mut reader, &header)?;
                pending.apply_pax(value);
                continue;
            }
            HeaderKind::PaxGlobal => {
                let value = pending.take_pax(&mut reader, &header)?;
                check_pax_global(&value)?;
                continue;
            }
            HeaderKind::LongName => {
                pending.long_name = Some(read_text(&mut reader, header.size)?);
                continue;
            }
            HeaderKind::LongLink => {
                pending.long_link = Some(read_text(&mut reader, header.size)?);
                continue;
            }
            HeaderKind::Sparse => {
                let path = pending.path(&header);
                return Err(UnpackError::BadRequest {
                    path,
                    message: "a GNU sparse file is not supported".to_string(),
                });
            }
            HeaderKind::Regular
            | HeaderKind::Directory
            | HeaderKind::Symlink
            | HeaderKind::HardLink
            | HeaderKind::Char
            | HeaderKind::Block
            | HeaderKind::Fifo => {}
        }
        stats.entries = stats.entries.saturating_add(1);
        if stats.entries > limits.max_entries as u64 {
            return Err(UnpackError::Limit {
                message: format!(
                    "{UNPACK_LIMIT_MESSAGE}: more than {} entries",
                    limits.max_entries
                ),
            });
        }
        let path = pending.path(&header);
        let relative = normalize_rel(&path).ok_or_else(|| UnpackError::BadRequest {
            path: path.clone(),
            message: "the path is empty, absolute only, or has a `..` part".to_string(),
        })?;
        if relative.len() > limits.max_path_bytes {
            return Err(UnpackError::Limit {
                message: format!(
                    "{UNPACK_LIMIT_MESSAGE}: the path is longer than {} bytes",
                    limits.max_path_bytes
                ),
            });
        }
        // A whiteout creates no file. It removes what it names.
        if options.oci_whiteouts {
            if let Some(removed) = apply_whiteout(root, &relative, &created)? {
                if removed {
                    reader.skip_data(header.size)?;
                    pending.clear();
                    continue;
                }
            }
        }
        if skip.contains(&relative) {
            reader.skip_data(header.size)?;
            pending.clear();
            continue;
        }
        if header.kind == HeaderKind::Regular {
            let total = stats.bytes.saturating_add(header.size);
            if total > limits.max_bytes {
                return Err(UnpackError::Limit {
                    message: format!(
                        "{UNPACK_LIMIT_MESSAGE}: more than {} bytes",
                        limits.max_bytes
                    ),
                });
            }
            stats.bytes = total;
        }
        let destination = resolve_in_root(root, &relative)?;
        apply_entry(&mut reader, &header, &pending, root, &destination, &path).map_err(
            |error| match error {
                // The reader reports the entry path, not one host path of the
                // entry, so a caller can name the tar entry that failed.
                UnpackError::Entry { errno, message, .. } => UnpackError::Entry {
                    path: path.clone(),
                    errno,
                    message,
                },
                other => other,
            },
        )?;
        mark_created(&mut created, &relative);
        pending.clear();
    }
    Ok(stats)
}

/// The tar header fields that the applier uses.
#[derive(Clone, Debug)]
struct Header {
    name: Vec<u8>,
    link_name: Vec<u8>,
    mode: u32,
    uid: u32,
    gid: u32,
    size: u64,
    mtime: u64,
    kind: HeaderKind,
    dev_major: u32,
    dev_minor: u32,
}

/// The tar entry kinds that this reader knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeaderKind {
    Regular,
    Directory,
    Symlink,
    HardLink,
    Char,
    Block,
    Fifo,
    Pax,
    PaxGlobal,
    LongName,
    LongLink,
    Sparse,
}

/// The kind of one type flag. A test reads it for a header that it cannot
/// create without the right to root.
#[must_use]
pub fn kind_from_typeflag(flag: u8) -> Option<&'static str> {
    Some(match flag {
        b'0' | 0 | b'7' => "regular",
        b'1' => "hard_link",
        b'2' => "symlink",
        b'3' => "char",
        b'4' => "block",
        b'5' => "directory",
        b'6' => "fifo",
        b'L' => "long_name",
        b'K' => "long_link",
        b'x' => "pax",
        b'g' => "pax_global",
        b'S' => "sparse",
        _ => return None,
    })
}

impl Header {
    fn parse(block: &[u8; BLOCK]) -> Result<Self, UnpackError> {
        if !checksum_ok(block) {
            return Err(UnpackError::BadRequest {
                path: String::new(),
                message: "the header checksum is wrong".to_string(),
            });
        }
        let kind = match block[156] {
            b'0' | 0 | b'7' => HeaderKind::Regular,
            b'1' => HeaderKind::HardLink,
            b'2' => HeaderKind::Symlink,
            b'3' => HeaderKind::Char,
            b'4' => HeaderKind::Block,
            b'5' => HeaderKind::Directory,
            b'6' => HeaderKind::Fifo,
            b'L' => HeaderKind::LongName,
            b'K' => HeaderKind::LongLink,
            b'x' => HeaderKind::Pax,
            b'g' => HeaderKind::PaxGlobal,
            b'S' => HeaderKind::Sparse,
            other => {
                return Err(UnpackError::BadRequest {
                    path: String::new(),
                    message: format!("the type flag {other:?} is not supported"),
                });
            }
        };
        // The prefix field belongs to the ustar format. Another format may put
        // other bytes there.
        let prefix = if &block[257..262] == b"ustar" {
            trim_zeros(&block[345..500])
        } else {
            &[][..]
        };
        let name = if prefix.is_empty() {
            trim_zeros(&block[0..100]).to_vec()
        } else {
            let mut full = prefix.to_vec();
            full.push(b'/');
            full.extend_from_slice(trim_zeros(&block[0..100]));
            full
        };
        Ok(Self {
            name,
            link_name: trim_zeros(&block[157..257]).to_vec(),
            mode: u32::try_from(parse_numeric(&block[100..108]).unwrap_or(0)).unwrap_or(0),
            uid: u32::try_from(parse_numeric(&block[108..116]).unwrap_or(0)).unwrap_or(0),
            gid: u32::try_from(parse_numeric(&block[116..124]).unwrap_or(0)).unwrap_or(0),
            size: parse_numeric(&block[124..136]).unwrap_or(0),
            mtime: parse_numeric(&block[136..148]).unwrap_or(0),
            kind,
            dev_major: u32::try_from(parse_numeric(&block[329..337]).unwrap_or(0)).unwrap_or(0),
            dev_minor: u32::try_from(parse_numeric(&block[337..345]).unwrap_or(0)).unwrap_or(0),
        })
    }
}

/// The values of the extended headers before the next entry.
#[derive(Clone, Debug, Default)]
struct Pending {
    long_name: Option<Vec<u8>>,
    long_link: Option<Vec<u8>>,
    pax_path: Option<Vec<u8>>,
    pax_link: Option<Vec<u8>>,
    pax_size: Option<u64>,
    pax_uid: Option<u32>,
    pax_gid: Option<u32>,
    pax_mtime: Option<u64>,
    pax_capability: Option<Vec<u8>>,
}

impl Pending {
    fn clear(&mut self) {
        *self = Self::default();
    }

    fn path(&self, header: &Header) -> String {
        let bytes = self
            .pax_path
            .clone()
            .or_else(|| self.long_name.clone())
            .unwrap_or_else(|| header.name.clone());
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn link(&self, header: &Header) -> Vec<u8> {
        self.pax_link
            .clone()
            .or_else(|| self.long_link.clone())
            .unwrap_or_else(|| header.link_name.clone())
    }

    fn take_pax<R: Read>(
        &self,
        reader: &mut TarReader<R>,
        header: &Header,
    ) -> Result<Vec<(String, Vec<u8>)>, UnpackError> {
        if header.size > 16 * 1024 * 1024 {
            return Err(UnpackError::BadRequest {
                path: self.path(header),
                message: "the PAX header is too large".to_string(),
            });
        }
        let data = reader.read_data(header.size)?;
        parse_pax(&data)
    }

    fn apply_pax(&mut self, records: Vec<(String, Vec<u8>)>) {
        for (key, value) in records {
            match key.as_str() {
                "path" => self.pax_path = Some(value),
                "linkpath" => self.pax_link = Some(value),
                "size" => self.pax_size = Some(parse_decimal(&value)),
                "uid" => self.pax_uid = Some(parse_decimal_u32(&value)),
                "gid" => self.pax_gid = Some(parse_decimal_u32(&value)),
                "mtime" => self.pax_mtime = Some(parse_decimal(&value)),
                "SCHILY.xattr.security.capability" => self.pax_capability = Some(value),
                _ => {}
            }
        }
    }
}

/// Refuses a PAX global header that sets a path.
fn check_pax_global(records: &[(String, Vec<u8>)]) -> Result<(), UnpackError> {
    if records.iter().any(|(key, _)| key == "path") {
        return Err(UnpackError::BadRequest {
            path: String::new(),
            message: "a PAX global header may not set a path".to_string(),
        });
    }
    Ok(())
}

/// A buffered reader of 512-byte tar blocks.
struct TarReader<R> {
    inner: R,
    finished: bool,
}

impl<R: Read> TarReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            finished: false,
        }
    }

    fn next_block(&mut self) -> io::Result<Option<[u8; BLOCK]>> {
        if self.finished {
            return Ok(None);
        }
        let mut block = [0_u8; BLOCK];
        let mut filled = 0;
        while filled < BLOCK {
            match self.inner.read(&mut block[filled..])? {
                0 if filled == 0 => {
                    self.finished = true;
                    return Ok(None);
                }
                0 => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "the tar archive ends in the middle of a block",
                    ));
                }
                read => filled += read,
            }
        }
        Ok(Some(block))
    }

    fn read_data(&mut self, size: u64) -> Result<Vec<u8>, UnpackError> {
        let length = usize::try_from(size).map_err(|_| UnpackError::BadRequest {
            path: String::new(),
            message: "the entry is too large for this host".to_string(),
        })?;
        let mut data = vec![0_u8; length];
        self.read_exact(&mut data)?;
        self.skip_padding(size)?;
        Ok(data)
    }

    fn read_into<W: Write>(&mut self, size: u64, sink: &mut W) -> Result<(), UnpackError> {
        let mut left = size;
        let mut buffer = vec![0_u8; COPY_BUFFER];
        while left > 0 {
            let step = usize::try_from(left.min(COPY_BUFFER as u64)).unwrap_or(COPY_BUFFER);
            self.read_exact(&mut buffer[..step])?;
            sink.write_all(&buffer[..step])
                .map_err(|error| UnpackError::Entry {
                    path: String::new(),
                    errno: error.raw_os_error(),
                    message: error.to_string(),
                })?;
            left -= step as u64;
        }
        self.skip_padding(size)
    }

    fn skip_data(&mut self, size: u64) -> Result<(), UnpackError> {
        let mut left = size;
        let mut buffer = vec![0_u8; COPY_BUFFER];
        while left > 0 {
            let step = usize::try_from(left.min(COPY_BUFFER as u64)).unwrap_or(COPY_BUFFER);
            self.read_exact(&mut buffer[..step])?;
            left -= step as u64;
        }
        self.skip_padding(size)
    }

    fn read_exact(&mut self, buffer: &mut [u8]) -> Result<(), UnpackError> {
        let mut filled = 0;
        while filled < buffer.len() {
            match self.inner.read(&mut buffer[filled..]) {
                Ok(0) => {
                    return Err(UnpackError::BadRequest {
                        path: String::new(),
                        message: "the tar archive ends in the middle of an entry".to_string(),
                    });
                }
                Ok(read) => filled += read,
                Err(error) => {
                    return Err(UnpackError::Entry {
                        path: String::new(),
                        errno: error.raw_os_error(),
                        message: error.to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    fn skip_padding(&mut self, size: u64) -> Result<(), UnpackError> {
        let remainder = usize::try_from(size % BLOCK as u64).unwrap_or(0);
        if remainder == 0 {
            return Ok(());
        }
        let mut padding = [0_u8; BLOCK];
        self.read_exact(&mut padding[..BLOCK - remainder])
    }
}

/// Applies one entry.
fn apply_entry<R: Read>(
    reader: &mut TarReader<R>,
    header: &Header,
    pending: &Pending,
    root: &Path,
    destination: &Path,
    path: &str,
) -> Result<(), UnpackError> {
    let uid = pending.pax_uid.unwrap_or(header.uid);
    let gid = pending.pax_gid.unwrap_or(header.gid);
    let size = pending.pax_size.unwrap_or(header.size);
    let mtime = pending.pax_mtime.unwrap_or(header.mtime);
    let mode = header.mode & 0o7777;
    match header.kind {
        HeaderKind::Regular => {
            remove_other(destination, false)?;
            ensure_parent(destination)?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(destination)
                .map_err(|error| UnpackError::entry(path, &error))?;
            reader.read_into(size, &mut file)?;
            file.flush()
                .map_err(|error| UnpackError::entry(path, &error))?;
            set_owner(destination, uid, gid, path)?;
            set_mode(destination, mode, path)?;
            set_capability(destination, pending.pax_capability.as_deref(), path)?;
        }
        HeaderKind::Directory => {
            match fs::symlink_metadata(destination) {
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => {
                    remove_other(destination, true)?;
                    create_dir(destination, path)?;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    create_dir(destination, path)?;
                }
                Err(error) => return Err(UnpackError::entry(path, &error)),
            }
            set_owner(destination, uid, gid, path)?;
            set_mode(destination, mode, path)?;
        }
        HeaderKind::Symlink => {
            remove_other(destination, true)?;
            ensure_parent(destination)?;
            let target = pending.link(header);
            let target = String::from_utf8_lossy(&target).into_owned();
            std::os::unix::fs::symlink(&target, destination)
                .map_err(|error| UnpackError::entry(path, &error))?;
            set_owner(destination, uid, gid, path)?;
        }
        HeaderKind::HardLink => {
            remove_other(destination, true)?;
            ensure_parent(destination)?;
            let target = pending.link(header);
            let target = String::from_utf8_lossy(&target).into_owned();
            let relative = normalize_rel(&target).ok_or_else(|| UnpackError::BadRequest {
                path: path.to_string(),
                message: format!("the hard link target {target} is not a safe relative path"),
            })?;
            let source = resolve_in_root(root, &relative)?;
            if !source.exists() {
                return Err(UnpackError::BadRequest {
                    path: path.to_string(),
                    message: format!("the hard link target {target} is not an earlier entry"),
                });
            }
            fs::hard_link(&source, destination)
                .map_err(|error| UnpackError::entry(path, &error))?;
        }
        HeaderKind::Char | HeaderKind::Block | HeaderKind::Fifo => {
            remove_other(destination, true)?;
            ensure_parent(destination)?;
            let kind: libc::mode_t = match header.kind {
                HeaderKind::Char => libc::S_IFCHR,
                HeaderKind::Block => libc::S_IFBLK,
                _ => libc::S_IFIFO,
            };
            let device = if kind == libc::S_IFIFO {
                0
            } else {
                device_number(header.dev_major, header.dev_minor)
            };
            let path_c = cstring(destination)?;
            // SAFETY: `path_c` is a valid zero-terminated path. `mknod` creates
            // one node and reads no other state.
            let node_mode = (mode as libc::mode_t) | kind;
            let result = unsafe { libc::mknod(path_c.as_ptr(), node_mode, device) };
            if result != 0 {
                return Err(UnpackError::entry(path, &io::Error::last_os_error()));
            }
            set_owner(destination, uid, gid, path)?;
            set_mode(destination, mode, path)?;
        }
        HeaderKind::Pax
        | HeaderKind::PaxGlobal
        | HeaderKind::LongName
        | HeaderKind::LongLink
        | HeaderKind::Sparse => {}
    }
    if header.kind != HeaderKind::Regular {
        // A malformed archive may give a non-regular entry data. Consume it so
        // the next header stays aligned.
        reader.skip_data(header.size)?;
    }
    set_times(destination, mtime, path)
}

/// Applies one OCI whiteout. It answers `Some(true)` when the entry was a
/// whiteout and created no file.
fn apply_whiteout(
    root: &Path,
    relative: &str,
    created: &HashSet<String>,
) -> Result<Option<bool>, UnpackError> {
    let (parent, name) = match relative.rsplit_once('/') {
        Some((parent, name)) => (parent, name),
        None => ("", relative),
    };
    if name == ".wh..wh..opq" {
        let directory = if parent.is_empty() {
            root.to_path_buf()
        } else {
            resolve_in_root(root, parent)?
        };
        let entries = fs::read_dir(&directory).map_err(|error| UnpackError::Entry {
            path: relative.to_string(),
            errno: error.raw_os_error(),
            message: error.to_string(),
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| UnpackError::Entry {
                path: relative.to_string(),
                errno: error.raw_os_error(),
                message: error.to_string(),
            })?;
            let child_name = entry.file_name();
            let child = if parent.is_empty() {
                child_name.to_string_lossy().into_owned()
            } else {
                format!("{parent}/{}", child_name.to_string_lossy())
            };
            if !created.contains(&child) {
                remove_path(&entry.path())?;
            }
        }
        return Ok(Some(true));
    }
    let Some(stripped) = name.strip_prefix(".wh.") else {
        return Ok(None);
    };
    if stripped.is_empty() {
        return Ok(None);
    }
    let target = if parent.is_empty() {
        stripped.to_string()
    } else {
        format!("{parent}/{stripped}")
    };
    let destination = resolve_in_root(root, &target)?;
    remove_path(&destination)?;
    Ok(Some(true))
}

/// Marks one relative path and its parents as created by this unpack.
fn mark_created(created: &mut HashSet<String>, relative: &str) {
    let mut current = relative;
    loop {
        created.insert(current.to_string());
        match current.rsplit_once('/') {
            Some((parent, _)) if !parent.is_empty() => current = parent,
            _ => break,
        }
    }
    created.insert(String::new());
}

/// Resolves a relative path inside `root`, following symbolic links as a
/// `chroot` child does. An absolute link target is inside the root.
fn resolve_in_root(root: &Path, relative: &str) -> Result<PathBuf, UnpackError> {
    let mut resolved = root.to_path_buf();
    let mut pending: VecDeque<String> = relative
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .map(ToString::to_string)
        .collect();
    let mut links = 0_usize;
    while let Some(part) = pending.pop_front() {
        if part == ".." {
            if resolved != root {
                resolved.pop();
            }
            continue;
        }
        let candidate = resolved.join(&part);
        if pending.is_empty() {
            resolved = candidate;
            break;
        }
        match fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                links += 1;
                if links > MAX_LINK_DEPTH {
                    return Err(UnpackError::BadRequest {
                        path: relative.to_string(),
                        message: "a symbolic link loop".to_string(),
                    });
                }
                let target = fs::read_link(&candidate).map_err(|error| UnpackError::Entry {
                    path: relative.to_string(),
                    errno: error.raw_os_error(),
                    message: error.to_string(),
                })?;
                let target = target.to_string_lossy();
                if target.starts_with('/') {
                    resolved = root.to_path_buf();
                }
                let mut next: VecDeque<String> = target
                    .split('/')
                    .filter(|part| !part.is_empty() && *part != ".")
                    .map(ToString::to_string)
                    .collect();
                next.append(&mut pending);
                pending = next;
            }
            _ => resolved = candidate,
        }
    }
    Ok(resolved)
}

/// Normalizes one tar path to a safe relative path.
fn normalize_rel(path: &str) -> Option<String> {
    let mut parts = Vec::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return None;
        }
        parts.push(part);
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

/// Removes what is at `path`, when its type differs from the new entry.
fn remove_other(path: &Path, keep_directory: bool) -> Result<(), UnpackError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(UnpackError::entry(&path.display().to_string(), &error)),
        Ok(metadata) => {
            if keep_directory && metadata.is_dir() {
                return Ok(());
            }
            remove_path(path)
        }
    }
}

/// Removes one path and everything below it.
fn remove_path(path: &Path) -> Result<(), UnpackError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(UnpackError::entry(&path.display().to_string(), &error)),
        Ok(metadata) => {
            let result = if metadata.is_dir() {
                fs::remove_dir_all(path)
            } else {
                fs::remove_file(path)
            };
            result.map_err(|error| UnpackError::entry(&path.display().to_string(), &error))
        }
    }
}

fn ensure_parent(path: &Path) -> Result<(), UnpackError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| UnpackError::entry(&parent.display().to_string(), &error))?;
    }
    Ok(())
}

fn create_dir(path: &Path, label: &str) -> Result<(), UnpackError> {
    fs::create_dir_all(path).map_err(|error| UnpackError::entry(label, &error))
}

fn set_owner(path: &Path, uid: u32, gid: u32, label: &str) -> Result<(), UnpackError> {
    let path_c = cstring(path)?;
    // SAFETY: `path_c` is valid and zero-terminated; `lchown` reads only it.
    let result = unsafe { libc::lchown(path_c.as_ptr(), uid, gid) };
    if result != 0 {
        return Err(UnpackError::entry(label, &io::Error::last_os_error()));
    }
    Ok(())
}

fn set_mode(path: &Path, mode: u32, label: &str) -> Result<(), UnpackError> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|error| UnpackError::entry(label, &error))
}

/// The device number of one character or block device.
///
/// Linux takes unsigned major and minor numbers; the Apple systems take signed
/// ones.
fn device_number(major: u32, minor: u32) -> libc::dev_t {
    #[cfg(target_os = "linux")]
    {
        libc::makedev(major, minor)
    }
    #[cfg(not(target_os = "linux"))]
    {
        libc::makedev(
            i32::try_from(major).unwrap_or(i32::MAX),
            i32::try_from(minor).unwrap_or(i32::MAX),
        )
    }
}

/// Sets the security capability attribute on Linux. Another system ignores it:
/// the attribute is a Linux kernel feature.
#[allow(
    clippy::unnecessary_wraps,
    reason = "the Linux body can fail; another system has nothing to do"
)]
fn set_capability(path: &Path, capability: Option<&[u8]>, label: &str) -> Result<(), UnpackError> {
    let Some(capability) = capability else {
        return Ok(());
    };
    #[cfg(target_os = "linux")]
    {
        let path_c = cstring(path)?;
        let name = c"security.capability";
        // SAFETY: `path_c` and `name` are valid; the value lives for the call.
        let result = unsafe {
            libc::lsetxattr(
                path_c.as_ptr(),
                name.as_ptr(),
                capability.as_ptr().cast(),
                capability.len(),
                0,
            )
        };
        if result != 0 {
            return Err(UnpackError::entry(label, &io::Error::last_os_error()));
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (path, capability, label);
    }
    Ok(())
}

fn set_times(path: &Path, mtime: u64, label: &str) -> Result<(), UnpackError> {
    let path_c = cstring(path)?;
    let seconds = i64::try_from(mtime).unwrap_or(i64::MAX);
    let times = [
        libc::timespec {
            tv_sec: seconds,
            tv_nsec: 0,
        },
        libc::timespec {
            tv_sec: seconds,
            tv_nsec: 0,
        },
    ];
    // SAFETY: `path_c` is valid, `times` holds two values, and the call reads
    // only them. It does not follow a symbolic link.
    let result = unsafe {
        libc::utimensat(
            libc::AT_FDCWD,
            path_c.as_ptr(),
            times.as_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result != 0 {
        return Err(UnpackError::entry(label, &io::Error::last_os_error()));
    }
    Ok(())
}

fn cstring(path: &Path) -> Result<CString, UnpackError> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| UnpackError::BadRequest {
        path: path.display().to_string(),
        message: "the path has a zero byte".to_string(),
    })
}

/// Reads a text value of one extended header.
fn read_text<R: Read>(reader: &mut TarReader<R>, size: u64) -> Result<Vec<u8>, UnpackError> {
    let mut data = reader.read_data(size)?;
    while data.last() == Some(&0) {
        data.pop();
    }
    Ok(data)
}

/// Parses the records of one PAX extended header.
fn parse_pax(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>, UnpackError> {
    let mut records = Vec::new();
    let mut index = 0;
    while index < data.len() {
        let start = index;
        while index < data.len() && data[index].is_ascii_digit() {
            index += 1;
        }
        if index == start || index >= data.len() || data[index] != b' ' {
            return Err(UnpackError::BadRequest {
                path: String::new(),
                message: "a PAX record has no length".to_string(),
            });
        }
        let length: usize = std::str::from_utf8(&data[start..index])
            .ok()
            .and_then(|text| text.parse().ok())
            .ok_or_else(|| UnpackError::BadRequest {
                path: String::new(),
                message: "a PAX record has a bad length".to_string(),
            })?;
        let end = start
            .checked_add(length)
            .ok_or_else(|| UnpackError::BadRequest {
                path: String::new(),
                message: "a PAX record length is too large".to_string(),
            })?;
        if end > data.len() || end <= index + 1 {
            return Err(UnpackError::BadRequest {
                path: String::new(),
                message: "a PAX record length is out of bounds".to_string(),
            });
        }
        let mut record = &data[index + 1..end];
        if record.last() == Some(&b'\n') {
            record = &record[..record.len() - 1];
        }
        let Some(split) = record.iter().position(|byte| *byte == b'=') else {
            return Err(UnpackError::BadRequest {
                path: String::new(),
                message: "a PAX record has no key".to_string(),
            });
        };
        let key = String::from_utf8_lossy(&record[..split]).into_owned();
        records.push((key, record[split + 1..].to_vec()));
        index = end;
    }
    Ok(records)
}

/// The value of one decimal PAX field. A fractional part is cut.
fn parse_decimal(value: &[u8]) -> u64 {
    let text = String::from_utf8_lossy(value);
    let whole = text.split('.').next().unwrap_or_default().trim();
    whole.parse().unwrap_or(0)
}

fn parse_decimal_u32(value: &[u8]) -> u32 {
    u32::try_from(parse_decimal(value)).unwrap_or(u32::MAX)
}

/// Reads one tar numeric field: octal, or base-256 when the first byte has the
/// high bit set.
fn parse_numeric(field: &[u8]) -> Option<u64> {
    let first = *field.first()?;
    if first & 0x80 != 0 {
        let mut value = u64::from(first & 0x7f);
        for byte in &field[1..] {
            value = value.checked_mul(256)?.checked_add(u64::from(*byte))?;
        }
        return Some(value);
    }
    let mut value = 0_u64;
    let mut seen = false;
    for byte in field {
        match byte {
            b'0'..=b'7' => {
                seen = true;
                value = value.checked_mul(8)?.checked_add(u64::from(byte - b'0'))?;
            }
            b' ' | 0 => {
                if seen {
                    break;
                }
            }
            _ => return None,
        }
    }
    Some(value)
}

/// Whether the header checksum matches.
fn checksum_ok(block: &[u8; BLOCK]) -> bool {
    let Some(stored) = parse_numeric(&block[148..156]) else {
        return false;
    };
    let mut unsigned = 0_u64;
    let mut signed = 0_i64;
    for (index, byte) in block.iter().enumerate() {
        if (148..156).contains(&index) {
            unsigned += u64::from(b' ');
            signed += i64::from(b' ');
        } else {
            unsigned += u64::from(*byte);
            // The signed sum treats one byte as a signed character.
            signed += if *byte < 128 {
                i64::from(*byte)
            } else {
                i64::from(*byte) - 256
            };
        }
    }
    // The checksum field is spaces in both sums.
    let stored = i64::try_from(stored).unwrap_or(i64::MAX);
    stored == i64::try_from(unsigned).unwrap_or(i64::MAX) || stored == signed
}

fn is_zero(block: &[u8; BLOCK]) -> bool {
    block.iter().all(|byte| *byte == 0)
}

fn trim_zeros(field: &[u8]) -> &[u8] {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    &field[..end]
}
