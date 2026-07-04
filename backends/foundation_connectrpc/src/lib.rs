//! `foundation_connectrpc` — ConnectRPC for the EWE platform.
//!
//! An independent Rust port of the Connect protocol (connect-go is the design
//! reference), built on the platform's foundation crates: `foundation_errstacks`
//! for the error model, `foundation_netio` for HTTP/wire types, and valtron for
//! execution.
//!
//! This crate is being built feature-by-feature (see `specifications/41-connectrpc`).
//! The first landed piece is the protocol-agnostic **error model** (Decision 03):
//! see [`error`].

pub mod codec;
pub mod compression;
pub mod error;

pub use codec::{Codec, CodecError, CodecFor, JsonCodec, ProcedureCodecs, ProtoCodec};
pub use compression::{
    negotiate_compression, with_worker_buffer, worker_freeze, BufferPool, CompressionError,
    CompressionRegistry, Compressor, GzipCompressor, GzipStreamCompressor, NegotiatedCompression,
    SizeLimits,
};

#[cfg(feature = "brotli")]
pub use compression::BrotliCompressor;
#[cfg(feature = "zstd")]
pub use compression::ZstdCompressor;
pub use error::{
    code_of, wrap_if_context, wrap_if_h2c, wrap_if_rst, wrap_if_uncoded, Code, ConnectError,
    ConnectResult, EndStreamResponse, ErrorDetail, WireError, WireErrorDetail, ERRSTACKS_TYPE_URL,
};

#[cfg(feature = "arrow")]
pub use codec::ArrowCodec;
