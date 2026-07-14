//! Build script — generates BuildKit gRPC message types from vendored protos.
//!
//! WHY: The `buildkit` feature talks to buildkitd over gRPC (via our
//! `foundation_connectrpc`). Its request/response messages (SolveRequest,
//! StatusResponse, InfoResponse, the LLB `Op` DAG, …) are defined by the
//! upstream BuildKit `.proto` files. We generate `buffa::Message` Rust types
//! from them so `ProcedureCodecs::defaults()` (proto + json) accepts them — the
//! same `buffa` runtime `foundation_connectrpc`'s `ProtoCodec` already uses.
//!
//! WHAT: When the `buildkit` feature is on, runs `buffa-build` over the vendored
//! Control-service proto graph in `specs/buildkit/` and emits per-package Rust
//! into `OUT_DIR`, wired together by `src/buildkit/generated.rs`.
//!
//! HOW: `buffa-build` shells out to `protoc` to produce a `FileDescriptorSet`,
//! then `buffa-codegen` emits the Rust. Requires `protoc` on PATH at build time
//! (only for the optional `buildkit` feature). No-op otherwise.

fn main() {
    #[cfg(feature = "buildkit")]
    generate_buildkit();
}

#[cfg(feature = "buildkit")]
fn generate_buildkit() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let root = format!("{manifest}/specs/buildkit");
    let bk = format!("{root}/github.com/moby/buildkit");

    let fsutil = format!("{root}/github.com/tonistiigi/fsutil/types");
    let files = [
        // Control service (Solve/Status/Info/DiskUsage/Prune/ListWorkers).
        format!("{bk}/api/services/control/control.proto"),
        format!("{bk}/api/types/worker.proto"),
        format!("{bk}/solver/pb/ops.proto"),
        format!("{bk}/sourcepolicy/pb/policy.proto"),
        format!("{root}/google/rpc/status.proto"),
        // Session sidecar services (served by the client during a Solve).
        format!("{bk}/session/filesync/filesync.proto"),
        format!("{bk}/session/auth/auth.proto"),
        format!("{bk}/session/secrets/secrets.proto"),
        format!("{bk}/session/sshforward/ssh.proto"),
        format!("{fsutil}/wire.proto"),
        format!("{fsutil}/stat.proto"),
    ];

    buffa_build::Config::new()
        .files(&files)
        .includes(&[root.as_str()])
        .generate_json(true)
        .compile()
        .expect("buffa-build: compile BuildKit control protos");

    generate_services(&root, &files);

    println!("cargo:rerun-if-changed={root}");
}

/// Generate ConnectRPC service code (trait + register fn + typed client) for the
/// BuildKit services, one file per service so each service's `pub mod procedure`
/// lands in its own module (avoids duplicate-module collisions for multi-service
/// protos). Message types are the buffa types generated above and brought into
/// scope by `src/buildkit/generated.rs`.
#[cfg(feature = "buildkit")]
fn generate_services(root: &str, files: &[String]) {
    use prost::Message;
    use prost_types::FileDescriptorSet;

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let protoc = std::env::var("PROTOC").unwrap_or_else(|_| "protoc".to_string());

    // 1. Ask protoc for a self-contained FileDescriptorSet (with imports).
    let desc_path = format!("{out_dir}/buildkit_service_descriptors.bin");
    let mut cmd = std::process::Command::new(&protoc);
    cmd.arg("--include_imports")
        .arg(format!("--descriptor_set_out={desc_path}"))
        .arg(format!("-I{root}"));
    for f in files {
        cmd.arg(f);
    }
    let status = cmd.status().expect("run protoc for descriptor set");
    assert!(status.success(), "protoc descriptor generation failed");

    let bytes = std::fs::read(&desc_path).expect("read descriptor set");
    let fds = FileDescriptorSet::decode(bytes.as_slice()).expect("decode descriptor set");

    // 2. (proto file basename, service name, output module file). Only the
    //    services we drive/serve.
    let targets: &[(&str, &str, &str)] = &[
        ("control.proto", "Control", "control_service.rs"),
        ("filesync.proto", "FileSync", "filesync_service.rs"),
        ("auth.proto", "Auth", "auth_service.rs"),
        ("secrets.proto", "Secrets", "secrets_service.rs"),
        ("ssh.proto", "SSH", "ssh_service.rs"),
    ];

    for (file_suffix, service_name, out_name) in targets {
        let file = fds
            .file
            .iter()
            .find(|f| f.name.as_deref().map_or(false, |n| n.ends_with(file_suffix)))
            .unwrap_or_else(|| panic!("descriptor for {file_suffix} not found"));

        // Retain only the one service so its `pub mod procedure` is unique.
        let mut one = file.clone();
        one.service.retain(|s| s.name.as_deref() == Some(service_name));
        assert_eq!(one.service.len(), 1, "service {service_name} not found in {file_suffix}");

        let code = foundation_connectrpc_codegen::generate_services(&[one]);
        std::fs::write(format!("{out_dir}/{out_name}"), code).expect("write service code");
    }
}
