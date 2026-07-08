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

pub mod client;
pub mod codec;
pub mod compression;
pub mod context;
pub mod envelope;
pub mod error;
pub mod error_writer;
pub mod interceptor;
#[cfg(feature = "auth")]
pub mod auth;
pub mod message;
pub mod router;

/// Native server connection-owner adapter (foundation_http `Serve`).
#[cfg(not(target_family = "wasm"))]
pub mod server;

pub use error_writer::ErrorWriter;
pub mod protocol;
pub mod transport;

// Proto code generation (Decision 10 Modes 1–2) lives in the build-time
// companion crate `foundation_connectrpc_codegen` (the tonic/tonic-build split)
// — add it under `[build-dependencies]`. Code-first generation (Mode 3) is the
// `service!`/`generate!` proc-macros re-exported below and stays in-crate.

pub use message::{Request, Response};
pub use router::{ConnectRpcHandler, HandlerOptions, RequestStream, Router};

/// Proc-macro for code-first ConnectRPC service generation (Feature 27,
/// Decision 10 Mode 3). See `foundation_macros::service` for docs.
pub use foundation_macros::{generate, service};

/// Re-export of the `buffa` protobuf crate as `foundation_connectrpc::buffa`.
///
/// Proto message types need a `buffa::Message` impl (normally emitted by
/// codegen; there is no derive). Hand-writing one — or working with buffa
/// messages directly — otherwise means adding a matching `buffa` dependency and
/// keeping its version in lockstep with ours. Re-exporting it here means a
/// consumer reaches the whole authoring surface through this crate:
/// `foundation_connectrpc::buffa::{Message, DefaultInstance, SizeCache}`,
/// `::buffa::encoding::{Tag, WireType, encode_varint, …}`, and even
/// `::buffa::bytes::{Buf, BufMut}` (buffa re-exports `bytes`). Pure-JSON /
/// code-first `codecs(json)` services need none of this — `JsonCodec` is bounded
/// on serde alone.
pub use buffa;

/// Re-exported client types for generated code.
pub use client::{BidiStream, Client, ClientOptions, ClientStream, ServerStream};

#[cfg(not(target_family = "wasm"))]
pub use server::ConnectRpcServe;

pub use protocol::{
    canonicalize_content_type, parse_connect_content_type, ClientExchange, HandlerExchange,
    ProtocolClient, ProtocolHandler,
};
pub use protocol::connect::{ConnectClient, ConnectHandler};
pub use protocol::grpc_web::{GrpcWebClient, GrpcWebHandler};

pub use interceptor::{
    Interceptor, InterceptorChain, RecoverInterceptor, StreamCall, StreamingClientFunc,
    StreamingHandlerFunc, UnaryCall, UnaryFunc, UnaryInterceptorFunc, UnaryReply,
};

#[cfg(feature = "auth")]
pub use auth::{
    authenticate_request, bearer_token, get_auth_artifact, get_auth_info, infer_procedure,
    infer_protocol, AuthFunc, AuthInfo, AuthzInterceptor, CompositeAuthenticator,
    JwtAuthenticator, PerProcedureAuth,
};

pub use transport::{
    check_compatible, requirements, CallRequirements, ClientConn, ConnReceiver, ConnSender,
    Frame, HandlerConn, MessageSink, MessageSource, ProtocolKind, Transport, TransportCapabilities,
    TransportError,
};

#[cfg(not(target_family = "wasm"))]
pub use transport::h1::H1Transport;

pub use envelope::{
    decode_grpc_timeout, encode_grpc_timeout, Envelope, EnvelopeError, EnvelopeReader,
    EnvelopeWriter,
};

pub use context::{
    CancelSignal, Ctx, IdempotencyLevel, Peer, RequestContext, Spec, StreamType,
};

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
