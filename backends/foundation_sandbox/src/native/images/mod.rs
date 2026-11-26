//! The sandbox image store (feature 12).
//!
//! An image is `<cache_dir>/images/store/<key>/` with `rootfs.ext4` and
//! `manifest.json`. A tag is a file `<cache_dir>/images/tags/<name>` that holds
//! a key. A work image is a file in `<cache_dir>/images/work/` that is not an
//! image yet.
//!
//! A committed image never changes (invariant I1). A commit is atomic: the
//! store writes a temporary directory and renames it into place. A commit of a
//! key that already exists keeps the first image and removes the new work file.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use foundation_compact::rng::Rng as _;
use foundation_context::Ctx;
use futures::future::{select, Either};
use sha2::{Digest as _, Sha256};

use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::{HostFs, ProcessSpec, SandboxHost};
use crate::shared::images::{check_arch, is_image_key, ImageManifest, ImageResolver, IMAGE_SCHEMA};
use crate::shared::spec::ImageRef;
use crate::shared::BoxFuture;

/// The importer that turns a Docker image, a tar file or the Alpine archive
/// into a sandbox image (feature 13).
#[cfg(feature = "images")]
pub mod import;

/// The build file grammar and its executor (feature 14).
#[cfg(feature = "images")]
pub mod build;

/// The build file grammar (feature 14).
#[cfg(feature = "images")]
pub mod sandboxfile;

/// The names of `mkfs.ext4` that the store tries on this host.
const MKFS_CANDIDATES: [&str; 5] = [
    "/sbin/mkfs.ext4",
    "/usr/sbin/mkfs.ext4",
    "/opt/homebrew/opt/e2fsprogs/sbin/mkfs.ext4",
    "/usr/local/opt/e2fsprogs/sbin/mkfs.ext4",
    "/usr/bin/mkfs.ext4",
];

/// The directory of the image store below the cache directory.
const IMAGES: &str = "images";
/// The store directory name.
const STORE: &str = "store";
/// The tags directory name.
const TAGS: &str = "tags";
/// The work directory name.
const WORK: &str = "work";

/// The sandbox image store.
///
/// It holds no mutable state of its own: every image is a file under the cache
/// directory. Two stores over one cache directory see the same images.
#[derive(Clone)]
pub struct ImageStore {
    ctx: Ctx,
    host: SandboxHost,
    root: PathBuf,
}

impl std::fmt::Debug for ImageStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImageStore")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl ImageStore {
    /// The store below the cache directory of `host`.
    ///
    /// It makes the three directories and removes the work images that are
    /// older than `ImageLimits::work_max_age`.
    #[must_use]
    pub fn new(ctx: Ctx, host: SandboxHost) -> Self {
        let root = host.config.cache_dir.join(IMAGES);
        let store = Self { ctx, host, root };
        store.ensure_dirs();
        store.sweep_work();
        store
    }

    fn ensure_dirs(&self) {
        for directory in [STORE, TAGS, WORK] {
            if let Err(error) = self
                .host
                .fs
                .create_dir_all(&self.root.join(directory), 0o755)
            {
                tracing::warn!(
                    directory = %directory,
                    error = %error,
                    "the image store cannot make a directory"
                );
            }
        }
    }

    fn store_dir(&self) -> PathBuf {
        self.root.join(STORE)
    }

    fn tags_dir(&self) -> PathBuf {
        self.root.join(TAGS)
    }

    fn work_dir(&self) -> PathBuf {
        self.root.join(WORK)
    }

    /// The limits of this store.
    fn limits(&self) -> &crate::shared::limits::ImageLimits {
        &self.host.config.image_limits
    }

    /// The configured or discovered `mkfs.ext4` program.
    fn mkfs_path(&self) -> SandboxResult<PathBuf> {
        // An explicit path is the only choice: a caller that sets
        // `EWE_MKFS_EXT4` gets that program or the error.
        if let Some(path) = &self.host.config.mkfs_ext4_path {
            if self.host.fs.exists(path) {
                return Ok(path.clone());
            }
            return Err(missing_mkfs());
        }
        for candidate in MKFS_CANDIDATES {
            let path = PathBuf::from(candidate);
            if self.host.fs.exists(&path) {
                return Ok(path);
            }
        }
        // The host boundary may search `PATH`: the program list above covers
        // the usual system and Homebrew locations, and `PATH` covers the rest.
        if let Some(found) = find_on_path("mkfs.ext4") {
            if self.host.fs.exists(&found) {
                return Ok(found);
            }
        }
        Err(missing_mkfs())
    }

    /// Makes one new sparse ext4 work image.
    ///
    /// `size_mib = 0` uses `ImageLimits::default_size_mib`.
    ///
    /// # Errors
    /// [`SandboxError::BinaryNotFound`] when `mkfs.ext4` is absent, and
    /// [`SandboxError::Io`] with the program output tail when it fails.
    pub async fn create_blank(&self, size_mib: u32) -> SandboxResult<PathBuf> {
        let size_mib = if size_mib == 0 {
            self.limits().default_size_mib
        } else {
            size_mib
        };
        let mkfs = self.mkfs_path()?;
        let path = self
            .work_dir()
            .join(format!("{}.ext4", self.random_name()?));
        // Make the file before the program runs. `mkfs.ext4` writes only
        // metadata, so the file stays sparse.
        drop(self.host.fs.create_file(&path)?);
        let output = self.run_mkfs(&mkfs, &path, size_mib).await;
        match output {
            Ok(()) => Ok(path),
            Err(error) => {
                let _ = self.host.fs.remove_file(&path);
                Err(error)
            }
        }
    }

    async fn run_mkfs(&self, mkfs: &Path, path: &Path, size_mib: u32) -> SandboxResult<()> {
        let dir = self.work_dir().join(format!("{}.run", self.random_name()?));
        self.host.fs.create_dir_all(&dir, 0o700)?;
        let stdout = dir.join("stdout");
        let stderr = dir.join("stderr");
        let spec = ProcessSpec {
            program: mkfs.to_path_buf(),
            args: vec![
                "-q".to_string(),
                "-F".to_string(),
                "-m".to_string(),
                "0".to_string(),
                "-E".to_string(),
                "root_owner=0:0".to_string(),
                path.display().to_string(),
                format!("{size_mib}M"),
            ],
            env: Vec::new(),
            cwd: dir.clone(),
            stdout: stdout.clone(),
            stderr: stderr.clone(),
            kill_with_parent: true,
            extra_fds: Vec::new(),
        };
        let child = self.host.processes.spawn(spec)?;
        let deadline = self.ctx.clock().now() + self.host.config.artifact_limits.idle_timeout;
        let mut tail = Vec::new();
        let exit = {
            let timer = self.ctx.require_env().ok().map(|env| env.timer());
            match timer {
                Some(timer) => {
                    match select(child.wait(), timer.sleep_until(self.ctx.clone(), deadline)).await
                    {
                        Either::Left((exit, _)) => exit?,
                        Either::Right(_) => {
                            let _ = child.signal(libc::SIGKILL);
                            let _ = child.wait().await;
                            let _ = self.host.fs.remove_dir_all(&dir);
                            return Err(SandboxError::Io {
                                op: "run mkfs.ext4".to_string(),
                                message: "mkfs.ext4 did not finish before the deadline".to_string(),
                            }
                            .into());
                        }
                    }
                }
                None => child.wait().await?,
            }
        };
        tail.extend_from_slice(&self.host.fs.tail(&stdout, 4096).unwrap_or_default());
        tail.extend_from_slice(&self.host.fs.tail(&stderr, 4096).unwrap_or_default());
        let _ = self.host.fs.remove_dir_all(&dir);
        if exit.code == Some(0) {
            Ok(())
        } else {
            Err(SandboxError::Io {
                op: "run mkfs.ext4".to_string(),
                message: format!(
                    "{} exited with {:?}: {}",
                    mkfs.display(),
                    exit.code.or(exit.signal),
                    String::from_utf8_lossy(&tail).trim()
                ),
            }
            .into())
        }
    }

    /// Copies an image or a work image to a new work image.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the source cannot be read or the copy fails.
    #[allow(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "every store call is async, so a caller awaits one shape (root rule 6)"
    )]
    pub async fn snapshot(&self, source: &Path) -> SandboxResult<PathBuf> {
        if !self.host.fs.exists(source) {
            return Err(SandboxError::NotFound {
                what: source.display().to_string(),
            }
            .into());
        }
        let path = self
            .work_dir()
            .join(format!("{}.ext4", self.random_name()?));
        self.host.fs.clone_file(source, &path)?;
        Ok(path)
    }

    /// Moves a work image into the store and writes its manifest.
    ///
    /// A key that exists keeps the first image: the store removes `work` and
    /// returns the existing reference, so a commit is idempotent.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when a file cannot be read, hashed or moved.
    #[allow(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "every store call is async, so a caller awaits one shape (root rule 6)"
    )]
    pub async fn commit(
        &self,
        work: PathBuf,
        manifest: ImageManifest,
        tag: Option<&str>,
    ) -> SandboxResult<ImageRef> {
        let (sha256, size) = self.hash_file(&work)?;
        let key: String = sha256[..32].to_string();
        self.commit_as(&work, manifest, tag, key, sha256, size)
    }

    /// Moves a work image into the store under the content-derived key `key`.
    ///
    /// The importer computes this key from the source identity, so a second
    /// import of the same source finds the image before it exports or
    /// unpacks anything (feature 13, invariant I3). The manifest keeps the
    /// SHA-256 and the size of the work file.
    ///
    /// # Errors
    /// [`SandboxError::InvalidSpec`] when `key` is not a 32-character key, and
    /// [`SandboxError::Io`] when a file cannot be read, hashed or moved.
    #[allow(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "every store call is async, so a caller awaits one shape (root rule 6)"
    )]
    pub async fn commit_keyed(
        &self,
        work: PathBuf,
        manifest: ImageManifest,
        tag: Option<&str>,
        key: String,
    ) -> SandboxResult<ImageRef> {
        if !is_image_key(&key) {
            return Err(SandboxError::InvalidSpec {
                field: "image".to_string(),
                reason: format!("an image key is 32 lower-case hexadecimal characters: {key}"),
            }
            .into());
        }
        let (sha256, size) = self.hash_file(&work)?;
        self.commit_as(&work, manifest, tag, key, sha256, size)
    }

    /// The common body of [`Self::commit`] and [`Self::commit_keyed`].
    fn commit_as(
        &self,
        work: &Path,
        mut manifest: ImageManifest,
        tag: Option<&str>,
        key: String,
        sha256: String,
        size: u64,
    ) -> SandboxResult<ImageRef> {
        manifest.schema = IMAGE_SCHEMA;
        manifest.key.clone_from(&key);
        manifest.rootfs_sha256 = sha256;
        manifest.rootfs_size = size;
        if manifest.created_unix_ms == 0 {
            manifest.created_unix_ms = created_unix_ms(&self.ctx);
        }

        let store = self.store_dir().join(&key);
        if self.host.fs.exists(&store) {
            let _ = self.host.fs.remove_file(work);
            if let Some(tag) = tag {
                self.write_tag(tag, &key)?;
                return Ok(ImageRef::Tag(tag.to_string()));
            }
            return Ok(ImageRef::Key(key));
        }

        let temporary = self
            .store_dir()
            .join(format!("{}.tmp-{}", key, self.random_name()?));
        let prepared = (|| -> SandboxResult<()> {
            self.host.fs.create_dir_all(&temporary, 0o755)?;
            self.host.fs.rename(work, &temporary.join("rootfs.ext4"))?;
            let json = serde_json::to_vec_pretty(&manifest).map_err(|error| SandboxError::Io {
                op: "encode the image manifest".to_string(),
                message: error.to_string(),
            })?;
            self.host
                .fs
                .write(&temporary.join("manifest.json"), &json)?;
            // Two builders can commit the same key at the same time. The
            // second rename finds the first image; its temporary directory
            // goes, and the first image stays (invariant I11).
            match self.host.fs.rename(&temporary, &store) {
                Ok(()) => Ok(()),
                Err(_) if self.host.fs.exists(&store) => {
                    let _ = self.host.fs.remove_dir_all(&temporary);
                    Ok(())
                }
                Err(error) => Err(error),
            }
        })();
        if let Err(error) = prepared {
            let _ = self.host.fs.remove_dir_all(&temporary);
            let _ = self.host.fs.remove_file(work);
            return Err(error);
        }
        if let Some(tag) = tag {
            self.write_tag(tag, &key)?;
            Ok(ImageRef::Tag(tag.to_string()))
        } else {
            Ok(ImageRef::Key(key))
        }
    }

    /// The manifest of the image `key`, when the store has it.
    ///
    /// The importer uses it for the cache check before it exports the source
    /// (feature 13, invariant I3).
    #[allow(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "every store call is async, so a caller awaits one shape (root rule 6)"
    )]
    pub async fn manifest_of(&self, key: &str) -> Option<ImageManifest> {
        if !is_image_key(key) {
            return None;
        }
        let store = self.store_dir().join(key);
        if !self.host.fs.exists(&store.join("manifest.json")) {
            return None;
        }
        read_manifest(&self.host.fs, &store).ok()
    }

    /// Points `tag` at the existing image `key`.
    ///
    /// # Errors
    /// [`SandboxError::NotFound`] when the store has no image `key`, and
    /// [`SandboxError::InvalidSpec`] when `tag` is not a plain file name.
    #[allow(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "every store call is async, so a caller awaits one shape (root rule 6)"
    )]
    pub async fn set_tag(&self, key: &str, tag: &str) -> SandboxResult<()> {
        if !self.host.fs.exists(&self.store_dir().join(key)) {
            return Err(SandboxError::NotFound {
                what: format!("image key {key}"),
            }
            .into());
        }
        self.write_tag(tag, key)
    }

    /// Whether `image` names an image in this store.
    ///
    /// This is a direct file check, not a call: the command line reads it
    /// while it builds a spec (feature 13, section 6.5).
    #[must_use]
    pub fn has(&self, image: &ImageRef) -> bool {
        self.key_of(image)
            .is_ok_and(|key| self.host.fs.exists(&self.store_dir().join(key)))
    }

    /// Resolves one reference to its root file and manifest.
    ///
    /// # Errors
    /// [`SandboxError::NotFound`] for an unknown tag or key, and
    /// [`SandboxError::Io`] when a file cannot be read or parsed.
    #[allow(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "every store call is async, so a caller awaits one shape (root rule 6)"
    )]
    pub async fn resolve(&self, image: &ImageRef) -> SandboxResult<(PathBuf, ImageManifest)> {
        let key = self.key_of(image)?;
        let store = self.store_dir().join(&key);
        if !self.host.fs.exists(&store) {
            return Err(SandboxError::NotFound {
                what: image_name(image),
            }
            .into());
        }
        let manifest = read_manifest(&self.host.fs, &store)?;
        manifest.check_schema()?;
        check_arch(&manifest)?;
        Ok((store.join("rootfs.ext4"), manifest))
    }

    /// Every committed image, in directory order.
    ///
    /// A directory without a manifest, such as a temporary directory of a
    /// commit that a crash cut short, is skipped.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the store cannot be listed.
    #[allow(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "every store call is async, so a caller awaits one shape (root rule 6)"
    )]
    pub async fn list(&self) -> SandboxResult<Vec<ImageManifest>> {
        let mut manifests = Vec::new();
        for entry in self.host.fs.read_dir(&self.store_dir())? {
            if !is_image_key(
                entry
                    .file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .unwrap_or_default(),
            ) {
                continue;
            }
            if !self.host.fs.exists(&entry.join("manifest.json")) {
                continue;
            }
            if let Ok(manifest) = read_manifest(&self.host.fs, &entry) {
                manifests.push(manifest);
            }
        }
        manifests.sort_by(|left, right| left.key.cmp(&right.key));
        Ok(manifests)
    }

    /// Removes one image and every tag that names it.
    ///
    /// # Errors
    /// [`SandboxError::NotFound`] for an unknown reference, and
    /// [`SandboxError::Io`] when a file cannot be removed.
    #[allow(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "every store call is async, so a caller awaits one shape (root rule 6)"
    )]
    pub async fn remove(&self, image: &ImageRef) -> SandboxResult<()> {
        let key = self.key_of(image)?;
        let store = self.store_dir().join(&key);
        if !self.host.fs.exists(&store) {
            return Err(SandboxError::NotFound {
                what: image_name(image),
            }
            .into());
        }
        self.host.fs.remove_dir_all(&store)?;
        for tag in self.host.fs.read_dir(&self.tags_dir())? {
            let names_key = self
                .host
                .fs
                .read(&tag, 64)
                .is_ok_and(|body| body == key.as_bytes());
            if names_key {
                self.host.fs.remove_file(&tag)?;
            }
        }
        Ok(())
    }

    /// The resolution of one reference, without the arch check.
    fn key_of(&self, image: &ImageRef) -> SandboxResult<String> {
        match image {
            ImageRef::Key(key) => {
                if is_image_key(key) {
                    Ok(key.clone())
                } else {
                    Err(SandboxError::InvalidSpec {
                        field: "image".to_string(),
                        reason: format!(
                            "an image key is 32 lower-case hexadecimal characters: {key}"
                        ),
                    }
                    .into())
                }
            }
            ImageRef::Tag(name) => {
                if !valid_tag(name) {
                    return Err(SandboxError::InvalidSpec {
                        field: "image".to_string(),
                        reason: format!("an image tag is a plain file name: {name}"),
                    }
                    .into());
                }
                let path = self.tags_dir().join(name);
                let body = self
                    .host
                    .fs
                    .read(&path, 64)
                    .map_err(|_| SandboxError::NotFound {
                        what: format!("image tag {name}"),
                    })?;
                let key = String::from_utf8_lossy(&body).trim().to_string();
                if is_image_key(&key) {
                    Ok(key)
                } else {
                    Err(SandboxError::Io {
                        op: "read an image tag".to_string(),
                        message: format!("the tag {name} does not name a key"),
                    }
                    .into())
                }
            }
        }
    }

    /// Writes one tag through a temporary file and a rename.
    fn write_tag(&self, name: &str, key: &str) -> SandboxResult<()> {
        if !valid_tag(name) {
            return Err(SandboxError::InvalidSpec {
                field: "tag".to_string(),
                reason: format!("a tag is a plain file name: {name}"),
            }
            .into());
        }
        let target = self.tags_dir().join(name);
        let temporary = self
            .tags_dir()
            .join(format!(".{name}.tmp-{}", self.random_name()?));
        self.host.fs.write(&temporary, key.as_bytes())?;
        self.host.fs.rename(&temporary, &target)
    }

    /// Streams one file through SHA-256 and returns `(hex, bytes)`.
    fn hash_file(&self, path: &Path) -> SandboxResult<(String, u64)> {
        let mut reader = self.host.fs.read_stream(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; 64 * 1024];
        let mut size = 0_u64;
        loop {
            let read = reader.read(&mut buffer).map_err(|error| SandboxError::Io {
                op: "hash an image".to_string(),
                message: format!("{}: {error}", path.display()),
            })?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            size = size.saturating_add(read as u64);
        }
        Ok((hex(&hasher.finalize()), size))
    }

    /// A fresh random name for one file below the store.
    fn random_name(&self) -> SandboxResult<String> {
        let rng = self.ctx.rng();
        let mut rng = rng.lock().map_err(|_| SandboxError::Io {
            op: "name an image file".to_string(),
            message: "the random number generator lock is poisoned".to_string(),
        })?;
        Ok(format!("{:016x}", rng.next_u64()))
    }

    /// Removes the work images and temporary directories that are too old.
    fn sweep_work(&self) {
        let max_age = self.limits().work_max_age;
        let now = self.ctx.clock().wall_now();
        for entry in self.host.fs.read_dir(&self.work_dir()).unwrap_or_default() {
            let Some(modified) = self.host.fs.modified(&entry) else {
                continue;
            };
            let Ok(age) = now.duration_since(modified) else {
                continue;
            };
            if age < max_age {
                continue;
            }
            if self.host.fs.remove_file(&entry).is_err() {
                let _ = self.host.fs.remove_dir_all(&entry);
            }
        }
        // A crash between the temporary directory and the rename leaves a
        // `<key>.tmp-*` directory. It is never an image; remove an old one.
        for entry in self.host.fs.read_dir(&self.store_dir()).unwrap_or_default() {
            let name = entry
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or_default();
            if !name.contains(".tmp-") {
                continue;
            }
            let Some(modified) = self.host.fs.modified(&entry) else {
                continue;
            };
            if now.duration_since(modified).is_ok_and(|age| age >= max_age) {
                let _ = self.host.fs.remove_dir_all(&entry);
            }
        }
    }
}

impl ImageResolver for ImageStore {
    fn resolve<'a>(
        &'a self,
        image: &'a ImageRef,
    ) -> BoxFuture<'a, SandboxResult<(PathBuf, ImageManifest)>> {
        Box::pin(async move { ImageStore::resolve(self, image).await })
    }
}

/// Reads and parses the manifest of one image directory.
fn read_manifest(fs: &std::sync::Arc<dyn HostFs>, store: &Path) -> SandboxResult<ImageManifest> {
    let path = store.join("manifest.json");
    let body = fs.read(&path, 1024 * 1024)?;
    serde_json::from_slice(&body).map_err(|error| {
        SandboxError::Io {
            op: "read an image manifest".to_string(),
            message: format!("{}: {error}", path.display()),
        }
        .into()
    })
}

/// The error of a host without `mkfs.ext4`.
fn missing_mkfs() -> foundation_errstacks::ErrorTrace<SandboxError> {
    // The error carries the program name, so the host name carries the hint
    // that a macOS host needs.
    let name = if cfg!(target_os = "macos") {
        "mkfs.ext4 (brew install e2fsprogs)".to_string()
    } else {
        "mkfs.ext4".to_string()
    };
    SandboxError::BinaryNotFound { name }.into()
}

/// Whether a tag is a plain file name that stays in the tags directory.
fn valid_tag(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\0')
}

/// The human name of one reference.
fn image_name(image: &ImageRef) -> String {
    match image {
        ImageRef::Tag(name) => format!("image tag {name}"),
        ImageRef::Key(key) => format!("image key {key}"),
    }
}

/// The lower-case hexadecimal text of 32 bytes.
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    text
}

/// Finds one program on `PATH`.
///
/// This is a host boundary read: only the native image store calls it.
fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// The creation time of a manifest as Unix milliseconds.
fn created_unix_ms(ctx: &Ctx) -> u64 {
    let wall = ctx.clock().wall_now();
    wall.duration_since(foundation_compact::SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}
