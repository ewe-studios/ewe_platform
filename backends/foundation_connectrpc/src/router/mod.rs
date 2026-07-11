//! Router, registration, and multi-protocol dispatch (Decision 08).
//!
//! WHY: The server spine. One [`Router`] holds every procedure keyed by its
//! full leading-slash path (R1) and serves all three protocols (Connect, gRPC,
//! gRPC-Web) from a **single** foundation_http prefix route — protocol is
//! detected per request from `Content-Type`, never per registration. Type
//! erasure at the byte boundary (see [`erased`]) keeps the map generic-free.
//!
//! WHAT: [`Router`] + its four registration methods (`unary`/`server_stream`/
//! `client_stream`/`bidi_stream`), [`HandlerOptions`], the [`ProcedureMeta`]
//! dispatch view, and [`ConnectRpcHandler`] (the frozen prefix route — see
//! [`dispatch`]). Generated code (Decision 10) calls the registration methods.
//!
//! HOW: registration builds the innermost erased func, composes the interceptor
//! chain (global ++ per-procedure) around it **once**, and freezes it into a
//! [`HandlerEntry`]. [`Router::into_handler`] consumes the router (Q10 — no
//! post-build mutation).

mod dispatch;
mod erased;

use std::future::Future;
use std::sync::Arc;

use foundation_netio::shared::http::SimpleHeaders;
use std::any::Any;
use std::collections::HashMap;

use crate::codec::ProcedureCodecs;
use crate::compression::{CompressionRegistry, Compressor, SizeLimits};
use crate::context::{Ctx, IdempotencyLevel, Spec, StreamType};
use crate::error::{ConnectError, ConnectResult};
use crate::interceptor::{
    Interceptor, InterceptorChain, RecoverInterceptor, StreamingHandlerFunc, UnaryFunc,
};
use crate::message::{Request, Response};
use crate::protocol::{connect::ConnectHandler, grpc::GrpcHandler, grpc_web::GrpcWebHandler, ProtocolHandler};
use crate::transport::DEFAULT_PIPE_DEPTH;

use erased::{
    bidi_stream_handler, client_stream_handler, codec_meta, server_stream_handler, unary_handler,
    ErasedStreamHandler, ErasedUnaryHandler, StreamHandlerImpl, UnaryHandlerImpl,
};

pub use dispatch::ConnectRpcHandler;
pub use erased::RequestStream;
pub(crate) use erased::ProcedureMeta;

/// Per-procedure (and global) handler configuration (Decision 08 §HandlerOptions).
/// Codecs are **not** here — they are installed only through the registration
/// `ProcedureCodecs` parameter (Decision 02); the client picks per request.
pub struct HandlerOptions {
    /// Seam interceptors (bytes + metadata), composed closest to the handler.
    pub interceptors: Vec<Arc<dyn Interceptor>>,
    /// Per-procedure compression registry (overrides the router's global one).
    pub compression: Option<CompressionRegistry>,
    /// Read/send/compress size policy.
    pub limits: SizeLimits,
    /// Replay safety (drives GET eligibility).
    pub idempotency: IdempotencyLevel,
    /// Require `Connect-Protocol-Version: 1` on unary POST (Decision 05 P7).
    pub require_connect_protocol_header: bool,
    /// Seam pipe depth (Decision 11; default [`DEFAULT_PIPE_DEPTH`]).
    pub pipe_depth: usize,
}

impl Default for HandlerOptions {
    fn default() -> Self {
        Self {
            interceptors: Vec::new(),
            compression: None,
            limits: SizeLimits::default(),
            idempotency: IdempotencyLevel::Unknown,
            require_connect_protocol_header: false,
            pipe_depth: DEFAULT_PIPE_DEPTH,
        }
    }
}

impl HandlerOptions {
    /// New options with framework defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the interceptor list.
    #[must_use]
    pub fn with_interceptors(mut self, interceptors: Vec<Arc<dyn Interceptor>>) -> Self {
        self.interceptors = interceptors;
        self
    }

    /// Set the max decompressed request size (`0` = unlimited).
    #[must_use]
    pub fn with_read_max_bytes(mut self, n: usize) -> Self {
        self.limits.read_max_bytes = n;
        self
    }

    /// Set the max (post-compression) response size (`0` = unlimited).
    #[must_use]
    pub fn with_send_max_bytes(mut self, n: usize) -> Self {
        self.limits.send_max_bytes = n;
        self
    }

    /// Set the procedure's replay safety.
    #[must_use]
    pub fn with_idempotency(mut self, level: IdempotencyLevel) -> Self {
        self.idempotency = level;
        self
    }

    /// Require the `Connect-Protocol-Version` header on unary POST (P7).
    #[must_use]
    pub fn with_require_connect_protocol_header(mut self, required: bool) -> Self {
        self.require_connect_protocol_header = required;
        self
    }

    /// Register a compressor on this procedure's registry (creating it, seeded
    /// with gzip, if absent).
    #[must_use]
    pub fn with_compression(mut self, _name: &str, compressor: Arc<dyn Compressor>) -> Self {
        let mut registry = self.compression.take().unwrap_or_default();
        registry.register(compressor);
        self.compression = Some(registry);
        self
    }

    /// Set the seam pipe depth (Decision 11; default 4).
    #[must_use]
    pub fn with_pipe_depth(mut self, n: usize) -> Self {
        self.pipe_depth = n;
        self
    }

    /// Install a panic-recovery interceptor (Decision 08 R? / Decision 04 RS4):
    /// the handler runs under `catch_unwind`; on panic `handler` maps the payload
    /// to a [`ConnectError`].
    #[must_use]
    pub fn with_recover<F>(mut self, handler: F) -> Self
    where
        F: Fn(&Ctx, &Spec, &SimpleHeaders, Box<dyn Any + Send>) -> ConnectError
            + Clone
            + Send
            + Sync
            + 'static,
    {
        self.interceptors
            .push(Arc::new(RecoverInterceptor::new(handler)));
        self
    }
}

/// Which streaming shape an entry drives (the erased handler is byte-level; the
/// `Spec`'s `StreamType` selects the dispatch/validation path).
pub(crate) enum HandlerKind {
    Unary(Arc<dyn ErasedUnaryHandler>),
    ServerStream(Arc<dyn ErasedStreamHandler>),
    ClientStream(Arc<dyn ErasedStreamHandler>),
    BidiStream(Arc<dyn ErasedStreamHandler>),
}

/// One registered procedure (Decision 08).
pub(crate) struct HandlerEntry {
    pub(crate) spec: Spec,
    pub(crate) handler: HandlerKind,
    pub(crate) options: HandlerOptions,
    pub(crate) protocol_handlers: Vec<Box<dyn ProtocolHandler>>,
    pub(crate) codec_meta: Arc<dyn ProcedureMeta>,
}

/// Path-keyed multi-protocol router (Decision 08). Holds no codec state — each
/// procedure's [`ProcedureCodecs`] table lives inside its erased wrapper.
pub struct Router {
    handlers: HashMap<String, HandlerEntry>,
    compression: Arc<CompressionRegistry>,
    global_options: Vec<Arc<dyn Interceptor>>,
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    /// A router with the default compression registry (gzip) and no global
    /// interceptors.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            compression: Arc::new(CompressionRegistry::new()),
            global_options: Vec::new(),
        }
    }

    /// Override the shared compression registry all procedures inherit.
    #[must_use]
    pub fn with_compression(mut self, compression: Arc<CompressionRegistry>) -> Self {
        self.compression = compression;
        self
    }

    /// Install interceptors that wrap **every** procedure (composed outside the
    /// per-procedure ones).
    #[must_use]
    pub fn with_global_interceptors(mut self, interceptors: Vec<Arc<dyn Interceptor>>) -> Self {
        self.global_options = interceptors;
        self
    }

    /// Register a unary procedure (Decision 04 unary shape).
    ///
    /// `procedure` is the full `/package.Service/Method` path (R1, leading slash).
    pub fn unary<Req, Res, F, Fut>(
        &mut self,
        procedure: &str,
        codecs: ProcedureCodecs<Req, Res>,
        handler: F,
        options: HandlerOptions,
    ) where
        Req: Send + 'static,
        Res: Send + 'static,
        F: Fn(Ctx, Request<Req>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ConnectResult<Response<Res>>> + Send + 'static,
    {
        let codecs = Arc::new(codecs);
        let inner = unary_handler(codecs.clone(), handler);
        let func = self.compose_unary(&options, inner);
        let entry = self.build_entry(
            procedure,
            StreamType::Unary,
            HandlerKind::Unary(Arc::new(UnaryHandlerImpl::new(func))),
            codec_meta(codecs),
            options,
        );
        self.handlers.insert(procedure.to_string(), entry);
    }

    /// Register a server-streaming procedure (one request, a `Stream` of responses).
    pub fn server_stream<Req, Res, S, F, Fut>(
        &mut self,
        procedure: &str,
        codecs: ProcedureCodecs<Req, Res>,
        handler: F,
        options: HandlerOptions,
    ) where
        Req: Send + 'static,
        Res: Send + 'static,
        S: futures::Stream<Item = ConnectResult<Res>> + Send + 'static,
        F: Fn(Ctx, Request<Req>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ConnectResult<S>> + Send + 'static,
    {
        let codecs = Arc::new(codecs);
        let inner = server_stream_handler(codecs.clone(), handler);
        let func = self.compose_streaming(&options, inner);
        let entry = self.build_entry(
            procedure,
            StreamType::ServerStream,
            HandlerKind::ServerStream(Arc::new(StreamHandlerImpl::new(func))),
            codec_meta(codecs),
            options,
        );
        self.handlers.insert(procedure.to_string(), entry);
    }

    /// Register a client-streaming procedure (a `Stream` of requests, one response).
    pub fn client_stream<Req, Res, F, Fut>(
        &mut self,
        procedure: &str,
        codecs: ProcedureCodecs<Req, Res>,
        handler: F,
        options: HandlerOptions,
    ) where
        Req: Send + 'static,
        Res: Send + 'static,
        F: Fn(Ctx, RequestStream<Req>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ConnectResult<Response<Res>>> + Send + 'static,
    {
        let codecs = Arc::new(codecs);
        let inner = client_stream_handler(codecs.clone(), handler);
        let func = self.compose_streaming(&options, inner);
        let entry = self.build_entry(
            procedure,
            StreamType::ClientStream,
            HandlerKind::ClientStream(Arc::new(StreamHandlerImpl::new(func))),
            codec_meta(codecs),
            options,
        );
        self.handlers.insert(procedure.to_string(), entry);
    }

    /// Register a bidi-streaming procedure (a `Stream` in, a `Stream` out).
    pub fn bidi_stream<Req, Res, S, F, Fut>(
        &mut self,
        procedure: &str,
        codecs: ProcedureCodecs<Req, Res>,
        handler: F,
        options: HandlerOptions,
    ) where
        Req: Send + 'static,
        Res: Send + 'static,
        S: futures::Stream<Item = ConnectResult<Res>> + Send + 'static,
        F: Fn(Ctx, RequestStream<Req>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ConnectResult<S>> + Send + 'static,
    {
        let codecs = Arc::new(codecs);
        let inner = bidi_stream_handler(codecs.clone(), handler);
        let func = self.compose_streaming(&options, inner);
        let entry = self.build_entry(
            procedure,
            StreamType::BidiStream,
            HandlerKind::BidiStream(Arc::new(StreamHandlerImpl::new(func))),
            codec_meta(codecs),
            options,
        );
        self.handlers.insert(procedure.to_string(), entry);
    }

    /// Consume the router, producing the frozen foundation_http prefix handler
    /// (Q10 — no post-build mutation).
    #[must_use]
    pub fn into_handler(self) -> ConnectRpcHandler {
        ConnectRpcHandler::new(self)
    }

    /// Number of registered procedures.
    #[must_use]
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// Whether no procedure is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }

    // ── internals ──

    pub(crate) fn lookup(&self, path: &str) -> Option<&HandlerEntry> {
        self.handlers.get(path)
    }

    pub(crate) fn global_compression(&self) -> &Arc<CompressionRegistry> {
        &self.compression
    }

    fn compose_unary(&self, options: &HandlerOptions, inner: UnaryFunc) -> UnaryFunc {
        InterceptorChain::new(self.merged_interceptors(options)).wrap_unary(inner)
    }

    fn compose_streaming(
        &self,
        options: &HandlerOptions,
        inner: StreamingHandlerFunc,
    ) -> StreamingHandlerFunc {
        InterceptorChain::new(self.merged_interceptors(options)).wrap_streaming_handler(inner)
    }

    /// Global interceptors first (outermost), then per-procedure (innermost) —
    /// the chain applies them in reverse so the first listed executes first.
    fn merged_interceptors(&self, options: &HandlerOptions) -> Vec<Arc<dyn Interceptor>> {
        let mut all = self.global_options.clone();
        all.extend(options.interceptors.iter().cloned());
        all
    }

    fn build_entry(
        &self,
        procedure: &str,
        stream_type: StreamType,
        handler: HandlerKind,
        codec_meta: Arc<dyn ProcedureMeta>,
        options: HandlerOptions,
    ) -> HandlerEntry {
        HandlerEntry {
            spec: Spec {
                stream_type,
                procedure: procedure.to_string(),
                is_client: false,
                idempotency: options.idempotency,
            },
            handler,
            protocol_handlers: default_protocol_handlers(),
            codec_meta,
            options,
        }
    }
}

/// The HTTP/1.1 protocol handlers every procedure serves (Connect + gRPC-Web).
/// gRPC (HTTP/2) is added by a later feature; the router is protocol-agnostic.
fn default_protocol_handlers() -> Vec<Box<dyn ProtocolHandler>> {
    vec![
        Box::new(ConnectHandler),
        Box::new(GrpcHandler),
        Box::new(GrpcWebHandler),
    ]
}
