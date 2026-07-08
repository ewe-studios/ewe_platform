//! `foundation_connectrpc_codegen` — build-time ConnectRPC code generation.
//!
//! WHY: `.proto` files are one source of truth for a ConnectRPC service. This
//! crate turns proto file descriptors into the same artifacts the code-first
//! `#[connectrpc::service]` macro emits — procedure constants, service traits,
//! registration functions, and typed clients — so proto-first and code-first
//! users land on an identical runtime surface.
//!
//! WHAT: One generator with two front doors:
//!   - [`generate_services`] (Mode 2 core) — file descriptors → Rust source.
//!     The `protoc-gen-connect-ewe` binary feeds `protoc`'s
//!     `CodeGeneratorRequest` straight into it.
//!   - [`build::Config`] (Mode 1) — a `build.rs` helper that runs prost-build
//!     for message types and this generator for services in one pass.
//!
//! HOW: This is the build-time companion to the `foundation_connectrpc` runtime
//! crate — the same split as `tonic` / `tonic-build` and `prost` / `prost-build`.
//! It depends only on the prost toolchain and emits Rust *source strings*; the
//! generated code resolves its `connectrpc::…` paths against the consumer's
//! `foundation_connectrpc` dependency, so there is no dependency edge back to
//! the runtime crate. Add it under `[build-dependencies]`.
//!
//! Code-first generation (Mode 3) needs none of this — it is the
//! `service!`/`generate!` proc-macros re-exported from `foundation_connectrpc`.

pub mod build;
mod generator;

pub use generator::generate_services;
