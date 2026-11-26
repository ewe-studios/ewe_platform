//! The streamed unpack of a `.tar.gz` archive.
//!
//! The archive is read through the gzip decoder and the tar reader. Each entry
//! goes to the target directory in a bounded copy, so the unpack never holds a
//! whole member in memory (invariant I3).
//!
//! The unpack rejects a path with a `..` part or an absolute path (invariant
//! I5). It also rejects a write through a symbolic link that an earlier entry
//! made, so no entry can escape the target directory. Device nodes are skipped
//! (the Alpine minirootfs has none).

use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use flate2::read::GzDecoder;
use tar::EntryType;

use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::HostFs;
use crate::shared::limits::ArtifactLimits;

/// An input or output failure with its operation.
fn io_error(op: &str, path: &Path, error: &std::io::Error) -> SandboxError {
    SandboxError::Io {
        op: format!("unpack {op}"),
        message: format!("{}: {error}", path.display()),
    }
}

/// An unsafe path error.
fn unsafe_path(path: &Path) -> SandboxError {
    SandboxError::ArtifactUnsafePath {
        path: path.display().to_string(),
    }
}

/// A too-large error.
fn too_large(limit: u64, message: String) -> SandboxError {
    SandboxError::ArtifactTooLarge { limit, message }
}

/// The safe form of an archive path: no absolute root, no `..` part.
///
/// A `.` part and a repeated `/` are dropped. `None` means the archive root
/// entry (`./`), which names the target directory itself.
fn checked_relative(path: &Path) -> SandboxResult<Option<PathBuf>> {
    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => safe.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(unsafe_path(path).into());
            }
        }
    }
    if safe.as_os_str().is_empty() {
        return Ok(None);
    }
    Ok(Some(safe))
}

/// Whether any parent part of `path` is a symbolic link that this unpack made.
fn passes_through_symlink(path: &Path, symlinks: &HashSet<PathBuf>) -> bool {
    let mut ancestor = PathBuf::new();
    for component in path.components() {
        ancestor.push(component);
        if symlinks.contains(&ancestor) {
            return true;
        }
    }
    false
}

/// Sets the mode of `path`. A non-Unix host has no mode, so it skips the call.
#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> SandboxResult<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode & 0o7777))
        .map_err(|error| io_error("chmod", path, &error).into())
}

#[cfg(not(unix))]
fn set_mode(path: &Path, mode: u32) -> SandboxResult<()> {
    let _ = (path, mode);
    Ok(())
}

/// Copies one regular member to `dest` in bounded chunks.
fn write_member(
    fs: &Arc<dyn HostFs>,
    member: &mut impl Read,
    dest: &Path,
    mode: u32,
    limits: &ArtifactLimits,
    unpacked: &mut u64,
) -> SandboxResult<()> {
    let mut writer = fs.create_file(dest)?;
    let mut buffer = vec![0u8; limits.chunk_bytes];
    loop {
        let read = member
            .read(&mut buffer)
            .map_err(|error| io_error("read member", dest, &error))?;
        if read == 0 {
            break;
        }
        *unpacked = unpacked.saturating_add(read as u64);
        if *unpacked > limits.max_unpacked_bytes {
            return Err(too_large(
                limits.max_unpacked_bytes,
                format!("the archive passed the limit after {} bytes", *unpacked),
            )
            .into());
        }
        writer
            .write_all(&buffer[..read])
            .map_err(|error| io_error("write member", dest, &error))?;
    }
    writer
        .flush()
        .map_err(|error| io_error("flush member", dest, &error))?;
    drop(writer);
    set_mode(dest, mode)
}

/// Makes the parent directory of `dest`, with mode `0755`.
fn make_parent(fs: &Arc<dyn HostFs>, dest: &Path) -> SandboxResult<()> {
    if let Some(parent) = dest.parent() {
        fs.create_dir_all(parent, 0o755)?;
    }
    Ok(())
}

/// Unpacks `archive` into `target`.
///
/// The caller makes `target` first and removes it on failure.
///
/// # Errors
/// [`SandboxError::ArtifactUnsafePath`] for an unsafe path,
/// [`SandboxError::ArtifactTooLarge`] for an archive above a limit, and
/// [`SandboxError::Io`] for a read or write failure.
pub fn unpack(
    fs: &Arc<dyn HostFs>,
    archive: &Path,
    target: &Path,
    limits: &ArtifactLimits,
) -> SandboxResult<()> {
    fs.create_dir_all(target, 0o755)?;
    let file = std::fs::File::open(archive).map_err(|error| io_error("open", archive, &error))?;
    let decoder = GzDecoder::new(std::io::BufReader::new(file));
    let mut reader = tar::Archive::new(decoder);
    let entries = reader
        .entries()
        .map_err(|error| io_error("read index", archive, &error))?;
    let mut unpacked = 0u64;
    let mut count = 0usize;
    let mut symlinks: HashSet<PathBuf> = HashSet::new();
    for entry in entries {
        let mut entry = entry.map_err(|error| io_error("read entry", archive, &error))?;
        count += 1;
        if count > limits.max_entries {
            return Err(too_large(
                u64::try_from(limits.max_entries).unwrap_or(u64::MAX),
                format!("the archive passed the entry limit after {count} entries"),
            )
            .into());
        }
        let raw = entry
            .path()
            .map_err(|error| io_error("read name", archive, &error))?
            .into_owned();
        let mode = entry.header().mode().unwrap_or(0o644);
        let Some(relative) = checked_relative(&raw)? else {
            // The archive root entry names the target directory itself.
            set_mode(target, mode)?;
            continue;
        };
        if passes_through_symlink(&relative, &symlinks) {
            return Err(unsafe_path(&raw).into());
        }
        let dest = target.join(&relative);
        match entry.header().entry_type() {
            EntryType::Directory => {
                make_parent(fs, &dest)?;
                fs.create_dir_all(&dest, mode)?;
                set_mode(&dest, mode)?;
            }
            EntryType::Regular | EntryType::Continuous | EntryType::GNUSparse => {
                make_parent(fs, &dest)?;
                write_member(fs, &mut entry, &dest, mode, limits, &mut unpacked)?;
            }
            EntryType::Symlink => {
                make_parent(fs, &dest)?;
                let link = entry
                    .link_name()
                    .map_err(|error| io_error("read link", archive, &error))?
                    .ok_or_else(|| unsafe_path(&raw))?;
                make_symlink(&link, &dest, &raw)?;
                symlinks.insert(relative);
            }
            EntryType::Link => {
                make_parent(fs, &dest)?;
                let link = entry
                    .link_name()
                    .map_err(|error| io_error("read link", archive, &error))?
                    .ok_or_else(|| unsafe_path(&raw))?;
                let link_relative = checked_relative(&link)?.ok_or_else(|| unsafe_path(&raw))?;
                if passes_through_symlink(&link_relative, &symlinks) {
                    return Err(unsafe_path(&raw).into());
                }
                std::fs::hard_link(target.join(&link_relative), &dest)
                    .map_err(|error| io_error("hard link", &dest, &error))?;
            }
            EntryType::Char | EntryType::Block | EntryType::Fifo => {
                tracing::debug!(
                    path = %raw.display(),
                    "artifact unpack skips a device or fifo entry"
                );
            }
            _ => {
                tracing::debug!(path = %raw.display(), "artifact unpack skips an entry");
            }
        }
    }
    Ok(())
}

/// Makes the symbolic link `dest` with the target `link`.
#[cfg(unix)]
fn make_symlink(link: &Path, dest: &Path, raw: &Path) -> SandboxResult<()> {
    let _ = raw;
    std::os::unix::fs::symlink(link, dest).map_err(|error| io_error("symlink", dest, &error).into())
}

/// A non-Unix host cannot make a symbolic link. The skip keeps the tree usable
/// for the tests that run there.
#[cfg(not(unix))]
fn make_symlink(link: &Path, dest: &Path, raw: &Path) -> SandboxResult<()> {
    tracing::debug!(
        path = %raw.display(),
        target = %link.display(),
        dest = %dest.display(),
        "artifact unpack skips a symbolic link on this host"
    );
    Ok(())
}
