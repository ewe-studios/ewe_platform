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

/// The cargo profile a build script is building for, read from its `OUT_DIR`.
///
/// `OUT_DIR` is `<target>[/<triple>]/<profile-dir>/build/<pkg>-<hash>/out`. The
/// profile directory is `debug` for `dev` (and `test`), `release` for `release`
/// (and `bench`), and the profile's own name for a custom profile such as `uat`.
#[must_use]
pub fn profile_from_out_dir(out_dir: &Path) -> Option<String> {
    let parts: Vec<&str> = out_dir.iter().filter_map(|c| c.to_str()).collect();
    let build = parts.iter().rposition(|c| *c == "build")?;
    let dir = *parts.get(build.checked_sub(1)?)?;
    Some(match dir {
        "debug" => "dev".to_string(),
        other => other.to_string(),
    })
}

/// The codegen backend `profile` resolves to in a workspace manifest, following
/// `inherits`. `None` means no profile in the chain sets one (cargo's default,
/// LLVM).
///
/// Reads only `[profile.<name>]` tables' `codegen-backend` and `inherits` keys,
/// which is all this needs, so the build helpers stay dependency-free.
#[must_use]
pub fn profile_codegen_backend(manifest: &str, profile: &str) -> Option<String> {
    let mut name = profile.to_string();
    // Inheritance chains are short; the bound only guards a malformed cycle.
    for _ in 0..16 {
        let (backend, inherits) = profile_keys(manifest, &name);
        if backend.is_some() {
            return backend;
        }
        name = match (inherits, name.as_str()) {
            (Some(parent), _) => parent,
            // cargo's built-in parents for its built-in profiles
            (None, "test") => "dev".to_string(),
            (None, "bench") => "release".to_string(),
            (None, _) => return None,
        };
    }
    None
}

/// `codegen-backend` and `inherits` from the `[profile.<name>]` table.
fn profile_keys(manifest: &str, name: &str) -> (Option<String>, Option<String>) {
    let header = format!("[profile.{name}]");
    let mut in_table = false;
    let (mut backend, mut inherits) = (None, None);
    for line in manifest.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            in_table = line == header;
            continue;
        }
        if !in_table {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim().trim_matches('"').to_string();
            match key.trim() {
                "codegen-backend" => backend = Some(value),
                "inherits" => inherits = Some(value),
                _ => {}
            }
        }
    }
    (backend, inherits)
}

/// Emit `cargo::rustc-cfg=cranelift_backend` when the crate is being built with
/// the cranelift backend.
///
/// Call from `build.rs` when your crate needs a `#[cfg(cranelift_backend)]` gate.
/// Also registers the cfg with `cargo::rustc-check-cfg` so clippy doesn't warn.
///
/// The backend comes from, in order: `-Zcodegen-backend=cranelift` in the
/// rustflags; a `CARGO_PROFILE_<NAME>_CODEGEN_BACKEND` override; the active
/// profile's `codegen-backend` in the workspace `Cargo.toml`, following
/// `inherits`. The active profile is read from `OUT_DIR`, so a `--profile uat`
/// build on LLVM does not get the cfg even though `[profile.dev]` uses cranelift.
///
/// # Panics
/// Panics if `CARGO_MANIFEST_DIR` is not set (should never happen in `build.rs`).
pub fn set_cranelift_cfg() {
    println!("cargo::rustc-check-cfg=cfg(cranelift_backend)");
    println!("cargo::rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");

    if is_cranelift_active() {
        println!("cargo::rustc-cfg=cranelift_backend");
        return;
    }

    let Some(profile) = env::var_os("OUT_DIR").and_then(|d| profile_from_out_dir(Path::new(&d)))
    else {
        return;
    };

    let override_var = format!(
        "CARGO_PROFILE_{}_CODEGEN_BACKEND",
        profile.to_uppercase().replace('-', "_")
    );
    println!("cargo::rerun-if-env-changed={override_var}");
    let backend = env::var(&override_var).ok().or_else(|| {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let root = Path::new(&manifest_dir).parent().and_then(|p| p.parent())?;
        let manifest_path = root.join("Cargo.toml");
        println!("cargo::rerun-if-changed={}", manifest_path.display());
        let manifest = std::fs::read_to_string(manifest_path).ok()?;
        profile_codegen_backend(&manifest, &profile)
    });

    if backend.as_deref() == Some("cranelift") {
        println!("cargo::rustc-cfg=cranelift_backend");
    }
}
