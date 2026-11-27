//! The step keys of a `Sandboxfile` build (feature 14, section 6.2).
//!
//! A key depends only on the key before it, the canonical step text and the
//! content of the step's inputs. It never depends on time, host paths or file
//! modification times (invariant I3).

use sha2::{Digest as _, Sha256};

use crate::shared::spec::GuestArch;

/// The prefix of a `FROM` key. It freezes the key rules of version 1.
const BASE_PREFIX: &str = "ewe-build-v1";

/// The tree entry type of a step input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeKind {
    /// A regular file.
    File,
    /// A directory.
    Dir,
    /// A symbolic link.
    Symlink,
}

impl TreeKind {
    /// The one-character name of this type in a tree hash.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "f",
            Self::Dir => "d",
            Self::Symlink => "l",
        }
    }
}

/// One entry of a step input tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeEntry {
    /// The path, relative to the step input, with `/` separators.
    pub path: String,
    /// The type of the entry.
    pub kind: TreeKind,
    /// The mode that the entry gets in the image.
    pub mode: u32,
    /// The content size in bytes. A directory has size 0.
    pub size: u64,
    /// The SHA-256 of the content, or the target of a symbolic link.
    pub content: String,
}

/// The key of a `FROM` step.
#[must_use]
pub fn base_key(base_image_key: &str, arch: GuestArch, size_mib: u32) -> String {
    let mut hasher = Sha256::new();
    hasher.update(BASE_PREFIX.as_bytes());
    hasher.update(b"\n");
    hasher.update(base_image_key.as_bytes());
    hasher.update(b"\n");
    hasher.update(arch.as_str().as_bytes());
    hasher.update(b"\n");
    hasher.update(size_mib.to_string().as_bytes());
    hex(&hasher.finalize())[..32].to_string()
}

/// The key of one step that follows `previous`.
///
/// `extra` is the tree hash of an `ADD` step, and `None` for every other
/// step.
#[must_use]
pub fn step_key(previous: &str, canonical_line: &str, extra: Option<&str>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(previous.as_bytes());
    hasher.update(b"\n");
    hasher.update(canonical_line.as_bytes());
    if let Some(extra) = extra {
        hasher.update(b"\n");
        hasher.update(extra.as_bytes());
    }
    hex(&hasher.finalize())[..32].to_string()
}

/// The tree hash of one `ADD` input.
///
/// `entries` are sorted by path. Each entry contributes its relative path,
/// type, mode, size and the SHA-256 of its content, or the link target.
/// Modification times and owners do not contribute (invariant I3).
#[must_use]
pub fn tree_hash(entries: &[TreeEntry]) -> String {
    let mut sorted: Vec<&TreeEntry> = entries.iter().collect();
    sorted.sort_by(|left, right| left.path.cmp(&right.path));
    let mut hasher = Sha256::new();
    for entry in sorted {
        hasher.update(entry.path.as_bytes());
        hasher.update(b"\0");
        hasher.update(entry.kind.as_str().as_bytes());
        hasher.update(b"\0");
        hasher.update(format!("{:o}", entry.mode).as_bytes());
        hasher.update(b"\0");
        hasher.update(entry.size.to_string().as_bytes());
        hasher.update(b"\0");
        hasher.update(entry.content.as_bytes());
        hasher.update(b"\n");
    }
    hex(&hasher.finalize())[..32].to_string()
}

/// The key of the final image.
#[must_use]
pub fn image_key(last_step_key: &str, metadata_json: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(last_step_key.as_bytes());
    hasher.update(b"\n");
    hasher.update(metadata_json.as_bytes());
    hex(&hasher.finalize())[..32].to_string()
}

/// The SHA-256 of `bytes`, in lowercase hexadecimal.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
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
