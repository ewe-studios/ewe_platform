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

    let files = [
        format!("{bk}/api/services/control/control.proto"),
        format!("{bk}/api/types/worker.proto"),
        format!("{bk}/solver/pb/ops.proto"),
        format!("{bk}/sourcepolicy/pb/policy.proto"),
        format!("{root}/google/rpc/status.proto"),
    ];

    buffa_build::Config::new()
        .files(&files)
        .includes(&[root.as_str()])
        .generate_json(true)
        .compile()
        .expect("buffa-build: compile BuildKit control protos");

    println!("cargo:rerun-if-changed={root}");
}
