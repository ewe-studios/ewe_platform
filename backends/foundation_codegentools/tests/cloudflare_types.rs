//! The Cloudflare wire types in `foundation_cloudflare_types` come from the
//! generator, and the committed output is what the generator writes (feature 52).
//!
//! WHY: `foundation_db` used to carry hand-written D1 and R2 types, and build the
//! D1 request body with `json!`. Rule 5 says to use generated types. These tests
//! hold that line: regenerate and compare byte for byte, and fail if a type is
//! written by hand again.

use std::path::{Path, PathBuf};

use foundation_codegentools::{cloudflare_wire_types, generate};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The same declaration `backends/foundation_deployment_cloudflare/build.rs`
/// checks, so a pass here means that build script finds nothing stale.
fn cloudflare_declaration() -> foundation_codegentools::Codegen {
    let root = workspace_root();
    let codegen = generate()
        .provider("cloudflare")
        .spec(root.join("artefacts/cloud_providers/cloudflare/openapi.json"))
        .split_groups(
            root.join("backends"),
            root.join("backends/foundation_cloudflare_types"),
        )
        .crate_dir(root.join("backends/foundation_deployment_cloudflare"));
    cloudflare_wire_types(codegen, &root).expect("read the cloudflare overlays")
}

/// Regenerate Cloudflare into a scratch tree and compare it with what is
/// committed: the shim, every group crate, and `foundation_cloudflare_types`'s
/// `src/generated.rs`.
#[test]
fn cloudflare_types_are_generated() {
    cloudflare_declaration()
        .check()
        .expect("the committed Cloudflare output is what the generator writes");
}

/// The D1 request body and the R2 listing types are in the generated file, not
/// in a hand-written one.
#[test]
fn the_d1_and_r2_wire_types_are_in_the_generated_file() {
    let generated = std::fs::read_to_string(
        workspace_root().join("backends/foundation_cloudflare_types/src/generated.rs"),
    )
    .expect("read generated.rs");
    for needle in [
        "#[serde(untagged)]\npub enum D1BatchQuery {",
        "pub struct D1SingleQuery {",
        "pub type D1QueryDatabaseResponse = ApiEnvelope<Vec<D1QueryResultResponse>>;",
        "pub struct R2ListObjectsResponse {",
        "pub struct R2ObjectInfo {",
    ] {
        assert!(generated.contains(needle), "generated.rs lacks `{needle}`");
    }
}

/// `lib.rs` of the types crate defines no type of its own.
#[test]
fn the_types_crate_has_no_hand_written_type() {
    let lib = std::fs::read_to_string(
        workspace_root().join("backends/foundation_cloudflare_types/src/lib.rs"),
    )
    .expect("read lib.rs");
    for keyword in ["pub struct ", "pub enum ", "pub type ", "pub trait "] {
        assert!(
            !lib.contains(keyword),
            "lib.rs defines a type by hand (`{keyword}`); change the generator or an overlay"
        );
    }
}
