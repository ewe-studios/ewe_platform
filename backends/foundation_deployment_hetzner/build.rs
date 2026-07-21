//! Keeps `src/generated/` honest against the vendor's spec.
//!
//! **WHY:** this file *is* the declaration of what this crate generates — the
//! slice, and the versions it was generated from. Keeping it here rather than in
//! the generator means the surface is visible next to the code that uses it, and
//! anyone can reproduce it (spec-56 feature 00 §4).
//!
//! **WHAT:** verifies the pins still hold, and regenerates when the
//! committed output no longer matches this declaration.
//!
//! **HOW:** the three outcomes are deliberately different:
//!
//! | `check()` says | meaning | what happens |
//! |---|---|---|
//! | `Ok` | output is current | nothing |
//! | `Stale` | the declaration moved | **regenerate** |
//! | anything else | the spec moved under the pin, or is unreadable | **fail the build** |
//!
//! A moved spec is fatal on purpose: regenerating against a document that changed
//! would rewrite this crate's types with nobody choosing to. Bump `spec_version`,
//! rebuild, and read the diff.
//!
//! Regenerate by hand with:  cargo run --bin genapi --features cli -- generate hetzner

use std::path::Path;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let spec = Path::new(&manifest).join("../../artefacts/cloud_providers/hetzner/openapi.json");

    // The spec artefact is not published to crates.io, so a consumer building
    // this crate from the registry has no spec to check against — and the
    // committed `src/generated/` is exactly what they should be compiling anyway.
    // Nothing to do, and failing here would break their build.
    if !spec.exists() {
        return;
    }

    // Rerun only when the spec or this declaration changes. Without these, cargo
    // watches every file in the package — and since this script writes into
    // `src/`, that would rebuild forever.
    println!("cargo:rerun-if-changed={}", spec.display());
    println!("cargo:rerun-if-changed=build.rs");

    let codegen = foundation_codegentools::generate()
        .provider("hetzner")
        .spec(&spec)
        .api_version("v1")
        .spec_version("1.0.0")
        .include_operations([
            "create_server",
            "get_server",
            "list_servers",
            "delete_server",
            "list_ssh_keys",
            "create_ssh_key",
        ])
        .crate_dir(&manifest);

    match codegen.check() {
        // Already what this declaration produces.
        Ok(()) => {}
        // The selection or the spec's shape moved — regenerate.
        Err(foundation_codegentools::CodegenError::Stale { .. }) => {
            codegen.write().expect("regenerate hetzner's API surface");
            println!("cargo:warning=regenerated hetzner's src/generated/ from {}", spec.display());
        }
        // A moved pin, an unreadable spec, a spec that will not canonicalise.
        Err(e) => panic!("{e}"),
    }
}
