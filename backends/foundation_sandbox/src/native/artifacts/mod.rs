//! The guest-artifact store.
//!
//! A sandbox needs files for the guest: a kernel, a root file system and an
//! initramfs. The store keeps them in a host cache with a checked Secure Hash
//! Algorithm 256 (SHA-256).
//!
//! The cache layout is `<cache_dir>/<kind>/<first 16 hex digits>/`. An entry is
//! a directory that holds the payload (`artifact`, `root` or `initramfs.cpio`)
//! and a `.ready` marker. `ensure` builds into a sibling
//! `<entry>.tmp-<random>` directory and renames it. The marker is written
//! inside the temporary entry before the rename, so a directory that has the
//! entry name is complete (section 3.1 of the feature).
//!
//! The module is native code: it uses the HTTP client, `flate2` and `tar`.

pub mod cpio;
mod fetch;
pub mod manifest;
mod tarball;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll};

use foundation_context::env::Timer;
use foundation_context::Ctx;
use foundation_netio::{DynNetClient, HttpClientBuilder};
use futures::Stream;
use serde::{Deserialize, Serialize};

use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::{HostFs, SandboxHost};
use crate::shared::spec::GuestArch;

pub use crate::shared::limits::ArtifactLimits;
pub use cpio::{write_newc, CpioEntry};
pub use fetch::sha256_hex;
pub use manifest::{
    alpine_minirootfs, alpine_minirootfs_archive, firecracker_ci_rootfs, firecracker_kernel,
};

/// The marker of a complete entry.
pub const READY_MARKER: &str = ".ready";

/// The payload name of a downloaded file entry.
pub const FILE_PAYLOAD: &str = "artifact";

/// The payload name of an unpacked tree entry.
pub const TREE_PAYLOAD: &str = "root";

/// The name of the initramfs payload.
pub const INITRAMFS_PAYLOAD: &str = "initramfs.cpio";

/// The name of the archive inside a temporary tree entry.
const TMP_ARCHIVE: &str = "archive.tar.gz";

/// What one artifact spec produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// The entry is the downloaded file.
    File,
    /// The entry is the unpacked tree of a `.tar.gz` archive.
    TarGzDir,
}

impl ArtifactKind {
    /// The cache directory name of this kind.
    #[must_use]
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::TarGzDir => "tar-gz-dir",
        }
    }

    /// The payload name inside the entry.
    #[must_use]
    pub fn payload_name(self) -> &'static str {
        match self {
            Self::File => FILE_PAYLOAD,
            Self::TarGzDir => TREE_PAYLOAD,
        }
    }
}

/// One artifact to fetch or build.
///
/// The manifest pins the known ones. A caller can pass its own spec to
/// [`ArtifactStore::ensure`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSpec {
    /// The display name, used in error messages.
    pub name: String,
    /// The download address.
    pub url: String,
    /// The expected SHA-256, in lowercase hex.
    pub sha256: String,
    /// What the entry holds.
    pub kind: ArtifactKind,
}

impl ArtifactSpec {
    /// Checks the fields that the store reads.
    fn validate(&self) -> SandboxResult<()> {
        if self.name.is_empty() {
            return Err(invalid("name", "the name is empty"));
        }
        if self.url.is_empty() {
            return Err(invalid("url", "the address is empty"));
        }
        if self.sha256.len() != 64 || !self.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid("sha256", "the hash is not 64 hexadecimal digits"));
        }
        Ok(())
    }
}

/// An invalid spec error.
fn invalid(field: &str, reason: &str) -> foundation_errstacks::ErrorTrace<SandboxError> {
    SandboxError::InvalidSpec {
        field: field.to_string(),
        reason: reason.to_string(),
    }
    .into()
}

/// One stage of an artifact build.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactProgress {
    /// The body is arriving. `total` is the server `Content-Length`.
    Downloading {
        /// The bytes that arrived.
        done: u64,
        /// The body size, when the server gave it.
        total: Option<u64>,
    },
    /// The body ended and the store checks its hash.
    Verifying,
    /// The archive unpacks into the entry.
    Unpacking,
    /// The store builds the payload, for example the initramfs.
    Building,
}

/// One item of [`ArtifactStream`]: a progress stage, then the ready path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtifactUpdate {
    /// A stage of the build.
    Progress(ArtifactProgress),
    /// The path of the ready entry. It is the last item.
    Ready(PathBuf),
}

/// The progress stream of one [`ArtifactStore::ensure_with_progress`] call.
///
/// The stream runs the same steps as `ensure`. It yields each
/// [`ArtifactUpdate::Progress`] and one final [`ArtifactUpdate::Ready`]. An
/// error ends the stream.
pub struct ArtifactStream(
    Pin<Box<dyn Stream<Item = SandboxResult<ArtifactUpdate>> + Send + 'static>>,
);

impl ArtifactStream {
    /// Wraps the inner async stream.
    #[must_use]
    pub fn new(
        inner: Pin<Box<dyn Stream<Item = SandboxResult<ArtifactUpdate>> + Send + 'static>>,
    ) -> Self {
        Self(inner)
    }
}

impl Stream for ArtifactStream {
    type Item = SandboxResult<ArtifactUpdate>;

    fn poll_next(self: Pin<&mut Self>, context: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().0.as_mut().poll_next(context)
    }
}

/// The phases of one build.
enum Phase {
    /// The HTTP response is open and the body is arriving.
    Fetch(Box<fetch::Download>),
    /// The body ended. The hash check happens here.
    Verify(Box<fetch::Download>),
    /// The archive unpacks into the entry.
    Unpack,
    /// The marker is written and the temporary entry is renamed.
    Commit,
    /// The entry is ready.
    Done,
}

/// One build of one cache entry.
struct Job {
    spec: ArtifactSpec,
    entry: PathBuf,
    tmp: PathBuf,
    payload: PathBuf,
    phase: Phase,
}

/// The paths of one entry.
struct Paths {
    entry: PathBuf,
    tmp: PathBuf,
    payload: PathBuf,
}

/// The state of the progress stream.
enum StreamState {
    /// The build has not started.
    Start(ArtifactSpec),
    /// The build is in progress.
    Running(Box<Job>),
    /// The stream ended.
    End,
}

impl Job {
    /// Begins the build: checks the ready entry, makes the temporary directory
    /// and opens the download.
    async fn begin(
        store: &ArtifactStore,
        spec: ArtifactSpec,
    ) -> SandboxResult<(ArtifactUpdate, Job)> {
        spec.validate()?;
        let paths = store.paths(&spec)?;
        let mut job = Job {
            spec,
            entry: paths.entry,
            tmp: paths.tmp,
            payload: paths.payload,
            phase: Phase::Done,
        };
        if store.fs.exists(&job.entry.join(READY_MARKER)) {
            return Ok((ArtifactUpdate::Ready(job.payload.clone()), job));
        }
        let result = job.start_download(store).await;
        match result {
            Ok(update) => Ok((update, job)),
            Err(error) => {
                job.cleanup(store);
                Err(error)
            }
        }
    }

    /// Makes the temporary directory and opens the download.
    async fn start_download(&mut self, store: &ArtifactStore) -> SandboxResult<ArtifactUpdate> {
        if let Some(parent) = self.entry.parent() {
            store.fs.create_dir_all(parent, 0o755)?;
        }
        store.fs.create_dir_all(&self.tmp, 0o755)?;
        let download_path = match self.spec.kind {
            ArtifactKind::File => self.tmp.join(FILE_PAYLOAD),
            ArtifactKind::TarGzDir => self.tmp.join(TMP_ARCHIVE),
        };
        let writer = store.fs.create_file(&download_path)?;
        let timer = store.timer()?;
        let download = fetch::open(
            &store.client,
            &store.ctx,
            timer,
            &self.spec.url,
            writer,
            &self.spec.sha256,
            &store.limits,
        )
        .await?;
        let update = ArtifactUpdate::Progress(ArtifactProgress::Downloading {
            done: 0,
            total: download.total(),
        });
        self.phase = Phase::Fetch(Box::new(download));
        Ok(update)
    }

    /// Runs one step of the build.
    async fn step(&mut self, store: &ArtifactStore) -> SandboxResult<ArtifactUpdate> {
        match std::mem::replace(&mut self.phase, Phase::Done) {
            Phase::Fetch(mut download) => {
                let step = download.step().await?;
                match step {
                    fetch::Step::Chunk => {
                        let update = ArtifactUpdate::Progress(ArtifactProgress::Downloading {
                            done: download.done(),
                            total: download.total(),
                        });
                        self.phase = Phase::Fetch(download);
                        Ok(update)
                    }
                    fetch::Step::Done => {
                        self.phase = Phase::Verify(download);
                        Ok(ArtifactUpdate::Progress(ArtifactProgress::Verifying))
                    }
                }
            }
            Phase::Verify(mut download) => {
                download.finish()?;
                match self.spec.kind {
                    ArtifactKind::File => {
                        self.phase = Phase::Commit;
                        self.commit(store)
                    }
                    ArtifactKind::TarGzDir => {
                        self.phase = Phase::Unpack;
                        Ok(ArtifactUpdate::Progress(ArtifactProgress::Unpacking))
                    }
                }
            }
            Phase::Unpack => {
                self.unpack(store)?;
                self.phase = Phase::Commit;
                self.commit(store)
            }
            Phase::Commit => {
                self.phase = Phase::Commit;
                self.commit(store)
            }
            Phase::Done => Err(SandboxError::Io {
                op: "artifact build".to_string(),
                message: "the build is already done".to_string(),
            }
            .into()),
        }
    }

    /// Unpacks the downloaded archive into the entry.
    fn unpack(&self, store: &ArtifactStore) -> SandboxResult<()> {
        let archive = self.tmp.join(TMP_ARCHIVE);
        let target = self.tmp.join(TREE_PAYLOAD);
        tarball::unpack(&store.fs, &archive, &target, &store.limits)?;
        // The archive is not part of the entry. The tree is the entry.
        std::fs::remove_file(&archive).map_err(|error| SandboxError::Io {
            op: "remove the unpacked archive".to_string(),
            message: format!("{}: {error}", archive.display()),
        })?;
        Ok(())
    }

    /// Writes the marker and renames the temporary entry.
    fn commit(&mut self, store: &ArtifactStore) -> SandboxResult<ArtifactUpdate> {
        store.fs.write(&self.tmp.join(READY_MARKER), b"")?;
        match store.fs.rename(&self.tmp, &self.entry) {
            Ok(()) => {
                self.phase = Phase::Done;
                Ok(ArtifactUpdate::Ready(self.payload.clone()))
            }
            // Another builder renamed its entry first. Its entry is complete,
            // because it wrote the marker before the rename.
            Err(_) if store.fs.exists(&self.entry.join(READY_MARKER)) => {
                let _ = store.fs.remove_dir_all(&self.tmp);
                self.phase = Phase::Done;
                Ok(ArtifactUpdate::Ready(self.payload.clone()))
            }
            Err(error) => Err(error),
        }
    }

    /// Removes the temporary entry after a failure (invariant I2).
    fn cleanup(&self, store: &ArtifactStore) {
        if store.fs.exists(&self.tmp) {
            let _ = store.fs.remove_dir_all(&self.tmp);
        }
    }
}

/// The cache of guest artifacts.
///
/// The store holds the `Ctx`, the host file system, the cache directory and the
/// limits. A clone is cheap.
#[derive(Clone)]
pub struct ArtifactStore {
    ctx: Ctx,
    fs: Arc<dyn HostFs>,
    root: PathBuf,
    limits: ArtifactLimits,
    client: DynNetClient,
}

impl std::fmt::Debug for ArtifactStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ArtifactStore")
            .field("root", &self.root)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl ArtifactStore {
    /// A store over `cache_dir`.
    ///
    /// `fs` is the host file system and `cache_dir` is `HostConfig.cache_dir`.
    /// The limits come from the `SandboxHost` bundle of `ctx` when it is there,
    /// and from [`ArtifactLimits::default`] otherwise.
    #[must_use]
    pub fn new(ctx: Ctx, fs: Arc<dyn HostFs>, cache_dir: PathBuf) -> Self {
        let limits = SandboxHost::from_ctx(&ctx).map_or_else(
            |_| ArtifactLimits::default(),
            |host| host.config.artifact_limits.clone(),
        );
        let client = HttpClientBuilder::new()
            .connect_timeout(limits.connect_timeout)
            .build();
        Self {
            ctx,
            fs,
            root: cache_dir,
            limits,
            client,
        }
    }

    /// A store with the given limits.
    #[must_use]
    pub fn with_limits(mut self, limits: ArtifactLimits) -> Self {
        self.client = HttpClientBuilder::new()
            .connect_timeout(limits.connect_timeout)
            .build();
        self.limits = limits;
        self
    }

    /// The limits of this store.
    #[must_use]
    pub fn limits(&self) -> &ArtifactLimits {
        &self.limits
    }

    /// The cache directory.
    #[must_use]
    pub fn cache_dir(&self) -> &Path {
        &self.root
    }

    /// Returns the path of a ready artifact. It downloads or builds the
    /// artifact when the cache does not have it.
    ///
    /// # Errors
    /// [`SandboxError::ArtifactHashMismatch`],
    /// [`SandboxError::ArtifactUnsafePath`], [`SandboxError::ArtifactTooLarge`]
    /// and [`SandboxError::Io`] for a failed build.
    pub async fn ensure(&self, spec: &ArtifactSpec) -> SandboxResult<PathBuf> {
        let (mut update, mut job) = Job::begin(self, spec.clone()).await?;
        loop {
            match update {
                ArtifactUpdate::Ready(path) => return Ok(path),
                ArtifactUpdate::Progress(_) => {}
            }
            match job.step(self).await {
                Ok(next) => update = next,
                Err(error) => {
                    job.cleanup(self);
                    return Err(error);
                }
            }
        }
    }

    /// The same as [`Self::ensure`], with progress.
    #[must_use]
    pub fn ensure_with_progress(&self, spec: ArtifactSpec) -> ArtifactStream {
        let store = self.clone();
        let stream = futures::stream::unfold(StreamState::Start(spec), move |state| {
            let store = store.clone();
            async move {
                match state {
                    StreamState::Start(spec) => match Job::begin(&store, spec).await {
                        Ok((update, job)) => {
                            let next = match update {
                                ArtifactUpdate::Ready(_) => StreamState::End,
                                ArtifactUpdate::Progress(_) => StreamState::Running(Box::new(job)),
                            };
                            Some((Ok(update), next))
                        }
                        Err(error) => Some((Err(error), StreamState::End)),
                    },
                    StreamState::Running(mut job) => match job.step(&store).await {
                        Ok(update) => {
                            let next = match update {
                                ArtifactUpdate::Ready(_) => StreamState::End,
                                ArtifactUpdate::Progress(_) => StreamState::Running(job),
                            };
                            Some((Ok(update), next))
                        }
                        Err(error) => {
                            job.cleanup(&store);
                            Some((Err(error), StreamState::End))
                        }
                    },
                    StreamState::End => None,
                }
            }
        });
        ArtifactStream::new(Box::pin(stream))
    }

    /// The Alpine minirootfs of `arch`, unpacked.
    ///
    /// # Errors
    /// The errors of [`Self::ensure`].
    pub async fn alpine_rootfs_dir(&self, arch: GuestArch) -> SandboxResult<PathBuf> {
        self.ensure(&manifest::alpine_minirootfs(arch).spec()).await
    }

    /// The Alpine archive file itself, for the image importer of feature 13.
    ///
    /// # Errors
    /// The errors of [`Self::ensure`].
    pub async fn alpine_rootfs_archive(&self, arch: GuestArch) -> SandboxResult<PathBuf> {
        self.ensure(&manifest::alpine_minirootfs_archive(arch).spec())
            .await
    }

    /// The Firecracker CI root file system of `arch`, as a squashfs file.
    ///
    /// # Errors
    /// The errors of [`Self::ensure`].
    pub async fn firecracker_ci_rootfs(&self, arch: GuestArch) -> SandboxResult<PathBuf> {
        self.ensure(&manifest::firecracker_ci_rootfs(arch).spec())
            .await
    }

    /// The Firecracker CI kernel of `arch`.
    ///
    /// # Errors
    /// The errors of [`Self::ensure`].
    pub async fn firecracker_kernel(&self, arch: GuestArch) -> SandboxResult<PathBuf> {
        self.ensure(&manifest::firecracker_kernel(arch).spec())
            .await
    }

    /// The pinned kernel whose manifest name is `name`, for this architecture.
    ///
    /// A name that is not a pinned kernel of `arch` fails with
    /// [`SandboxError::BinaryNotFound`], so a manifest cannot name an artifact
    /// that this host does not verify.
    ///
    /// # Errors
    /// [`SandboxError::BinaryNotFound`] for an unknown name, and the errors of
    /// [`Self::ensure`].
    pub async fn firecracker_kernel_named(
        &self,
        name: &str,
        arch: GuestArch,
    ) -> SandboxResult<PathBuf> {
        let pinned = manifest::firecracker_kernel(arch);
        if pinned.name != name {
            return Err(SandboxError::BinaryNotFound {
                name: name.to_string(),
            }
            .into());
        }
        self.firecracker_kernel(arch).await
    }

    /// The slave initramfs: a `newc` archive with the slave as `/init`.
    ///
    /// The cache key is the SHA-256 of the slave file, so the same slave gives
    /// the same archive and the same path.
    ///
    /// # Errors
    /// [`SandboxError::ArtifactTooLarge`] when the slave file is too large, and
    /// [`SandboxError::Io`] for a failed read or write.
    #[allow(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "the public API is async (root rule 6); the build is a file copy"
    )]
    pub async fn slave_initramfs(&self, slave_path: &Path) -> SandboxResult<PathBuf> {
        let max = usize::try_from(self.limits.max_download_bytes).unwrap_or(usize::MAX);
        let slave = self.fs.read(slave_path, max)?;
        if slave.len() as u64 >= self.limits.max_download_bytes {
            return Err(SandboxError::ArtifactTooLarge {
                limit: self.limits.max_download_bytes,
                message: format!("the slave file {} is too large", slave_path.display()),
            }
            .into());
        }
        let hash = sha256_hex(&slave);
        let entry = self.root.join("initramfs").join(&hash[..16]);
        let payload = entry.join(INITRAMFS_PAYLOAD);
        if self.fs.exists(&entry.join(READY_MARKER)) {
            return Ok(payload);
        }
        let tmp = self.temp_beside(&entry)?;
        let build = (|| -> SandboxResult<()> {
            if let Some(parent) = entry.parent() {
                self.fs.create_dir_all(parent, 0o755)?;
            }
            self.fs.create_dir_all(&tmp, 0o755)?;
            let entries = cpio::initramfs_entries(&slave);
            let mut writer = self.fs.create_file(&tmp.join(INITRAMFS_PAYLOAD))?;
            write_newc(&entries, &mut writer).map_err(|error| SandboxError::Io {
                op: "write the initramfs".to_string(),
                message: error.to_string(),
            })?;
            writer.flush().map_err(|error| SandboxError::Io {
                op: "write the initramfs".to_string(),
                message: error.to_string(),
            })?;
            drop(writer);
            self.fs.write(&tmp.join(READY_MARKER), b"")?;
            match self.fs.rename(&tmp, &entry) {
                Ok(()) => Ok(()),
                // Another build made the entry first; its copy is complete.
                Err(_) if self.fs.exists(&entry.join(READY_MARKER)) => {
                    let _ = self.fs.remove_dir_all(&tmp);
                    Ok(())
                }
                Err(error) => Err(error),
            }
        })();
        match build {
            Ok(()) => Ok(payload),
            Err(error) => {
                let _ = self.fs.remove_dir_all(&tmp);
                Err(error)
            }
        }
    }

    /// The paths of the entry and its temporary sibling.
    fn paths(&self, spec: &ArtifactSpec) -> SandboxResult<Paths> {
        let entry = self
            .root
            .join(spec.kind.dir_name())
            .join(&spec.sha256[..16]);
        let payload = entry.join(spec.kind.payload_name());
        let tmp = self.temp_beside(&entry)?;
        Ok(Paths {
            entry,
            tmp,
            payload,
        })
    }

    /// A fresh temporary sibling of `entry`, named with the entropy of the
    /// context.
    fn temp_beside(&self, entry: &Path) -> SandboxResult<PathBuf> {
        let suffix = {
            use foundation_compact::rng::Rng;
            let rng = self.ctx.rng();
            let mut rng = rng.lock().expect("the Ctx RNG lock is not poisoned");
            format!("{:016x}{:016x}", rng.next_u64(), rng.next_u64())
        };
        let name = entry
            .file_name()
            .ok_or_else(|| invalid("cache_dir", "the entry path has no name"))?;
        let mut temp = std::ffi::OsString::from(name);
        temp.push(format!(".tmp-{suffix}"));
        Ok(entry.with_file_name(temp))
    }

    /// The timer of the injected environment.
    fn timer(&self) -> SandboxResult<Arc<dyn Timer>> {
        self.ctx
            .require_env()
            .map(|env| env.timer())
            .map_err(|_| SandboxError::HostNotConfigured.into())
    }
}
