//! `SandboxSpec`: what a caller wants, and its validation rules.

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::shared::driver::Capabilities;
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::limits::SandboxLimits;
use crate::shared::serde_helpers::duration_millis_option;

/// The work directory of a spec that does not name one.
///
/// It is the default of [`SandboxSpec::new`], the CLI default and the value
/// that an empty `workdir` means (issue #1031). A program runs in this
/// directory when it names no other one.
pub const DEFAULT_WORKDIR: &str = "/workspace";

/// The serde default of [`SandboxSpec::workdir`].
fn default_workdir() -> String {
    DEFAULT_WORKDIR.to_string()
}

/// The root file system of a sandbox.
///
/// A path is a path on the machine that runs the control plane. A remote
/// caller of `SandboxService` names paths of that machine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RootFs {
    /// A directory on the control plane's host.
    Directory(PathBuf),
    /// A raw ext4 image file on the control plane's host.
    Ext4Image(PathBuf),
    /// A `SquashFS` image file on the control plane's host.
    SquashfsImage(PathBuf),
    /// A name that the driver understands, such as a Docker tag.
    ContainerImage(String),
    /// A sandbox image of feature 12.
    Image(ImageRef),
}

/// How the sandbox treats writes to its root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootMode {
    /// Writes go to a memory layer.
    Overlay,
    /// Only `/tmp`, `/run` and the work directory are writable.
    ReadOnly,
    /// The root file system is the image itself, mounted read-write.
    ///
    /// Only an [`RootFs::Ext4Image`] can use this mode. The driver holds an
    /// exclusive lock on the image file, so one sandbox writes it at a time.
    Writable,
}

/// The network mode of a sandbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkMode {
    /// No network.
    None,
    /// The host network.
    Host,
}

/// One host directory that appears at a guest path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostMount {
    /// The path on the control plane's host.
    pub host_path: PathBuf,
    /// The absolute path in the guest.
    pub guest_path: String,
    /// Whether the guest may write to the directory.
    pub read_only: bool,
}

/// Where the guest kernel comes from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum KernelSource {
    /// The driver's default kernel.
    Default,
    /// A kernel image file on the control plane's host.
    Path(PathBuf),
    /// A pinned kernel manifest name of feature 03, such as
    /// `firecracker-kernel-x86_64`.
    Named(String),
}

/// A sandbox image reference of feature 12.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageRef {
    /// A tag in the local image store.
    Tag(String),
    /// A content key in the local image store.
    Key(String),
}

/// What the caller wants a sandbox to be.
///
/// The control plane does not change a spec after `create` takes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxSpec {
    /// The driver to use, or `None` for the first usable driver in the
    /// configured order.
    pub driver: Option<crate::shared::ids::DriverId>,
    /// The parent sandbox of a nested child (feature 11).
    ///
    /// It is `Some` exactly when `driver` is `Some(Nested)`. Each path of the
    /// spec is a path in the parent container, because the inner control plane
    /// runs there.
    #[serde(default)]
    pub parent: Option<crate::shared::ids::SandboxId>,
    /// Whether this sandbox is a nested host: a container whose slave also
    /// serves the control plane (feature 11). Default `false`.
    #[serde(default)]
    pub nested_host: bool,
    /// The root file system. Required.
    pub rootfs: RootFs,
    /// How writes to the root are handled. Default [`RootMode::Overlay`].
    pub root_mode: RootMode,
    /// The number of virtual CPUs. Default 1. Must be at least 1.
    pub vcpus: u8,
    /// The memory in mebibytes. Default 512. Must be at least 64.
    pub memory_mib: u32,
    /// The network mode. Default [`NetworkMode::None`].
    pub network: NetworkMode,
    /// The DNS servers, used only with [`NetworkMode::Host`].
    pub dns: Vec<String>,
    /// The base environment of each command.
    pub env: Vec<(String, String)>,
    /// The working directory that the slave creates. Default `/workspace`.
    ///
    /// An empty value means the default. A remote caller and an older record
    /// can carry an empty value; [`Self::effective_workdir`] gives the path
    /// that the control plane sends in `Prepare`.
    #[serde(default = "default_workdir")]
    pub workdir: String,
    /// Host directories that the guest sees.
    pub mounts: Vec<HostMount>,
    /// The kernel source, or `None` for the driver default.
    pub kernel: Option<KernelSource>,
    /// Every size and time limit.
    pub limits: SandboxLimits,
    /// The time after which the sandbox stops, whatever it does.
    ///
    /// `None` (the default) gives no lifetime limit, so an older client that
    /// sends no `ttl` gets today's behaviour. The control plane computes the
    /// deadline once at create and stores it, so a restart does not move it.
    #[serde(default, with = "duration_millis_option")]
    pub ttl: Option<Duration>,
}

impl SandboxSpec {
    /// A spec with `rootfs` and every other field at its default.
    #[must_use]
    pub fn new(rootfs: RootFs) -> Self {
        Self {
            driver: None,
            parent: None,
            nested_host: false,
            rootfs,
            root_mode: RootMode::Overlay,
            vcpus: 1,
            memory_mib: 512,
            network: NetworkMode::None,
            dns: vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()],
            env: Vec::new(),
            workdir: default_workdir(),
            mounts: Vec::new(),
            kernel: None,
            limits: SandboxLimits::default(),
            ttl: None,
        }
    }

    /// The work directory that the control plane sends in `Prepare`.
    ///
    /// An empty `workdir` means the default, [`DEFAULT_WORKDIR`]. A remote
    /// caller and an older record can carry an empty value. The slave refuses
    /// an empty work directory, because it cannot start a program in it.
    #[must_use]
    pub fn effective_workdir(&self) -> &str {
        if self.workdir.is_empty() {
            DEFAULT_WORKDIR
        } else {
            &self.workdir
        }
    }

    /// Checks the spec against the capabilities of one host.
    ///
    /// The rules run in the order of the feature design, and the function
    /// returns at the first failure.
    ///
    /// # Errors
    /// [`SandboxError::InvalidSpec`] for a wrong field,
    /// [`SandboxError::UnsupportedRootFs`], [`SandboxError::UnsupportedNetwork`]
    /// or [`SandboxError::UnsupportedMount`] when the host cannot do what the
    /// spec asks.
    pub fn validate(&self, caps: &Capabilities) -> SandboxResult<()> {
        if self.vcpus < 1 {
            return Err(invalid("vcpus", "vcpus must be at least 1"));
        }
        if self.memory_mib < 64 {
            return Err(invalid("memory_mib", "memory_mib must be at least 64"));
        }

        // A child has a parent, and only a child has one (feature 11).
        let nested = self.driver == Some(crate::shared::ids::DriverId::Nested);
        if self.parent.is_some() != nested {
            return Err(invalid(
                "parent",
                "parent is set exactly when driver is `nested`",
            ));
        }

        // An empty workdir means the default (issue #1031); a value that the
        // caller gives must be absolute and must not leave the root.
        if !self.workdir.is_empty() && !absolute_without_parent(Path::new(&self.workdir)) {
            return Err(invalid(
                "workdir",
                &format!(
                    "workdir must be absolute and must not contain `..` (got {:?})",
                    self.workdir
                ),
            ));
        }
        for mount in &self.mounts {
            if !absolute_without_parent(Path::new(&mount.guest_path)) {
                return Err(invalid(
                    "mounts",
                    &format!(
                        "every guest_path must be absolute and must not contain `..` (got {:?})",
                        mount.guest_path
                    ),
                ));
            }
        }

        for (key, _) in &self.env {
            if key.is_empty() || key.contains('=') || key.as_bytes().contains(&0) {
                return Err(invalid(
                    "env",
                    "an environment key must not be empty and must not contain `=` or a zero byte",
                ));
            }
        }

        if self
            .dns
            .iter()
            .any(|dns| dns.parse::<std::net::IpAddr>().is_err())
        {
            return Err(invalid("dns", "every dns entry must be an IP address"));
        }

        if !self.rootfs_supported(caps) {
            return Err(SandboxError::UnsupportedRootFs.into());
        }

        if self.root_mode == RootMode::Writable && !matches!(self.rootfs, RootFs::Ext4Image(_)) {
            return Err(invalid(
                "root_mode",
                "the writable root mode needs an ext4 image root",
            ));
        }

        if !caps.root_modes.contains(&self.root_mode) {
            return Err(invalid(
                "root_mode",
                "the host does not support this root_mode",
            ));
        }

        let network_supported = match self.network {
            NetworkMode::None => caps.network_none,
            NetworkMode::Host => caps.network_host,
        };
        if !network_supported {
            return Err(SandboxError::UnsupportedNetwork.into());
        }

        if !self.mounts.is_empty() && !caps.host_mounts {
            return Err(SandboxError::UnsupportedMount.into());
        }

        if !self.limits.all_positive() {
            return Err(invalid("limits", "every limit must be more than zero"));
        }
        if self.limits.chunk_bytes > self.limits.max_message_bytes / 2 {
            return Err(invalid(
                "limits",
                "chunk_bytes must be at most half of max_message_bytes",
            ));
        }

        Ok(())
    }

    /// Whether the capabilities of the host allow this root file system kind.
    fn rootfs_supported(&self, caps: &Capabilities) -> bool {
        match &self.rootfs {
            RootFs::Directory(_) => caps.rootfs_directory,
            RootFs::Ext4Image(_) => caps.rootfs_ext4,
            RootFs::SquashfsImage(_) => caps.rootfs_squashfs,
            RootFs::ContainerImage(_) => caps.rootfs_container_image,
            // The control plane turns an `Image` into `Ext4Image` before it
            // calls `validate`. A spec that still holds one is not supported.
            RootFs::Image(_) => false,
        }
    }
}

/// A path is absolute and has no `..` part.
fn absolute_without_parent(path: &Path) -> bool {
    path.is_absolute()
        && !path
            .components()
            .any(|component| component == Component::ParentDir)
}

/// The error for a wrong spec field.
fn invalid(field: &str, reason: &str) -> foundation_errstacks::ErrorTrace<SandboxError> {
    SandboxError::InvalidSpec {
        field: field.to_string(),
        reason: reason.to_string(),
    }
    .into()
}

/// What the control plane sends in `Prepare`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootSetup {
    /// Where the root comes from.
    pub source: RootSource,
    /// How writes to the root are handled.
    pub mode: RootMode,
}

/// The source of a prepared root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RootSource {
    /// A block device with a file system.
    BlockDevice {
        /// The device path in the guest.
        device: String,
        /// The file system type.
        fstype: FsType,
    },
    /// A virtiofs share.
    VirtioFs {
        /// The share tag.
        tag: String,
    },
    /// The slave's own root, used as it is.
    Plain,
}

/// A file system type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FsType {
    /// The ext4 file system.
    Ext4,
    /// The `SquashFS` file system.
    Squashfs,
}

/// One guest mount that the control plane sends in `Prepare`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuestMount {
    /// The share tag.
    pub tag: String,
    /// The absolute path in the guest.
    pub guest_path: String,
    /// Whether the guest may write to the share.
    pub read_only: bool,
}

/// The guest architecture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuestArch {
    /// `x86_64`.
    X86_64,
    /// `aarch64`.
    Aarch64,
}

impl GuestArch {
    /// The architecture of this host.
    ///
    /// A target that is neither `x86_64` nor `aarch64` (for example wasm32)
    /// reports [`GuestArch::X86_64`], the first guest target.
    #[must_use]
    pub fn host() -> Self {
        #[cfg(target_arch = "aarch64")]
        {
            Self::Aarch64
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            Self::X86_64
        }
    }

    /// The architecture name, as a host program file uses it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
        }
    }
}
