//! The steps that change the image: `ADD`, `FETCH` and `RUN` (feature 14,
//! sections 6.3 to 6.5).
//!
//! `ADD` and `FETCH` build one tar stream and send it to the guest through
//! [`SandboxHandle::unpack`](crate::shared::control::SandboxHandle::unpack).
//! The stream reads each file in bounded chunks, so an input tree never sits
//! whole in host memory.
//!
//! `RUN` starts the step's shell in the build sandbox and writes the script to
//! its standard input.

use std::collections::VecDeque;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use foundation_context::Ctx;
use futures::Stream;
use sha2::{Digest as _, Sha256};

use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::{EntryKind, HostFs};
use crate::shared::limits::BuildLimits;
use crate::shared::BoxByteStream;

use super::keys::{sha256_hex, tree_hash, TreeEntry, TreeKind};

/// The largest chunk of file content that the tar stream yields.
const TAR_CHUNK: usize = 64 * 1024;

/// One entry of a tar stream.
#[derive(Clone, Debug)]
pub struct TarSpec {
    /// The absolute path in the guest.
    pub path: String,
    /// The type of the entry.
    pub kind: TreeKind,
    /// The mode.
    pub mode: u32,
    /// The content size. A directory has size 0.
    pub size: u64,
    /// Where the content comes from.
    pub source: TarSource,
}

/// Where the content of one tar entry comes from.
#[derive(Clone, Debug)]
pub enum TarSource {
    /// No content: a directory, or a symbolic link whose target is `link`.
    Empty,
    /// A host file, read in bounded chunks.
    File(PathBuf),
    /// The target of a symbolic link.
    Link(String),
}

/// One tar stream over a list of entries.
#[derive(Clone, Debug)]
pub struct TarPlan {
    /// The entries, parents first.
    pub entries: Vec<TarSpec>,
}

impl TarPlan {
    /// The sum of the file content sizes.
    #[must_use]
    pub fn file_bytes(&self) -> u64 {
        self.entries
            .iter()
            .filter(|entry| entry.kind == TreeKind::File)
            .fold(0_u64, |total, entry| total.saturating_add(entry.size))
    }

    /// The tar stream of this plan.
    ///
    /// The stream yields at most one 64 KiB chunk of file content at a time.
    pub fn tar_stream(self, fs: Arc<dyn HostFs>) -> BoxByteStream {
        Box::pin(TarStream::new(self.entries, fs))
    }
}

/// The input plan of one `ADD` step.
#[derive(Clone, Debug)]
pub struct AddPlan {
    /// The tar stream and its entries.
    pub tar: TarPlan,
    /// The tree hash of the input.
    pub tree_hash: String,
}

/// Builds the `ADD` plan of one source.
///
/// # Errors
/// [`SandboxError::InvalidSpec`] when the source leaves the build context or
/// is missing, and [`SandboxError::ArtifactTooLarge`] when the input passes a
/// configured limit.
pub fn plan_add(
    fs: &Arc<dyn HostFs>,
    context: &Path,
    source: &str,
    destination: &str,
    mode: Option<u32>,
    limits: &BuildLimits,
) -> SandboxResult<AddPlan> {
    let canonical_context = fs.canonicalize(context).map_err(|error| {
        invalid(format!(
            "the build context {} is not readable: {error}",
            context.display()
        ))
    })?;
    let source_path = Path::new(source);
    if source_path.is_absolute()
        || source_path
            .components()
            .any(|component| component == Component::ParentDir)
    {
        return Err(invalid(
            "the ADD source must stay inside the build context".to_string(),
        ));
    }
    let joined = canonical_context.join(source_path);
    let canonical = fs
        .canonicalize(&joined)
        .map_err(|_| invalid(format!("the ADD source {source} does not exist")))?;
    if !canonical.starts_with(&canonical_context) {
        return Err(invalid(format!(
            "the ADD source {source} leaves the build context"
        )));
    }
    let mut entries = Vec::new();
    let mut guest_paths = Vec::new();
    collect_add(
        fs,
        &canonical,
        source,
        destination,
        mode,
        &mut entries,
        &mut guest_paths,
    )?;
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    entries.dedup_by(|left, right| left.path == right.path);
    if entries.len() > limits.max_context_entries {
        return Err(SandboxError::ArtifactTooLarge {
            limit: u64::try_from(limits.max_context_entries).unwrap_or(u64::MAX),
            message: format!(
                "the ADD input has more than {} entries",
                limits.max_context_entries
            ),
        }
        .into());
    }
    let plan = TarPlan { entries };
    let bytes = plan.file_bytes();
    if bytes > limits.max_context_bytes {
        return Err(SandboxError::ArtifactTooLarge {
            limit: limits.max_context_bytes,
            message: format!(
                "the ADD input is larger than {} bytes",
                limits.max_context_bytes
            ),
        }
        .into());
    }
    let tree_hash = tree_hash(&guest_paths);
    Ok(AddPlan {
        tar: plan,
        tree_hash,
    })
}

/// The `FETCH` plan: one verified file plus its parent directories.
#[must_use]
pub fn plan_fetch(cache_path: &Path, destination: &str, size: u64) -> TarPlan {
    let mut entries = Vec::new();
    push_parents(&mut entries, destination);
    entries.push(TarSpec {
        path: destination.to_string(),
        kind: TreeKind::File,
        mode: 0o644,
        size,
        source: TarSource::File(cache_path.to_path_buf()),
    });
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    entries.dedup_by(|left, right| left.path == right.path);
    TarPlan { entries }
}

/// Collects the entries of one `ADD` source.
fn collect_add(
    fs: &Arc<dyn HostFs>,
    canonical: &Path,
    source: &str,
    destination: &str,
    mode: Option<u32>,
    entries: &mut Vec<TarSpec>,
    tree: &mut Vec<TreeEntry>,
) -> SandboxResult<()> {
    let meta = fs.entry_meta(canonical)?;
    match meta.kind {
        EntryKind::File => {
            let name = canonical.file_name().map_or_else(
                || source.to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            let target = if destination.ends_with('/') {
                join_guest(destination, &name)
            } else {
                destination.to_string()
            };
            push_parents(entries, &target);
            push_file(fs, entries, tree, canonical, &target, mode)
        }
        EntryKind::Directory => {
            let root = destination.trim_end_matches('/');
            push_parents(entries, root);
            entries.push(TarSpec {
                path: root.to_string(),
                kind: TreeKind::Dir,
                mode: 0o755,
                size: 0,
                source: TarSource::Empty,
            });
            walk_directory(fs, canonical, canonical, root, mode, entries, tree)
        }
        EntryKind::Symlink | EntryKind::Other => Err(invalid(format!(
            "the ADD source {source} must be a file or a directory"
        ))),
    }
}

/// Walks one directory that `ADD` copies.
fn walk_directory(
    fs: &Arc<dyn HostFs>,
    root: &Path,
    directory: &Path,
    guest_root: &str,
    mode: Option<u32>,
    entries: &mut Vec<TarSpec>,
    tree: &mut Vec<TreeEntry>,
) -> SandboxResult<()> {
    let mut children = fs.read_dir(directory)?;
    children.sort();
    for child in children {
        let meta = fs.entry_meta(&child)?;
        let relative = child.strip_prefix(root).map_err(|_| {
            invalid(format!(
                "the ADD path {} leaves its source",
                child.display()
            ))
        })?;
        let relative = guest_relative(relative)?;
        let guest = join_guest(guest_root, &relative);
        match meta.kind {
            EntryKind::Directory => {
                entries.push(TarSpec {
                    path: guest.clone(),
                    kind: TreeKind::Dir,
                    mode: 0o755,
                    size: 0,
                    source: TarSource::Empty,
                });
                tree.push(TreeEntry {
                    path: relative,
                    kind: TreeKind::Dir,
                    mode: 0o755,
                    size: 0,
                    content: String::new(),
                });
                walk_directory(fs, root, &child, guest_root, mode, entries, tree)?;
            }
            EntryKind::File => {
                let file_mode = mode.unwrap_or(meta.mode & 0o777);
                let content = hash_file(fs, &child)?;
                entries.push(TarSpec {
                    path: guest,
                    kind: TreeKind::File,
                    mode: file_mode,
                    size: meta.size,
                    source: TarSource::File(child),
                });
                tree.push(TreeEntry {
                    path: relative,
                    kind: TreeKind::File,
                    mode: file_mode,
                    size: meta.size,
                    content,
                });
            }
            EntryKind::Symlink => {
                let target = fs.read_link(&child)?;
                let target = target.to_string_lossy().into_owned();
                entries.push(TarSpec {
                    path: guest,
                    kind: TreeKind::Symlink,
                    mode: 0o777,
                    size: u64::try_from(target.len()).unwrap_or(u64::MAX),
                    source: TarSource::Link(target.clone()),
                });
                tree.push(TreeEntry {
                    path: relative,
                    kind: TreeKind::Symlink,
                    mode: 0o777,
                    size: u64::try_from(target.len()).unwrap_or(u64::MAX),
                    content: target,
                });
            }
            EntryKind::Other => {
                return Err(invalid(format!(
                    "the ADD entry {} is not a file, a directory or a link",
                    child.display()
                )));
            }
        }
    }
    Ok(())
}

/// Pushes one file entry and its tree entry.
fn push_file(
    fs: &Arc<dyn HostFs>,
    entries: &mut Vec<TarSpec>,
    tree: &mut Vec<TreeEntry>,
    path: &Path,
    guest: &str,
    mode: Option<u32>,
) -> SandboxResult<()> {
    let meta = fs.entry_meta(path)?;
    let file_mode = mode.unwrap_or(meta.mode & 0o777);
    let content = hash_file(fs, path)?;
    entries.push(TarSpec {
        path: guest.to_string(),
        kind: TreeKind::File,
        mode: file_mode,
        size: meta.size,
        source: TarSource::File(path.to_path_buf()),
    });
    let name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    tree.push(TreeEntry {
        path: name,
        kind: TreeKind::File,
        mode: file_mode,
        size: meta.size,
        content,
    });
    Ok(())
}

/// Adds the parent directories of one guest path.
fn push_parents(entries: &mut Vec<TarSpec>, guest: &str) {
    let mut current = Path::new(guest).parent();
    let mut parents = Vec::new();
    while let Some(path) = current {
        if path.as_os_str().is_empty() || path == Path::new("/") {
            break;
        }
        parents.push(path.to_string_lossy().into_owned());
        current = path.parent();
    }
    parents.reverse();
    for parent in parents {
        if entries.iter().any(|entry| entry.path == parent) {
            continue;
        }
        entries.push(TarSpec {
            path: parent,
            kind: TreeKind::Dir,
            mode: 0o755,
            size: 0,
            source: TarSource::Empty,
        });
    }
}

/// Joins one guest directory and one relative path.
fn join_guest(base: &str, relative: &str) -> String {
    let base = base.trim_end_matches('/');
    if base.is_empty() {
        format!("/{relative}")
    } else {
        format!("{base}/{relative}")
    }
}

/// The text of one relative host path with `/` separators.
fn guest_relative(path: &Path) -> SandboxResult<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(
                part.to_str()
                    .ok_or_else(|| invalid("an ADD path is not valid UTF-8".to_string()))?
                    .to_string(),
            ),
            Component::CurDir => {}
            _ => {
                return Err(invalid(
                    "an ADD path leaves its source directory".to_string(),
                ))
            }
        }
    }
    Ok(parts.join("/"))
}

/// The SHA-256 of one host file.
fn hash_file(fs: &Arc<dyn HostFs>, path: &Path) -> SandboxResult<String> {
    let mut reader = fs.open_read(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; TAR_CHUNK];
    loop {
        let read = reader.read(&mut buffer).map_err(|error| SandboxError::Io {
            op: "hash an ADD file".to_string(),
            message: format!("{}: {error}", path.display()),
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(sha256_hex(&hasher.finalize()))
}

/// The error of a wrong `ADD` input.
fn invalid(reason: String) -> foundation_errstacks::ErrorTrace<SandboxError> {
    SandboxError::InvalidSpec {
        field: "sandboxfile".to_string(),
        reason,
    }
    .into()
}

/// The state of the tar stream.
enum TarPhase {
    /// The next entry needs its header.
    Entry,
    /// The content of the current file.
    Body,
    /// The zero bytes that align the content to a 512-byte block.
    Pad,
    /// The two zero blocks that end an archive.
    End,
    /// The stream ended.
    Done,
}

/// The tar stream of one plan.
struct TarStream {
    fs: Arc<dyn HostFs>,
    entries: VecDeque<TarSpec>,
    current: Option<TarSpec>,
    reader: Option<Box<dyn Read + Send>>,
    remaining: u64,
    pad: usize,
    phase: TarPhase,
    error: Option<SandboxError>,
}

impl TarStream {
    fn new(entries: Vec<TarSpec>, fs: Arc<dyn HostFs>) -> Self {
        Self {
            fs,
            entries: entries.into(),
            current: None,
            reader: None,
            remaining: 0,
            pad: 0,
            phase: TarPhase::Entry,
            error: None,
        }
    }

    /// The next chunk, or `None` at the end.
    fn next_chunk(&mut self) -> Option<Vec<u8>> {
        loop {
            match self.phase {
                TarPhase::Done => return None,
                TarPhase::Entry => {
                    self.current = None;
                    self.reader = None;
                    let Some(entry) = self.entries.pop_front() else {
                        self.phase = TarPhase::End;
                        continue;
                    };
                    let header = match tar_header(&entry) {
                        Ok(header) => header,
                        Err(error) => {
                            self.error = Some(error.current_context().clone());
                            self.phase = TarPhase::Done;
                            return None;
                        }
                    };
                    if let TarSource::File(path) = &entry.source {
                        match self.fs.open_read(path) {
                            Ok(reader) => {
                                self.remaining = entry.size;
                                self.reader = Some(reader);
                                self.phase = if entry.size == 0 {
                                    TarPhase::Entry
                                } else {
                                    TarPhase::Body
                                };
                            }
                            Err(error) => {
                                self.error = Some(error.current_context().clone());
                                self.phase = TarPhase::Done;
                                return None;
                            }
                        }
                    }
                    self.current = Some(entry);
                    return Some(header.to_vec());
                }
                TarPhase::Body => {
                    let size = self.current.as_ref().map_or(0, |entry| entry.size);
                    let want =
                        usize::try_from(self.remaining.min(TAR_CHUNK as u64)).unwrap_or(TAR_CHUNK);
                    let mut buffer = vec![0_u8; TAR_CHUNK];
                    let read = match self.reader.as_mut() {
                        Some(reader) => reader.read(&mut buffer[..want]),
                        None => Ok(0),
                    };
                    match read {
                        Ok(0) => {
                            self.error = Some(SandboxError::Io {
                                op: "read an ADD file".to_string(),
                                message: "the file is shorter than its size".to_string(),
                            });
                            self.phase = TarPhase::Done;
                            return None;
                        }
                        Ok(read) => {
                            buffer.truncate(read);
                            self.remaining = self
                                .remaining
                                .saturating_sub(u64::try_from(read).unwrap_or(0));
                            if self.remaining == 0 {
                                self.pad = (512 - (size % 512) as usize) % 512;
                                self.phase = TarPhase::Pad;
                            }
                            return Some(buffer);
                        }
                        Err(error) => {
                            self.error = Some(SandboxError::Io {
                                op: "read an ADD file".to_string(),
                                message: error.to_string(),
                            });
                            self.phase = TarPhase::Done;
                            return None;
                        }
                    }
                }
                TarPhase::Pad => {
                    if self.pad == 0 {
                        self.phase = TarPhase::Entry;
                        continue;
                    }
                    let take = self.pad.min(TAR_CHUNK);
                    self.pad -= take;
                    if self.pad == 0 {
                        self.phase = TarPhase::Entry;
                    }
                    return Some(vec![0_u8; take]);
                }
                TarPhase::End => {
                    self.phase = TarPhase::Done;
                    return Some(vec![0_u8; 1024]);
                }
            }
        }
    }
}

impl Stream for TarStream {
    type Item = SandboxResult<Vec<u8>>;

    fn poll_next(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if let Some(error) = this.error.take() {
            return Poll::Ready(Some(Err(error.into())));
        }
        match this.next_chunk() {
            Some(chunk) => Poll::Ready(Some(Ok(chunk))),
            None => match this.error.take() {
                Some(error) => Poll::Ready(Some(Err(error.into()))),
                None => Poll::Ready(None),
            },
        }
    }
}

/// Builds the 512-byte tar header of one entry.
fn tar_header(entry: &TarSpec) -> SandboxResult<[u8; 512]> {
    let mut header = tar::Header::new_gnu();
    let path = entry.path.trim_start_matches('/');
    let entry_type = match entry.kind {
        TreeKind::File => tar::EntryType::Regular,
        TreeKind::Dir => tar::EntryType::Directory,
        TreeKind::Symlink => tar::EntryType::Symlink,
    };
    header.set_entry_type(entry_type);
    header.set_path(path).map_err(|error| SandboxError::Io {
        op: "write a tar header".to_string(),
        message: format!("{path}: {error}"),
    })?;
    header.set_size(if entry.kind == TreeKind::File {
        entry.size
    } else {
        0
    });
    header.set_mode(entry.mode);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header
        .set_username("root")
        .map_err(|error| SandboxError::Io {
            op: "write a tar header".to_string(),
            message: error.to_string(),
        })?;
    header
        .set_groupname("root")
        .map_err(|error| SandboxError::Io {
            op: "write a tar header".to_string(),
            message: error.to_string(),
        })?;
    if let TarSource::Link(target) = &entry.source {
        header
            .set_link_name_literal(target.as_bytes())
            .map_err(|error| SandboxError::Io {
                op: "write a tar link".to_string(),
                message: error.to_string(),
            })?;
    }
    header.set_cksum();
    Ok(*header.as_bytes())
}

/// The limits of one `RUN` step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunLimits {
    /// The most bytes of one input write.
    pub chunk_bytes: usize,
}

/// Writes `script` to the standard input of `handle` and closes it.
///
/// # Errors
/// The error of the input call.
pub async fn write_script(
    stdin: &mut crate::shared::handles::ExecStdin,
    script: &str,
    limits: RunLimits,
) -> SandboxResult<()> {
    let bytes = script.as_bytes();
    let chunk = limits.chunk_bytes.max(1);
    for part in bytes.chunks(chunk) {
        stdin.write(part).await?;
    }
    Ok(())
}

/// A builder of the tar stream for one verified download.
///
/// It is here so the executor names one type for `ADD` and `FETCH`.
pub type StreamFactory = Box<dyn FnOnce(Arc<dyn HostFs>) -> BoxByteStream + Send + 'static>;

/// Makes the factory of one tar plan.
pub fn stream_factory(plan: TarPlan) -> StreamFactory {
    Box::new(move |fs| plan.tar_stream(fs))
}

/// The context and host that a download needs.
pub struct DownloadContext<'a> {
    /// The injected context.
    pub ctx: &'a Ctx,
    /// The host file system.
    pub fs: &'a Arc<dyn HostFs>,
    /// The build limits.
    pub limits: &'a BuildLimits,
}

/// Downloads one `FETCH` file to the verified cache.
///
/// The body goes to `<cache_dir>/fetch/<sha256>` through a temporary file. A
/// file that is already in the cache is used as it is, because its name is its
/// verified SHA-256.
///
/// # Errors
/// [`SandboxError::ArtifactHashMismatch`] when the body does not have the
/// expected hash, and the error of the download or the file write.
pub async fn download(
    context: &DownloadContext<'_>,
    cache_dir: &Path,
    url: &str,
    sha256: &str,
) -> SandboxResult<PathBuf> {
    let fetch_dir = cache_dir.join("fetch");
    context.fs.create_dir_all(&fetch_dir, 0o755)?;
    let target = fetch_dir.join(sha256);
    if context.fs.exists(&target) {
        return Ok(target);
    }
    let temporary = fetch_dir.join(format!("{sha256}.tmp-{}", random_name(context.ctx)?));
    let writer = context.fs.create_file(&temporary)?;
    let timer = context
        .ctx
        .require_env()
        .map_err(|_| SandboxError::HostNotConfigured)?
        .timer();
    let mut limits = crate::shared::limits::ArtifactLimits::default();
    let client = foundation_netio::HttpClientBuilder::new()
        .connect_timeout(limits.connect_timeout)
        .max_redirects(5)
        .refuse_https_downgrade(true)
        .build();
    limits.max_download_bytes = context.limits.max_fetch_bytes;
    let open = crate::native::artifacts::fetch::open(
        &client,
        context.ctx,
        timer,
        url,
        writer,
        sha256,
        &limits,
    )
    .await;
    let mut download = match open {
        Ok(download) => download,
        Err(error) => {
            let _ = context.fs.remove_file(&temporary);
            return Err(error);
        }
    };
    loop {
        match download.step().await {
            Ok(crate::native::artifacts::fetch::Step::Chunk) => {}
            Ok(crate::native::artifacts::fetch::Step::Done) => break,
            Err(error) => {
                let _ = context.fs.remove_file(&temporary);
                return Err(error);
            }
        }
    }
    if let Err(error) = download.finish() {
        let _ = context.fs.remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = context.fs.rename(&temporary, &target) {
        let _ = context.fs.remove_file(&temporary);
        return Err(error);
    }
    Ok(target)
}

/// A fresh random name for one temporary download.
fn random_name(ctx: &Ctx) -> SandboxResult<String> {
    use foundation_compact::rng::Rng as _;
    let rng = ctx.rng();
    let mut rng = rng.lock().map_err(|_| SandboxError::Io {
        op: "name a download".to_string(),
        message: "the random number generator lock is poisoned".to_string(),
    })?;
    Ok(format!("{:016x}", rng.next_u64()))
}
