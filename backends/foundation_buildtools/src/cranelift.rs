//! Cranelift codegen-backend detection — extracted from `foundation_core/build.rs`.
//!
//! Cranelift doesn't support wasm targets or CMake/C++ foreign exceptions.
//! Build scripts that cross-compile to wasm or invoke `cmake` need to detect
//! it and either skip the build or override with LLVM.

use std::env;
use std::path::Path;

/// Returns `true` when the active rustc invocation uses the cranelift backend.
///
/// Checks `CARGO_ENCODED_RUSTFLAGS` for `codegen-backend=cranelift`.
#[must_use]
pub fn is_cranelift_active() -> bool {
    env::var("CARGO_ENCODED_RUSTFLAGS")
        .is_ok_and(|flags| flags.contains("codegen-backend=cranelift"))
}

/// Detect cranelift from the workspace `Cargo.toml` and emit `cargo::rustc-cfg=cranelift_backend`.
///
/// Call from `build.rs` when your crate needs a `#[cfg(cranelift_backend)]` gate.
/// Also registers the cfg with `cargo::rustc-check-cfg` so clippy doesn't warn.
///
/// # Panics
/// Panics if `CARGO_MANIFEST_DIR` is not set (should never happen in `build.rs`).
pub fn set_cranelift_cfg() {
    println!("cargo::rustc-check-cfg=cfg(cranelift_backend)");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent());

    if let Some(root) = workspace_root {
        if let Ok(content) = std::fs::read_to_string(root.join("Cargo.toml")) {
            if content.contains("codegen-backend") && content.contains("cranelift") {
                println!("cargo::rustc-cfg=cranelift_backend");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_without_env() {
        // When CARGO_ENCODED_RUSTFLAGS is absent, cranelift is not active.
        assert!(!is_cranelift_active());
    }
}
