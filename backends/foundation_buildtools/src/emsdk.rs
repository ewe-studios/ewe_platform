//! EMSDK (Emscripten SDK) discovery and wiring.
//!
//! The project vendors EMSDK at `tools/emsdk`. This module locates it,
//! validates the installation, and provides paths for `build.rs` consumers
//! that need to compile C/C++ via emscripten (`llama.cpp`, `fff`).

use std::env;
use std::path::{Path, PathBuf};

/// Handle to a discovered Emscripten SDK installation.
#[derive(Debug, Clone)]
pub struct Emsdk {
    root: PathBuf,
}

impl Emsdk {
    /// Discover the EMSDK from the `EMSDK_DIR` env var.
    ///
    /// Returns `None` if the env var is not set.
    ///
    /// # Panics
    /// Panics if the env var is set but the path doesn't exist.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let dir = env::var("EMSDK_DIR").ok()?;
        let root = std::fs::canonicalize(Path::new(&dir))
            .unwrap_or_else(|e| panic!("EMSDK_DIR={dir} is not a valid path: {e}"));
        Some(Self { root })
    }

    /// Discover the EMSDK from the vendored `tools/emsdk` directory relative
    /// to the workspace root.
    ///
    /// `workspace_root` is typically obtained from `CARGO_MANIFEST_DIR` + parent
    /// traversal.
    #[must_use]
    pub fn from_workspace(workspace_root: &Path) -> Option<Self> {
        let root = workspace_root.join("tools/emsdk");
        if root.exists() {
            Some(Self { root })
        } else {
            None
        }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn upstream_emscripten(&self) -> PathBuf {
        self.root.join("upstream/emscripten")
    }

    #[must_use]
    pub fn system_include(&self) -> PathBuf {
        self.upstream_emscripten().join("system/include")
    }

    #[must_use]
    pub fn cache_sysroot(&self) -> PathBuf {
        self.root.join("upstream/emscripten/cache/sysroot")
    }

    #[must_use]
    pub fn cmake_toolchain(&self) -> PathBuf {
        self.upstream_emscripten()
            .join("cmake/Modules/Platform/Emscripten.cmake")
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.upstream_emscripten().exists()
    }

    /// Emit `cargo:rerun-if-env-changed=EMSDK_DIR` for build.rs consumers.
    pub fn rerun_if_changed(&self) {
        println!("cargo:rerun-if-env-changed=EMSDK_DIR");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_returns_some_when_set() {
        // If EMSDK_DIR is set (as in our workspace), from_env should return Some.
        // If unset, it should return None. Either way, no panic.
        let result = Emsdk::from_env();
        if std::env::var("EMSDK_DIR").is_ok() {
            assert!(result.is_some());
        } else {
            assert!(result.is_none());
        }
    }

    #[test]
    fn from_workspace_missing() {
        let tmp = std::env::temp_dir().join("foundation_buildtools_test_emsdk");
        assert!(Emsdk::from_workspace(&tmp).is_none());
    }
}
