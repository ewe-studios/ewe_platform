//! Build script — generates the agent-rpc wire types from `proto/`.
//!
//! WHY: the RPC surface uses generated types, never hand-written structs
//! (AGENTS.md "use the generated types").
//!
//! WHAT: `buffa-build` emits the `buffa::Message` (+ serde JSON) message types
//! for `ewe.agent_rpc.v1`; `foundation_connectrpc_codegen` emits one service
//! file per service (trait + register fn + typed client + procedure paths), so
//! each service's `pub mod procedure` lands in its own module.
//!
//! HOW: both need `protoc` (`PROTOC` env var, else `protoc` on PATH) — the
//! same requirement as the `BuildKit` protos in `foundation_deployment_docker`.

use prost::Message;
use prost_types::FileDescriptorSet;

const PROTO: &str = "ewe/agent_rpc/v1/agent.proto";

fn main() {
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
        .expect("buffa-build: compile agent-rpc protos");

    generate_services(&root, &file);
    println!("cargo:rerun-if-changed={root}");
    println!("cargo:rerun-if-env-changed=PROTOC");
}

fn generate_services(root: &str, file: &str) {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let protoc = std::env::var("PROTOC").unwrap_or_else(|_| "protoc".to_string());

    let desc_path = format!("{out_dir}/agent_rpc_descriptors.bin");
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
        .expect("agent.proto descriptor");

    for (service, out_name) in [
        ("AgentService", "agent_service.rs"),
        ("BootstrapService", "bootstrap_service.rs"),
    ] {
        let mut one = descriptor.clone();
        one.service.retain(|s| s.name.as_deref() == Some(service));
        assert_eq!(one.service.len(), 1, "service {service} not found");
        let code = foundation_connectrpc_codegen::generate_services(&[one]);
        std::fs::write(format!("{out_dir}/{out_name}"), code).expect("write service code");
    }
}
