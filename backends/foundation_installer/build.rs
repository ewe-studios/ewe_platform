//! Give the crate the target triple Cargo built it for.
//!
//! A GitHub release names one asset for each Rust target triple. The
//! `github-release` source picks the asset by this value, so it must be the
//! exact triple, not a value built from `std::env::consts`.

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    println!("cargo:rustc-env=FOUNDATION_INSTALLER_TARGET={target}");
    println!("cargo:rerun-if-changed=build.rs");
}
