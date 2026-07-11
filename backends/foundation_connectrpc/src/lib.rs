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

// Cross-platform modules (native + wasm).
pub mod shared;

// Native-only modules (TCP server, h2/h3 transports).
#[cfg(not(target_family = "wasm"))]
pub mod native;

pub use shared::error_writer::ErrorWriter;

// Proto code generation (Decision 10 Modes 1–2) lives in the build-time
// companion crate `foundation_connectrpc_codegen` (the tonic/tonic-build split)
// — add it under `[build-dependencies]`. Code-first generation (Mode 3) is the
// `service!`/`generate!` proc-macros re-exported below and stays in-crate.

pub use shared::message::{Request, Response};
pub use shared::router::{ConnectRpcHandler, HandlerOptions, RequestStream, Router};

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
pub use shared::client::{BidiStream, Client, ClientOptions, ClientStream, ServerStream};

#[cfg(not(target_family = "wasm"))]
pub use native::server::ConnectRpcServe;
#[cfg(not(target_family = "wasm"))]
pub use native::h2_serve::ConnectRpcServeH2;

pub use shared::protocol::{
    canonicalize_content_type, parse_connect_content_type, ClientExchange, HandlerExchange,
    ProtocolClient, ProtocolHandler,
};
pub use shared::protocol::connect::{ConnectClient, ConnectHandler};
pub use shared::protocol::grpc::{GrpcClient, GrpcHandler};
pub use shared::protocol::grpc_web::{GrpcWebClient, GrpcWebHandler};

pub use shared::interceptor::{
    Interceptor, InterceptorChain, RecoverInterceptor, StreamCall, StreamingClientFunc,
    StreamingHandlerFunc, UnaryCall, UnaryFunc, UnaryInterceptorFunc, UnaryReply,
};

#[cfg(feature = "auth")]
pub use shared::auth::{
    authenticate_request, bearer_token, get_auth_artifact, get_auth_info, infer_procedure,
    infer_protocol, AuthFunc, AuthInfo, AuthzInterceptor, CompositeAuthenticator,
    JwtAuthenticator, PerProcedureAuth,
};

pub use shared::transport::{
    check_compatible, requirements, CallRequirements, ClientConn, ConnReceiver, ConnSender,
    Frame, HandlerConn, MessageSink, MessageSource, ProtocolKind, Transport, TransportCapabilities,
    TransportError,
};

// H1 + WS transports are cross-platform (they ride foundation_netio's
// platform-selecting client/websocket wrappers), so they are exported on wasm too.
pub use shared::transport::h1::H1Transport;
pub use shared::transport::ws::WsTransport;
#[cfg(not(target_family = "wasm"))]
pub use native::transport::h2::H2Transport;
#[cfg(all(not(target_family = "wasm"), feature = "h3"))]
pub use native::transport::h3::H3Transport;

pub use shared::envelope::{
    decode_grpc_timeout, encode_grpc_timeout, Envelope, EnvelopeError, EnvelopeReader,
    EnvelopeWriter,
};

pub use shared::context::{
    CancelSignal, Ctx, IdempotencyLevel, Peer, RequestContext, Spec, StreamType,
};

pub use shared::codec::{Codec, CodecError, CodecFor, JsonCodec, ProcedureCodecs, ProtoCodec};
pub use shared::compression::{
    negotiate_compression, with_worker_buffer, worker_freeze, BufferPool, CompressionError,
    CompressionRegistry, Compressor, GzipCompressor, GzipStreamCompressor, NegotiatedCompression,
    SizeLimits,
};

#[cfg(feature = "brotli")]
pub use shared::compression::BrotliCompressor;
#[cfg(feature = "zstd")]
pub use shared::compression::ZstdCompressor;
pub use shared::error::{
    code_of, wrap_if_context, wrap_if_h2c, wrap_if_rst, wrap_if_uncoded, Code, ConnectError,
    ConnectResult, EndStreamResponse, ErrorDetail, WireError, WireErrorDetail, ERRSTACKS_TYPE_URL,
};

#[cfg(feature = "arrow")]
pub use shared::codec::ArrowCodec;
