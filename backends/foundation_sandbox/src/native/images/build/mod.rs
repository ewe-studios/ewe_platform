//! The `Sandboxfile` builder (feature 14).
//!
//! [`keys`] computes the content key of each step. [`cache`] keeps the snapshot
//! of each step. The executor that drives a build runs the steps in order.

use serde::{Deserialize, Serialize};

use crate::shared::limits::BuildLimits;

pub mod cache;
pub mod keys;
pub mod steps;

/// The metadata that the steps of a build gather so far.
///
/// It becomes the manifest of the final image.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildMetadata {
    /// The environment entries, in file order. A later key wins.
    pub env: Vec<(String, String)>,
    /// The work directory, or `None` for the root.
    pub workdir: Option<String>,
    /// The user of the final image.
    pub user: Option<String>,
    /// The start command of the final image.
    pub cmd: Option<Vec<String>>,
    /// The kernel name of feature 03's manifest.
    pub kernel: Option<String>,
}

impl BuildMetadata {
    /// The metadata with every field empty.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The canonical JSON of this metadata.
    ///
    /// The final image key hashes this text (section 6.2).
    #[must_use]
    pub fn canonical_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// The options of one build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildOptions {
    /// The tag that the image gets, when the caller wants one.
    pub tag: Option<String>,
    /// The size of each work image in mebibytes.
    pub size_mib: u32,
    /// The virtual CPUs of each build sandbox.
    pub vcpus: u8,
    /// The memory of each build sandbox in mebibytes.
    pub memory_mib: u32,
    /// Whether every step after `FROM` runs, ignoring the cache, and still
    /// stores its result.
    pub no_cache: bool,
    /// The limits of this build.
    pub limits: BuildLimits,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            tag: None,
            size_mib: 4096,
            vcpus: 2,
            memory_mib: 2048,
            no_cache: false,
            limits: BuildLimits::default(),
        }
    }
}
