//! The pinned guest artifacts.
//!
//! The values here are data. A caller can pass its own
//! [`ArtifactSpec`](super::ArtifactSpec) to [`ArtifactStore::ensure`], so the
//! list is a convenience and not a closed set.
//!
//! The versions and the hash of each file came from one manual session on
//! 2026-11-25. The spec `learnings.md` records how each hash was made. The code
//! never lists a remote bucket at run time.

use crate::shared::spec::GuestArch;

use super::{ArtifactKind, ArtifactSpec};

/// The newest Alpine 3.22 patch release on 2026-11-25.
pub const ALPINE_VERSION: &str = "3.22.8";

/// The Firecracker CI prefix, found with one list call on 2026-11-25.
pub const FIRECRACKER_CI_PREFIX: &str = "v1.15";

/// The Firecracker CI kernel version under that prefix.
pub const FIRECRACKER_KERNEL_VERSION: &str = "6.1.155";

/// The Ubuntu version of the Firecracker CI root file system.
pub const FIRECRACKER_ROOTFS_VERSION: &str = "24.04";

/// One pinned artifact: its spec, its size and its source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinnedArtifact {
    /// The manifest name.
    pub name: &'static str,
    /// The download address.
    pub url: &'static str,
    /// The expected Secure Hash Algorithm 256 (SHA-256), in lowercase hex.
    pub sha256: &'static str,
    /// The size of the file in bytes, from the server `Content-Length`.
    pub size: u64,
    /// The cache kind.
    pub kind: ArtifactKind,
}

impl PinnedArtifact {
    /// The spec that [`ArtifactStore::ensure`](super::ArtifactStore::ensure)
    /// takes.
    #[must_use]
    pub fn spec(&self) -> ArtifactSpec {
        ArtifactSpec {
            name: self.name.to_string(),
            url: self.url.to_string(),
            sha256: self.sha256.to_string(),
            kind: self.kind,
        }
    }
}

/// The architecture word of an Alpine or Firecracker URL.
#[must_use]
pub fn arch_word(arch: GuestArch) -> &'static str {
    match arch {
        GuestArch::X86_64 => "x86_64",
        GuestArch::Aarch64 => "aarch64",
    }
}

/// The Alpine minirootfs archive of `arch`.
#[must_use]
pub fn alpine_minirootfs(arch: GuestArch) -> PinnedArtifact {
    match arch {
        GuestArch::X86_64 => PinnedArtifact {
            name: "alpine-minirootfs-x86_64",
            url: "https://dl-cdn.alpinelinux.org/alpine/v3.22/releases/x86_64/alpine-minirootfs-3.22.8-x86_64.tar.gz",
            sha256: "2d7f46af9b7e86ad64ebeba3fe9b75b62dd5b259e708afb2ab21268f627696e5",
            size: 3_647_469,
            kind: ArtifactKind::TarGzDir,
        },
        GuestArch::Aarch64 => PinnedArtifact {
            name: "alpine-minirootfs-aarch64",
            url: "https://dl-cdn.alpinelinux.org/alpine/v3.22/releases/aarch64/alpine-minirootfs-3.22.8-aarch64.tar.gz",
            sha256: "36091efacd083c1cf5a8c837272530bff4fc72db1fbfdef559f52de0d890bca4",
            size: 3_971_704,
            kind: ArtifactKind::TarGzDir,
        },
    }
}

/// The Firecracker CI kernel of `arch`.
#[must_use]
pub fn firecracker_kernel(arch: GuestArch) -> PinnedArtifact {
    match arch {
        GuestArch::X86_64 => PinnedArtifact {
            name: "firecracker-kernel-x86_64",
            url: "https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.15/x86_64/vmlinux-6.1.155",
            sha256: "e20e46d0c36c55c0d1014eb20576171b3f3d922260d9f792017aeff53af3d4f2",
            size: 44_279_576,
            kind: ArtifactKind::File,
        },
        GuestArch::Aarch64 => PinnedArtifact {
            name: "firecracker-kernel-aarch64",
            url: "https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.15/aarch64/vmlinux-6.1.155",
            sha256: "e3544b10603acbf3db492cb52e000d22ba202cb4b63b9add027565683e11c591",
            size: 17_111_552,
            kind: ArtifactKind::File,
        },
    }
}

/// The Firecracker CI root file system of `arch`, as a squashfs image.
#[must_use]
pub fn firecracker_ci_rootfs(arch: GuestArch) -> PinnedArtifact {
    match arch {
        GuestArch::X86_64 => PinnedArtifact {
            name: "firecracker-ci-rootfs-x86_64",
            url: "https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.15/x86_64/ubuntu-24.04.squashfs",
            sha256: "68321e0482baeb3844dafe8a6b08a6902401a7afc41fbfd8c3d9ea08aadd244f",
            size: 107_986_944,
            kind: ArtifactKind::File,
        },
        GuestArch::Aarch64 => PinnedArtifact {
            name: "firecracker-ci-rootfs-aarch64",
            url: "https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.15/aarch64/ubuntu-24.04.squashfs",
            sha256: "0efb6a3ff2982baa6ca7e3d940966516ba7ddd2df5deb3e6c2161d369a15d608",
            size: 105_332_736,
            kind: ArtifactKind::File,
        },
    }
}

/// The Alpine archive as a plain file, for the image importer of feature 13.
///
/// The tree entry of [`alpine_minirootfs`] does not keep the archive, because
/// the tree is the entry. This function caches the archive in its own entry.
#[must_use]
pub fn alpine_minirootfs_archive(arch: GuestArch) -> PinnedArtifact {
    let mut artifact = alpine_minirootfs(arch);
    artifact.name = match arch {
        GuestArch::X86_64 => "alpine-minirootfs-archive-x86_64",
        GuestArch::Aarch64 => "alpine-minirootfs-archive-aarch64",
    };
    artifact.kind = ArtifactKind::File;
    artifact
}
