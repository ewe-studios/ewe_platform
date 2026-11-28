//! Google Cloud Key Management Service (KMS) over gRPC (spec 92 feature 03).
//!
//! WHY: the REST path of `foundation_auth` needs a Google Cloud credential,
//! so a test or a continuous integration (CI) job cannot run it. The gRPC path
//! reaches a Cloud KMS emulator on the same machine, which accepts the same
//! calls without a credential.
//!
//! WHY A SEPARATE CRATE: this backend sits above `foundation_auth` and
//! `foundation_connectrpc`, so it keeps the Cloud KMS protobuf surface out of
//! the key-store trait. The old optional `auth` edge of
//! `foundation_connectrpc` closed a Cargo cycle here; spec 92 feature 05
//! removed it (issue #1107).
//!
//! WHAT: [`GcpKmsGrpcBackend`], a [`KmsBackend`](foundation_auth::keystore::KmsBackend)
//! over the Cloud KMS gRPC API, its [`GcpKmsGrpcConfig`], the generated Cloud
//! KMS messages in [`proto`], and the key management calls that a test needs.
//!
//! HOW: one typed `Client` per call over the h2c transport of
//! `foundation_connectrpc`, with the gRPC protocol and the binary protobuf
//! codec. The access token travels in the `authorization` metadata. A
//! non-zero `grpc-status` becomes a key-store storage error with the code and
//! the message.

#![cfg(not(target_family = "wasm"))]

mod backend;

pub use backend::{GcpKmsGrpcBackend, GcpKmsGrpcConfig};

/// The generated Cloud KMS messages.
///
/// The include file nests the packages as the proto package tree nests them,
/// so a message is `proto::google::cloud::kms::v1::EncryptRequest`, an
/// operation is `proto::google::longrunning::Operation`, and a failure is
/// `proto::google::rpc::Status`.
#[allow(
    clippy::all,
    clippy::pedantic,
    dead_code,
    non_camel_case_types,
    non_snake_case,
    unused_imports,
    unused_qualifications,
    rustdoc::all
)]
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/kms_proto.rs"));
}
