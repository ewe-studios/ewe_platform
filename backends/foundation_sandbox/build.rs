//! Keeps `src/native/firecracker/generated/` honest against the vendored API
//! file.
//!
//! WHY: This file is the declaration of what this crate generates: the provider,
//! the pinned API version and the destination. Keeping it here makes the
//! surface visible next to the code that uses it, and lets anyone reproduce the
//! output (spec 77 feature 04).
//!
//! WHAT: Verifies the pin still holds, and regenerates when the committed output
//! no longer matches this declaration. A moved specification fails the build on
//! purpose: regenerating against a changed document would rewrite the types with
//! nobody choosing to.
//!
//! HOW: The three outcomes are different:
//!
//! | `check()` says | meaning | what happens |
//! |---|---|---|
//! | `Ok` | output is current | nothing |
//! | `Stale` | the declaration moved | **regenerate** |
//! | anything else | the specification moved under the pin, or is unreadable | **fail the build** |
//!
//! The generated code sits behind the `firecracker` feature. Without that feature
//! this script does nothing, so the crate builds with no generated code and no
//! `foundation_deployment` dependency.
//!
//! Regenerate by hand with the build script of this crate, or with the shared
//! command:
//!
//! ```text
//! cargo check -p foundation_sandbox --features firecracker
//! ```

fn main() {
    read_min_firecracker_version();
    #[cfg(feature = "firecracker")]
    generate();
}

/// Reads the lowest accepted Firecracker version from `ci/sandbox.toml`.
///
/// The probe compares the version of the real program with this value. The file
/// is not published to crates.io, so a packaged build has no value and accepts
/// every version; the API fields that the driver sends are old and stable.
fn read_min_firecracker_version() {
    use std::path::Path;

    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let path = Path::new(&manifest).join("../../ci/sandbox.toml");
    println!("cargo:rerun-if-changed={}", path.display());
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "min_firecracker_version" {
            continue;
        }
        let value = value.trim().trim_matches('"');
        println!("cargo:rustc-env=FOUNDATION_SANDBOX_MIN_FIRECRACKER_VERSION={value}");
        return;
    }
}

#[cfg(feature = "firecracker")]
fn generate() {
    use std::path::Path;

    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let manifest = Path::new(&manifest);
    let spec = manifest.join("../../artefacts/cloud_providers/firecracker/openapi.json");

    // The specification artefact is not published to crates.io, so a consumer
    // that builds this crate from the registry has no specification to check
    // against. The committed `src/native/firecracker/generated/` is exactly
    // what they compile. Nothing to do.
    if !spec.exists() {
        return;
    }

    // Rerun only when the specification or this declaration changes. Without
    // these, Cargo watches every file in the package, and because this script
    // writes into `src/`, that would rebuild forever.
    println!("cargo:rerun-if-changed={}", spec.display());
    println!("cargo:rerun-if-changed=build.rs");

    let codegen = foundation_codegentools::generate()
        .provider("firecracker")
        .spec(&spec)
        .spec_version("1.19.0-dev")
        // The output root is the crate, so the generator finds this crate's
        // Cargo.toml. The provider directory is the module beside the client.
        .output_dir(manifest)
        .provider_dir(manifest.join("src/native/firecracker"))
        // Firecracker sends enum values with a space (`Not started`) and a
        // leading digit (`2M`), and rejects a `null` for several optional
        // fields. The client parses the vendor's `fault_message`, so the error
        // schema is generated too. These are opt-in generator rules, so other
        // providers keep their committed output.
        .with_inline_enums(true)
        .with_skip_optional_none(true)
        .with_error_types(true);

    match codegen.check() {
        Ok(()) => {}
        Err(foundation_codegentools::CodegenError::Stale { .. }) => {
            codegen
                .write()
                .expect("regenerate the Firecracker API surface");
            println!(
                "cargo:warning=regenerated src/native/firecracker/generated/ from {}",
                spec.display()
            );
        }
        Err(error) => panic!("{error}"),
    }
}
