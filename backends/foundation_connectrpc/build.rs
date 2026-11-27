//! Build script — generates the Durable Streams wire types from `proto/`.
//!
//! WHY: the service uses generated types, never hand-written structs
//! (`AGENTS.md` rule 5).
//!
//! WHAT: `buffa-build` emits the `buffa::Message` types for `ewe.durable.v1`.
//! `foundation_connectrpc_codegen` emits the service trait, its register
//! function, the typed client and the procedure paths.
//!
//! HOW: both need `protoc` (`PROTOC` env var, else `protoc` on PATH) — the
//! same requirement as `foundation_agent_rpc`.

use prost::Message;
use prost_types::FileDescriptorSet;

const PROTO: &str = "ewe/durable/v1/durable.proto";

fn main() {
    emit_cranelift_cfg();
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let root = format!("{manifest}/proto");
    let file = format!("{root}/{PROTO}");

    buffa_build::Config::new()
        .files(&[file.as_str()])
        .includes(&[root.as_str()])
        .generate_json(true)
        // Owned messages only: the zero-copy views are not used here.
        .generate_views(false)
        .compile()
        .expect("buffa-build: compile the Durable Streams protos");

    generate_service(&root, &file);
    println!("cargo:rerun-if-changed={root}");
    println!("cargo:rerun-if-env-changed=PROTOC");
}

/// Emit `cargo::rustc-cfg=cranelift_backend` when the Cranelift backend builds
/// this crate.
///
/// WHY: Cranelift cannot unwind a panic, so a test that catches one aborts the
/// process instead. Tests that need `catch_unwind` gate on this cfg. This is
/// `foundation_buildtools::set_cranelift_cfg`'s rule, kept here because the
/// crate takes no build-dependency on that helper.
///
/// WHAT: the backend comes from `-Zcodegen-backend=cranelift` in the rustflags,
/// or from the `CARGO_PROFILE_<NAME>_CODEGEN_BACKEND` override for the active
/// profile.
///
/// HOW: the active profile is the directory above `build` in `OUT_DIR`, matching
/// the build helper. The workspace manifest sets no `codegen-backend`, so the
/// helper's manifest fallback is not needed.
fn emit_cranelift_cfg() {
    println!("cargo::rustc-check-cfg=cfg(cranelift_backend)");
    println!("cargo::rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");

    let rustflags = std::env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    let mut cranelift = rustflags.contains("codegen-backend=cranelift");

    if let Some(out_dir) = std::env::var_os("OUT_DIR") {
        let parts: Vec<&str> = std::path::Path::new(&out_dir)
            .iter()
            .filter_map(|part| part.to_str())
            .collect();
        let profile = parts
            .iter()
            .rposition(|part| *part == "build")
            .and_then(|build| build.checked_sub(1))
            .and_then(|index| parts.get(index))
            // `debug` is the directory for `dev` and `test`; a custom
            // profile uses its own name.
            .map(|dir| if *dir == "debug" { "dev" } else { *dir });
        if let Some(profile) = profile {
            let key = format!(
                "CARGO_PROFILE_{}_CODEGEN_BACKEND",
                profile.to_uppercase().replace('-', "_")
            );
            println!("cargo::rerun-if-env-changed={key}");
            cranelift |= std::env::var(&key).is_ok_and(|value| value == "cranelift");
        }
    }

    if cranelift {
        println!("cargo::rustc-cfg=cranelift_backend");
    }
}

fn generate_service(root: &str, file: &str) {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let protoc = std::env::var("PROTOC").unwrap_or_else(|_| "protoc".to_string());

    let desc_path = format!("{out_dir}/durable_descriptors.bin");
    let status = std::process::Command::new(&protoc)
        .arg("--include_imports")
        .arg(format!("--descriptor_set_out={desc_path}"))
        .arg(format!("-I{root}"))
        .arg(file)
        .status()
        .expect("run protoc for the descriptor set");
    assert!(status.success(), "protoc descriptor generation failed");

    let bytes = std::fs::read(&desc_path).expect("read descriptor set");
    let fds = FileDescriptorSet::decode(bytes.as_slice()).expect("decode descriptor set");
    let descriptor = fds
        .file
        .iter()
        .find(|f| f.name.as_deref() == Some(PROTO))
        .expect("durable.proto descriptor");

    let mut one = descriptor.clone();
    one.service
        .retain(|s| s.name.as_deref() == Some("DurableStreamService"));
    assert_eq!(
        one.service.len(),
        1,
        "service DurableStreamService not found"
    );
    let code = foundation_connectrpc_codegen::generate_services(&[one]);
    std::fs::write(format!("{out_dir}/durable_stream_service.rs"), code)
        .expect("write service code");
}
