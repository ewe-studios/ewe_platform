//! A sandbox image: its manifest and the way the control plane finds it.
//!
//! An image is one ext4 file plus a [`ImageManifest`]. The store that keeps it
//! is native code (feature 12); this module holds the plain data and the trait
//! that turns an [`ImageRef`] into a file path and a manifest, so the control
//! plane in `shared/` needs no native code.
//!
//! The manifest holds the defaults of a sandbox that boots the image: the
//! command, the environment, the work directory and the user. An explicit spec
//! value wins over a manifest value (invariant I9).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::spec::{GuestArch, ImageRef, KernelSource, RootFs, SandboxSpec};

/// The manifest schema that this crate writes and reads.
pub const IMAGE_SCHEMA: u32 = 1;

/// The default work directory of a sandbox.
pub const DEFAULT_WORKDIR: &str = "/workspace";

/// The defaults and the identity of one sandbox image (feature 12, section
/// 6.2).
///
/// The image store writes one `manifest.json` beside each `rootfs.ext4`. The
/// fields never change after the commit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageManifest {
    /// The schema version. This crate writes [`IMAGE_SCHEMA`].
    pub schema: u32,
    /// The image key: 32 lower-case hexadecimal characters.
    pub key: String,
    /// The architecture of the programs in the image.
    pub arch: GuestArch,
    /// The commit time as Unix milliseconds.
    pub created_unix_ms: u64,
    /// The start command, or `None` when the image has none.
    pub cmd: Option<Vec<String>>,
    /// The base environment of each command.
    pub env: Vec<(String, String)>,
    /// The default work directory.
    pub workdir: Option<String>,
    /// The default user: `"uid"`, `"uid:gid"`, `"name"` or `"name:group"`.
    pub user: Option<String>,
    /// The manifest name of a feature 03 kernel, for Firecracker.
    pub kernel: Option<String>,
    /// The SHA-256 of `rootfs.ext4`, as 64 lower-case hexadecimal characters.
    pub rootfs_sha256: String,
    /// The size of `rootfs.ext4` in bytes.
    pub rootfs_size: u64,
    /// Free text that names where the image came from.
    pub source: String,
    /// The build file lines, for inspection. It is empty for a converted image.
    pub steps: Vec<String>,
}

impl ImageManifest {
    /// The manifest with every optional default empty.
    #[must_use]
    pub fn new(key: String, arch: GuestArch, rootfs_sha256: String, rootfs_size: u64) -> Self {
        Self {
            schema: IMAGE_SCHEMA,
            key,
            arch,
            created_unix_ms: 0,
            cmd: None,
            env: Vec::new(),
            workdir: None,
            user: None,
            kernel: None,
            rootfs_sha256,
            rootfs_size,
            source: String::new(),
            steps: Vec::new(),
        }
    }

    /// The manifest of this image, refused when its schema is unknown.
    ///
    /// # Errors
    /// [`SandboxError::InvalidSpec`] when the schema is not [`IMAGE_SCHEMA`].
    pub fn check_schema(&self) -> SandboxResult<()> {
        if self.schema == IMAGE_SCHEMA {
            Ok(())
        } else {
            Err(SandboxError::InvalidSpec {
                field: "manifest.schema".to_string(),
                reason: format!(
                    "the image manifest schema {} is not supported, expected {IMAGE_SCHEMA}",
                    self.schema
                ),
            }
            .into())
        }
    }
}

/// Turns an image reference into its root file and its manifest.
///
/// The native image store (feature 12) implements this trait. The control
/// plane in `shared/` holds the trait object, so it needs no native code.
pub trait ImageResolver: Send + Sync + 'static {
    /// Resolves one reference to `(rootfs path, manifest)`.
    ///
    /// # Errors
    /// [`SandboxError::NotFound`] for an unknown tag or key, and
    /// [`SandboxError::Io`] when the store cannot be read.
    fn resolve<'a>(
        &'a self,
        image: &'a ImageRef,
    ) -> crate::shared::BoxFuture<'a, SandboxResult<(PathBuf, ImageManifest)>>;
}

/// Whether `text` is a 32-character lower-case hexadecimal key.
#[must_use]
pub fn is_image_key(text: &str) -> bool {
    text.len() == 32
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Rewrites `spec` for an image root with the manifest defaults (section 6.3).
///
/// The manifest environment comes first and the spec environment second. An
/// explicit spec work directory wins over the manifest one; the spec default
/// `/workspace` does not. The manifest kernel applies only when the spec names
/// none.
#[must_use]
pub fn apply_manifest(
    spec: &SandboxSpec,
    rootfs: PathBuf,
    manifest: &ImageManifest,
) -> SandboxSpec {
    let mut out = spec.clone();
    out.rootfs = RootFs::Ext4Image(rootfs);

    let mut env = manifest.env.clone();
    env.extend(spec.env.iter().cloned());
    out.env = env;

    out.workdir = if spec.workdir == DEFAULT_WORKDIR {
        manifest
            .workdir
            .clone()
            .unwrap_or_else(|| DEFAULT_WORKDIR.to_string())
    } else {
        spec.workdir.clone()
    };

    if out.kernel.is_none() {
        if let Some(name) = &manifest.kernel {
            out.kernel = Some(KernelSource::Named(name.clone()));
        }
    }
    out
}

/// Refuses an image that another architecture built (invariant I11).
///
/// # Errors
/// [`SandboxError::ImageArchMismatch`] when the architectures differ.
pub fn check_arch(manifest: &ImageManifest) -> SandboxResult<()> {
    let host = GuestArch::host();
    if manifest.arch == host {
        Ok(())
    } else {
        Err(SandboxError::ImageArchMismatch {
            image: format!("{} ({})", manifest.key, manifest.arch.as_str()),
            host: host.as_str().to_string(),
        }
        .into())
    }
}
