//! The public types of an image unpack (feature 12).
//!
//! [`SandboxHandle::unpack`](crate::shared::control::SandboxHandle::unpack)
//! reads an uncompressed tar stream into a directory of the guest. The slave
//! applies the entries in a child process whose root is the target, so no entry
//! can write outside it.

use serde::{Deserialize, Serialize};

/// The options of one unpack.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnpackOptions {
    /// Whether OCI whiteout entries remove earlier entries.
    pub oci_whiteouts: bool,
    /// Exact paths, relative to the target, that the reader drops.
    pub skip: Vec<String>,
}

/// The result of one unpack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnpackReport {
    /// The number of tar entries that the reader applied.
    pub entries: u64,
    /// The number of file data bytes that the reader wrote.
    pub bytes: u64,
}

/// The text of a limit failure. The control plane maps it to
/// [`SandboxError::ArtifactTooLarge`](crate::shared::error::SandboxError::ArtifactTooLarge).
pub const UNPACK_LIMIT_MESSAGE: &str = "unpack limit";
