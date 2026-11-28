//! Generated wire types (`proto/ewe/durable/v1/durable.proto`).
//!
//! WHY: the RPC contract is defined once in protobuf and generated. Message
//! types come from `buffa-build`; the service trait, register function,
//! procedure paths and typed client come from `foundation_connectrpc_codegen`
//! (see `build.rs`).
//!
//! HOW: `include!` of the `OUT_DIR` files. The generated service file refers to
//! the messages by name, so it is included inside the package module.

/// `ewe.durable.v1`.
#[allow(
    clippy::all,
    clippy::pedantic,
    missing_docs,
    unused_qualifications,
    rustdoc::all
)]
pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/ewe.durable.v1.mod.rs"));

    /// `DurableStreamService`: create, append, read, subscribe, head, close,
    /// delete and fork.
    pub mod durable_stream_service {
        #[allow(unused_imports)]
        use super::*;
        include!(concat!(env!("OUT_DIR"), "/durable_stream_service.rs"));
    }
}
