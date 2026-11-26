//! Generate browser runtime scripts for a Cargo consumer.
pub use crate::binding_build::bindings;
use crate::runtime_build::{generate, MINIFIED, READABLE};
use std::{fs, io, path::Path, prelude::rust_2021::*};

/// Version shared by Cargo and the public JavaScript package.
pub const RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Build the classic browser runtime in the calling package's Cargo output directory.
///
/// # Errors
/// Returns a missing output directory, source, or bundling error.
pub fn generate_from_cargo() -> io::Result<()> {
    let output =
        std::env::var_os("OUT_DIR").ok_or_else(|| io::Error::other("Cargo did not set OUT_DIR"))?;
    let runtime = Path::new(env!("CARGO_MANIFEST_DIR")).join("runtimes");
    // Registry packages have versioned directory names. Resolve the cross-package
    // import through the same directory layout that the source entry declares.
    let layout = tempfile::tempdir_in(&output)?;
    let inputs = layout.path().join("foundation_wasm_ui/runtimes");
    let core = layout.path().join("foundation_wasm/runtime");
    fs::create_dir_all(&inputs)?;
    fs::create_dir_all(&core)?;
    fs::write(
        core.join("foundation-wasm.js"),
        foundation_wasm::embedded::FOUNDATION_WASM_JS,
    )?;
    copy_runtime(&runtime, &inputs)?;
    let inputs = inputs.canonicalize()?;
    for input in generate(&inputs, Path::new(&output), false)? {
        if let Ok(relative) = input.strip_prefix(&inputs) {
            std::println!(
                "cargo:rerun-if-changed={}",
                runtime.join(relative).display()
            );
        }
    }
    Ok(())
}

fn copy_runtime(runtime: &Path, destination: &Path) -> io::Result<()> {
    for entry in fs::read_dir(runtime)? {
        let entry = entry?;
        let name = entry.file_name();
        if entry.path().extension().is_some_and(|ext| ext == "js")
            && [READABLE, MINIFIED, "bundle.js"]
                .iter()
                .all(|skip| name != *skip)
        {
            fs::copy(entry.path(), destination.join(&name))?;
        }
    }
    Ok(())
}

/// Prepare framework modules as private inputs for an application script.
///
/// # Errors
/// Returns an error if a runtime source cannot be read or copied.
pub fn application_sources(destination: &Path) -> io::Result<()> {
    let runtime = Path::new(env!("CARGO_MANIFEST_DIR")).join("runtimes");
    fs::create_dir_all(destination)?;
    copy_runtime(&runtime, destination)?;
    fs::write(
        destination.join("foundation-wasm.js"),
        foundation_wasm::embedded::FOUNDATION_WASM_JS,
    )?;
    std::println!("cargo:rerun-if-changed={}", runtime.display());
    Ok(())
}
