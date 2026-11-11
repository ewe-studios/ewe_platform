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

/// Regenerate the shared Cloudflare types and compare them with
/// `foundation_cloudflare_types`'s committed `src/generated.rs`.
///
/// This uses `check_shared_types`, which skips the 43 group crates and the
/// umbrella shim, so the test stays fast in a dev build. The build script of
/// `foundation_deployment_cloudflare` still runs the full `check()` on every
/// build, which covers those files.
#[test]
fn cloudflare_types_are_generated() {
    cloudflare_declaration()
        .check_shared_types()
        .expect("the committed Cloudflare wire types are what the generator writes");
}

/// The full check the Cloudflare build script runs passes on the committed tree.
///
/// WHY: `check()` renders the group crates into a scratch tree and compares them
/// with the committed ones. That scratch tree holds no sibling crates, so a
/// generator that read the group manifests' versions from next to its output
/// failed the build. The build script only proves it after a full cargo build of
/// the umbrella crate; this test proves it in the generator's own suite.
///
/// It is slower than `cloudflare_types_are_generated` because it renders and
/// formats all 43 group crates. That is the point: it is the same work the build
/// script does.
#[test]
fn the_full_cloudflare_check_is_current() {
    cloudflare_declaration()
        .check()
        .expect("the committed group crates and umbrella shim are what the generator writes");
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
