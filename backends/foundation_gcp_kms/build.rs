//! Build script: generate the Cloud KMS protobuf messages.
//!
//! WHY: the backend sends Cloud KMS requests as protobuf, and the messages
//! must come from the API contract, not from hand-written structs (root rule
//! 5). The vendored protos under `proto/` are the contract.
//!
//! WHAT: `buffa-build` emits the owned `buffa::Message` types for the Cloud
//! KMS service, its resources, the long-running operation answer and
//! `google.rpc.Status` into `OUT_DIR`, with one include file that mirrors the
//! proto package tree.
//!
//! HOW: `buffa-build` runs `protoc` (`PROTOC` environment variable first, else
//! `protoc` on `PATH`). It reads the `google.protobuf` well-known types from
//! the include directory of `protoc` and maps them to `buffa-types`. No
//! service stubs are generated: the backend calls one typed client per method.

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let root = format!("{manifest}/proto");
    let kms = format!("{root}/google/cloud/kms/v1");
    // The long-running operation answer and its status type: `DeleteCryptoKey`
    // returns `google.longrunning.Operation`, and that message carries a
    // `google.rpc.Status` when it fails.
    let files = [
        format!("{kms}/service.proto"),
        format!("{kms}/resources.proto"),
        format!("{root}/google/longrunning/operations.proto"),
        format!("{root}/google/rpc/status.proto"),
    ];

    buffa_build::Config::new()
        .files(&files)
        .includes(&[root.as_str()])
        // The client speaks binary protobuf only, so the JSON impls are not
        // generated and no serde feature is needed.
        .generate_json(false)
        // Owned messages only: the client sends and receives owned values, so
        // the zero-copy views are not used and only cost build time.
        .generate_views(false)
        // One include file that nests the generated packages as the proto
        // package tree nests them.
        .include_file("kms_proto.rs")
        .compile()
        .expect("buffa-build: compile the Cloud KMS protos");

    println!("cargo:rerun-if-changed={root}");
    println!("cargo:rerun-if-env-changed=PROTOC");
}
