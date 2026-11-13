//! Client core (F24): typed `Client<Req,Res>`, `ClientOptions`, per-call `RpcCtx`,
//! and the four stream facades (Decision 07).
//!
//! WHY: Every RPC invocation flows through the Client's typed surface. The Client
//! owns the transport, protocol, codecs, and configuration, and derives a per-call
//! `RpcCtx` for every invocation (deadline merge, linked cancel, spec/peer overwritten).
//! The stream facades (`ServerStream`, `ClientStream`, `BidiStream`) wrap the
//! conn's split halves with typed encode/decode, so callers never touch raw bytes
//! or frames.
//!
//! WHAT: [`Client`], [`ClientConfig`], [`ClientOptions`], [`ProtocolSelection`],
//! and the four stream facades with their split halves.
//!
//! HOW: Unary calls use `Transport::open` + direct byte push + response collection
//! (no valtron spawning). Streaming calls use `ProtocolClient::new_conn` to build
//! the reader/writer tasks and spawn them on the valtron pool via `from_future`.

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use foundation_compact::Instant;
use foundation_context::Ctx;
use foundation_core::url::Uri;
use foundation_core::valtron;
use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::http::{
    Proto, RequestDescriptor, SimpleHeader, SimpleHeaders, SimpleMethod, SimpleUrl, Status,
};
use futures::{Stream, StreamExt};

use crate::shared::codec::{CodecFor, ProcedureCodecs};
use crate::shared::compression::{CompressionRegistry, Compressor};
use crate::shared::context::{
    CancelSignal, IdempotencyLevel, Peer, RequestContext, RpcCtx, Spec, StreamType,
};
use crate::shared::envelope::encode_grpc_timeout;
use crate::shared::error::{Code, ConnectError, ConnectResult};
use crate::shared::interceptor::{Interceptor, InterceptorChain, UnaryCall, UnaryFunc, UnaryReply};
use crate::shared::message::{Request, Response};
use crate::shared::protocol::connect::constants as connect_consts;
use crate::shared::protocol::grpc::parse_grpc_error_trailer;
use crate::shared::protocol::grpc_web::constants as grpc_web_consts;
use crate::shared::protocol::{ClientExchange, ClientExchangeConfig, ProtocolClient};
use crate::shared::transport::{
    capabilities::{check_compatible, requirements, ProtocolKind},
    conn::{ClientReceiver, ClientSender},
    frame::BoxFuture,
    Transport, TransportStream, DEFAULT_PIPE_DEPTH,
};
use crate::ConnectClient;
use crate::GrpcClient;
use crate::GrpcWebClient;

// ============================================================================
// ProtocolSelection
// ============================================================================

/// The wire protocol an RPC client call uses (Decision 07).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolSelection {
    /// The Connect protocol (default, works over HTTP/1.1 and HTTP/2).
    Connect,
    /// gRPC (requires HTTP/2).
    Grpc,
    /// gRPC-Web (works over HTTP/1.1, browser-compatible).
    GrpcWeb,
}

impl ProtocolSelection {
    /// Map to the [`ProtocolKind`] for capability matching.
    #[must_use]
    pub fn to_kind(self) -> ProtocolKind {
        match self {
            ProtocolSelection::Connect => ProtocolKind::Connect,
            ProtocolSelection::Grpc => ProtocolKind::Grpc,
            ProtocolSelection::GrpcWeb => ProtocolKind::GrpcWeb,
        }
    }

    /// The protocol name string for [`Peer.protocol`].
    fn peer_protocol_name(self) -> &'static str {
        match self {
            ProtocolSelection::Connect => "connect",
            ProtocolSelection::Grpc => "grpc",
            ProtocolSelection::GrpcWeb => "grpc-web",
        }
    }
}

// ============================================================================
// SizeLimits
// ============================================================================

/// Read and send size limits for a client call.
///
/// `read_max_bytes` defaults to 4 MiB, gRPC's default maximum receive message
/// size (grpc-go `defaultClientMaxReceiveMessageSize`), the same default the
/// server uses ([`crate::SizeLimits`]); `0` opts out.
///
/// `error_body_max_bytes` defaults to 8 KiB. It bounds the body of a non-OK
/// response that becomes an error message.
#[derive(Debug, Clone)]
pub struct SizeLimits {
    /// Maximum response message size (`0` = unlimited): a unary response body,
    /// or each streaming response envelope (checked against its length prefix
    /// before the payload is read).
    pub read_max_bytes: usize,
    /// Maximum bytes of a non-OK response body read to build an error message
    /// (`0` = read no error body). The reader stops at the cap and drops the
    /// stream, so a large or slow error body never delays the call or uses
    /// unbounded memory.
    ///
    /// Default 8 KiB: nginx's error-page buffer (`proxy_buffer_size`, one
    /// memory page), the same default `foundation_db` uses for an error body
    /// (`DEFAULT_MAX_ERROR_BODY`).
    pub error_body_max_bytes: usize,
    /// Maximum request body size (`0` = unlimited).
    pub send_max_bytes: usize,
    /// Minimum payload size for compression (`0` = always compress).
    pub compress_min_bytes: usize,
}

impl Default for SizeLimits {
    fn default() -> Self {
        Self {
            // 4 MiB — gRPC's default max receive message size (see above).
            read_max_bytes: 4 * 1024 * 1024,
            // 8 KiB — nginx's error-page buffer (see above).
            error_body_max_bytes: 8 * 1024,
            send_max_bytes: 0,
            compress_min_bytes: 0,
        }
    }
}

// ============================================================================
// ClientConfig
// ============================================================================

/// Frozen client configuration — built by [`ClientOptions`] and validated in
/// [`Client::new`] (Decision 07).
///
/// Manually implements `Debug` because [`CompressionRegistry`] does not.
#[derive(Clone)]
pub struct ClientConfig {
    /// The procedure URL base (scheme + authority + optional prefix path).
    pub url: String,
    /// The wire protocol to use.
    pub protocol: ProtocolSelection,
    /// The SEND-codec wire token, resolved against the [`ProcedureCodecs`] table
    /// at construction (default: `"proto"`).
    pub codec_name: String,
    /// Compression registry for negotiable algorithms.
    pub compression: CompressionRegistry,
    /// Compress outgoing messages with this algorithm (`None` = identity).
    pub send_compression: Option<String>,
    /// Read / send / compress-min size limits.
    pub limits: SizeLimits,
    /// Client interceptors (applied in registration order, outermost first).
    pub interceptors: Vec<Arc<dyn Interceptor>>,
    /// Per-call default timeout; merged with the caller's remaining deadline as
    /// `min(caller, default)`.
    pub default_timeout: Option<Duration>,
    /// Default client-side deadline. Used only when the caller's [`RpcCtx`] has no
    /// deadline: a `RpcCtx` deadline always wins over this value. `None` (the
    /// default) sets no deadline of its own.
    pub default_deadline: Option<Duration>,
    /// GET→POST fallback URL length threshold (default 8 KiB). `0` disables the
    /// preflight check (Decision 07 P14).
    pub get_url_max_bytes: usize,
    /// Retry a GET that was rejected with 405/415 as POST (Decision 07).
    pub get_use_fallback: bool,
    /// Preferred HTTP version for capability matching (Decision 11 T12).
    pub preferred_http_version: Option<Proto>,
    /// Seam pipe depth (frames in flight per direction).
    pub pipe_depth: usize,
    /// Extra request headers merged into every call this client makes. Unlike
    /// per-call `Request` headers, these also apply to streaming calls that take
    /// a request *stream* (client/bidi) rather than a `Request`. Multiple values
    /// per header name are preserved (e.g. repeated session grpc-method headers).
    pub extra_headers: SimpleHeaders,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            protocol: ProtocolSelection::Connect,
            codec_name: "proto".to_string(),
            compression: CompressionRegistry::new(),
            send_compression: None,
            limits: SizeLimits::default(),
            interceptors: Vec::new(),
            default_timeout: None,
            default_deadline: None,
            get_url_max_bytes: 8192,
            get_use_fallback: true,
            preferred_http_version: None,
            pipe_depth: DEFAULT_PIPE_DEPTH,
            extra_headers: SimpleHeaders::new(),
        }
    }
}

impl core::fmt::Debug for ClientConfig {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ClientConfig")
            .field("url", &self.url)
            .field("protocol", &self.protocol)
            .field("codec_name", &self.codec_name)
            .field("send_compression", &self.send_compression)
            .field("limits", &self.limits)
            .field("interceptors", &self.interceptors.len())
            .field("default_timeout", &self.default_timeout)
            .field("default_deadline", &self.default_deadline)
            .field("get_url_max_bytes", &self.get_url_max_bytes)
            .field("get_use_fallback", &self.get_use_fallback)
            .field("preferred_http_version", &self.preferred_http_version)
            .field("pipe_depth", &self.pipe_depth)
            .finish_non_exhaustive()
    }
}

// ============================================================================
// ClientOptions (Builder)
// ============================================================================

/// Builder for [`ClientConfig`] (Decision 07). Mirrors connect-go's option
/// pattern. Implements `Clone` per R19.
#[derive(Clone)]
pub struct ClientOptions {
    config: ClientConfig,
    idempotency: IdempotencyLevel,
    enable_http_get: bool,
}

impl core::fmt::Debug for ClientOptions {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ClientOptions")
            .field("config", &self.config)
            .field("idempotency", &self.idempotency)
            .field("enable_http_get", &self.enable_http_get)
            .finish()
    }
}

impl ClientOptions {
    /// Create the default builder (Connect protocol, proto codec, no compression,
    /// 8 KiB get-url max, fallback enabled, pipe depth 4).
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ClientConfig::default(),
            idempotency: IdempotencyLevel::Unknown,
            enable_http_get: false,
        }
    }

    // ── Protocol ────────────────────────────────────────────────────────────────

    /// Add an extra request header applied to **every** call (including
    /// streaming calls that take a request stream, where there is no `Request`
    /// to hang headers on). Repeated calls with the same name append additional
    /// values — used e.g. for `BuildKit`'s repeated
    /// `x-docker-expose-session-grpc-method` session headers.
    #[must_use]
    pub fn with_header(mut self, key: impl Into<SimpleHeader>, value: impl Into<String>) -> Self {
        self.config
            .extra_headers
            .entry(key.into())
            .or_default()
            .push(value.into());
        self
    }

    /// Select the Connect protocol (default).
    #[must_use]
    pub fn with_connect(mut self) -> Self {
        self.config.protocol = ProtocolSelection::Connect;
        self
    }

    /// Select gRPC (requires HTTP/2; fails at `Client::new` on h1-only transports).
    #[must_use]
    pub fn with_grpc(mut self) -> Self {
        self.config.protocol = ProtocolSelection::Grpc;
        self
    }

    /// Select gRPC-Web (browser-compatible, works over HTTP/1.1).
    #[must_use]
    pub fn with_grpc_web(mut self) -> Self {
        self.config.protocol = ProtocolSelection::GrpcWeb;
        self
    }

    // ── Codec ───────────────────────────────────────────────────────────────────

    /// Select the codec by wire name. Validated at `Client::new` against the
    /// installed [`ProcedureCodecs`] table. Unknown name → `Client::new` fails.
    #[must_use]
    pub fn with_codec(mut self, name: &str) -> Self {
        self.config.codec_name = name.to_string();
        self
    }

    // ── Compression ─────────────────────────────────────────────────────────────

    /// Enable gzip compression for outgoing messages.
    #[must_use]
    pub fn with_send_gzip(self) -> Self {
        self.with_send_compression("gzip")
    }

    /// Set the send compression algorithm by name.
    #[must_use]
    pub fn with_send_compression(mut self, name: &str) -> Self {
        self.config.send_compression = Some(name.to_string());
        self
    }

    /// Register an accept-compression algorithm.
    #[must_use]
    pub fn with_accept_compression(mut self, _name: &str, compressor: Arc<dyn Compressor>) -> Self {
        self.config.compression.register(compressor);
        self
    }

    // ── Limits ──────────────────────────────────────────────────────────────────

    /// Maximum response message size in bytes (`0` = unlimited; default 4 MiB,
    /// see [`SizeLimits`]).
    #[must_use]
    pub fn with_read_max_bytes(mut self, n: usize) -> Self {
        self.config.limits.read_max_bytes = n;
        self
    }

    /// Maximum bytes of a non-OK response body read to build an error message
    /// (`0` = read no error body; default 8 KiB, see [`SizeLimits`]). The reader
    /// stops at the cap and drops the stream, so the error returns promptly.
    #[must_use]
    pub fn with_error_body_max_bytes(mut self, n: usize) -> Self {
        self.config.limits.error_body_max_bytes = n;
        self
    }

    /// Maximum request body size in bytes (`0` = unlimited).
    #[must_use]
    pub fn with_send_max_bytes(mut self, n: usize) -> Self {
        self.config.limits.send_max_bytes = n;
        self
    }

    /// Minimum payload size in bytes before compression is applied (`0` = always).
    #[must_use]
    pub fn with_compress_min_bytes(mut self, n: usize) -> Self {
        self.config.limits.compress_min_bytes = n;
        self
    }

    // ── Interceptors ────────────────────────────────────────────────────────────

    /// Add a client interceptor.
    #[must_use]
    pub fn with_interceptor(mut self, interceptor: Arc<dyn Interceptor>) -> Self {
        self.config.interceptors.push(interceptor);
        self
    }

    // ── Timeout ─────────────────────────────────────────────────────────────────

    /// Set a default per-call timeout (merged with the caller's remaining deadline).
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.config.default_timeout = Some(timeout);
        self
    }

    /// Set a default client-side deadline for every call this client makes.
    ///
    /// The deadline is enforced by the client: a call that does not finish in
    /// time is cancelled and fails with [`Code::DeadlineExceeded`]. A deadline
    /// on the per-call [`RpcCtx`] **wins over** this default, and `default_timeout`
    /// still merges with the caller's deadline as `min`. The default is `None`
    /// (no deadline of the client's own).
    #[must_use]
    pub fn with_default_deadline(mut self, deadline: Duration) -> Self {
        self.config.default_deadline = Some(deadline);
        self
    }

    // ── Idempotency & GET ───────────────────────────────────────────────────────

    /// Set the idempotency level for this client (drives GET eligibility).
    #[must_use]
    pub fn with_idempotency(mut self, level: IdempotencyLevel) -> Self {
        self.idempotency = level;
        self
    }

    /// Enable HTTP GET for idempotent unary RPCs (requires `NoSideEffects` level).
    #[must_use]
    pub fn with_http_get(mut self) -> Self {
        self.enable_http_get = true;
        self
    }

    /// Override the GET→POST fallback URL length threshold (default 8 KiB).
    #[must_use]
    pub fn with_get_url_max_bytes(mut self, n: usize) -> Self {
        self.config.get_url_max_bytes = n;
        self
    }

    /// Enable or disable GET→POST fallback (default enabled).
    #[must_use]
    pub fn with_get_fallback(mut self, enabled: bool) -> Self {
        self.config.get_use_fallback = enabled;
        self
    }

    // ── HTTP version ────────────────────────────────────────────────────────────

    /// Set a preferred HTTP version for capability matching (Decision 11 T12).
    #[must_use]
    pub fn with_preferred_http_version(mut self, v: Proto) -> Self {
        self.config.preferred_http_version = Some(v);
        self
    }

    // ── Pipe depth ──────────────────────────────────────────────────────────────

    /// Set the seam pipe depth (frames in flight per direction).
    #[must_use]
    pub fn with_pipe_depth(mut self, n: usize) -> Self {
        self.config.pipe_depth = n;
        self
    }
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Client
// ============================================================================

/// A typed per-procedure RPC client (Decision 07).
///
/// WHY: One generic struct covers all four RPC shapes (unary, server-streaming,
/// client-streaming, bidi) with a single codec table, transport, and protocol.
/// The constructor validates codec name + protocol-vs-transport compatibility;
/// every RPC invocation derives a fresh per-call [`RpcCtx`] (deadline merge, linked
/// cancel, spec/peer overwrite).
pub struct Client<Req, Res> {
    /// The byte-level transport.
    transport: Arc<dyn Transport>,
    /// Frozen configuration.
    config: ClientConfig,
    /// Protocol-specific client (Connect, gRPC-Web).
    protocol: Box<dyn ProtocolClient>,
    /// Per-procedure typed codec table.
    codecs: ProcedureCodecs<Req, Res>,
    /// Client-level idempotency (used for GET eligibility when not overridden by
    /// the call).
    idempotency: IdempotencyLevel,
    /// Whether HTTP GET is enabled for idempotent unary RPCs.
    enable_http_get: bool,
}

impl<Req: Send + 'static, Res: Send + 'static> Client<Req, Res> {
    /// Build a new client.
    ///
    /// 1. Resolves `codec_name` against the `ProcedureCodecs` table (unknown name
    ///    → error).
    /// 2. Validates the protocol + stream-type requirements against the transport's
    ///    capabilities (e.g. gRPC requires HTTP/2).
    /// 3. Instantiates the protocol client.
    ///
    /// # Errors
    /// An [`ConnectError`]-carrying trace for an unknown codec or incompatible
    /// protocol/transport pair.
    pub fn new(
        transport: Arc<dyn Transport>,
        url: &str,
        codecs: ProcedureCodecs<Req, Res>,
        options: ClientOptions,
    ) -> ConnectResult<Self> {
        let config = options.config;

        // If an explicit URL is passed, prefer it over the one in options.
        let config = if url.is_empty() {
            config
        } else {
            ClientConfig {
                url: url.to_string(),
                ..config
            }
        };

        // 1. Validate codec name.
        codecs.for_request(&config.codec_name)?;

        // 2. Validate protocol against transport capabilities at unary minimum.
        let kind = config.protocol.to_kind();
        let reqs = requirements(kind, StreamType::Unary);
        check_compatible(&reqs, &transport.capabilities())?;

        // 3. Instantiate the protocol client.
        let protocol: Box<dyn ProtocolClient> = match config.protocol {
            ProtocolSelection::Connect => Box::new(ConnectClient),
            ProtocolSelection::Grpc => Box::new(GrpcClient),
            ProtocolSelection::GrpcWeb => Box::new(GrpcWebClient { text: false }),
        };

        let idempotency = options.idempotency;
        let enable_http_get = options.enable_http_get;

        Ok(Self {
            transport,
            config,
            protocol,
            codecs,
            idempotency,
            enable_http_get,
        })
    }

    // ════════════════════════════════════════════════════════════════════════
    // The four async RPC methods
    // ════════════════════════════════════════════════════════════════════════

    /// Unary RPC: one request message, one response message.
    ///
    /// If the client has idempotency `NoSideEffects` and `with_http_get()` was
    /// called, this method may use HTTP GET with automatic POST fallback.
    ///
    /// # Errors
    /// A trace on transport failure, codec failure, timeout, or an error response.
    pub async fn unary(&self, ctx: RpcCtx, request: Request<Req>) -> ConnectResult<Response<Res>> {
        let idempotency = self.idempotency;
        let spec = Spec {
            stream_type: StreamType::Unary,
            procedure: String::new(),
            is_client: true,
            idempotency,
        };
        let call_ctx = self.derive_call_ctx(&ctx, spec);
        let spec = call_ctx.spec().clone();

        // Build the full procedure URL.
        let url = build_url(&self.config.url, &spec.procedure);

        // Check GET eligibility.
        if self.enable_http_get && idempotency == IdempotencyLevel::NoSideEffects {
            return self
                .call_unary_get_or_fallback(call_ctx, request, url)
                .await;
        }

        self.call_unary_post(call_ctx, request, url).await
    }

    /// Server-streaming RPC: one request, stream of responses.
    ///
    /// # Errors
    /// A trace on transport failure, capability mismatch, or protocol setup failure.
    pub async fn server_stream(
        &self,
        ctx: RpcCtx,
        request: Request<Req>,
    ) -> ConnectResult<ServerStream<Res>> {
        with_deadline(&ctx, self.server_stream_inner(&ctx, request)).await
    }

    async fn server_stream_inner(
        &self,
        caller: &RpcCtx,
        request: Request<Req>,
    ) -> ConnectResult<ServerStream<Res>> {
        let spec = Spec {
            stream_type: StreamType::ServerStream,
            procedure: String::new(),
            is_client: true,
            idempotency: self.idempotency,
        };
        let call_ctx = self.derive_call_ctx(caller, spec);
        let spec = call_ctx.spec().clone();
        let peer = call_ctx.peer().clone();
        let url = build_url(&self.config.url, &spec.procedure);

        // Validate streaming capability.
        let reqs = requirements(self.config.protocol.to_kind(), StreamType::ServerStream);
        check_compatible(&reqs, &self.transport.capabilities())?;

        // Marshal the request. The protocol writer compresses and flags the
        // frame; sending compressed bytes here would mark them as plain.
        let request_codec = self.codecs.for_request(&self.config.codec_name)?;
        let body = request_codec.marshal(&request.msg)?;

        // Build headers. The header names the configured algorithm, so the peer
        // negotiates the decompressor for the frames the writer sends.
        let mut headers = request.headers().clone();
        self.protocol.write_request_headers(
            StreamType::ServerStream,
            &mut headers,
            &self.config.codec_name,
            self.config.send_compression.as_deref(),
        );
        add_timeout_header(
            self.config.protocol,
            &mut headers,
            call_ctx.remaining_timeout(),
        );
        // Merge client-wide extra headers (applies to streaming calls that take a
        // request stream rather than a `Request`, e.g. BuildKit session headers).
        for (key, values) in &self.config.extra_headers {
            for value in values {
                headers.entry(key.clone()).or_default().push(value.clone());
            }
        }

        // Build transport request.
        let desc = build_request_descriptor(&url, &headers, SimpleMethod::POST)?;
        let transport_stream = self.transport.open(desc).map_err(ConnectError::from)?;

        // Build the ClientExchange with linked cancel.
        let exchange =
            self.build_streaming_exchange(&spec, headers, transport_stream, &call_ctx)?;

        // Spawn reader/writer tasks.
        spawn_reader_writer(
            exchange.reader_task,
            exchange.writer_task,
            call_ctx.cancel_signal(),
        )?;

        let (mut sender, mut receiver) = exchange.conn.split();

        // Send the request frame and close.
        sender.send(body).await?;
        sender.close_send().await?;

        // Await the response head so the caller's `response_headers()` is sync-OK.
        let response_headers = receiver.response_headers().await?.clone();
        let response_codec = self.codecs.for_response(&self.config.codec_name)?;

        Ok(ServerStream {
            recv: receiver,
            codec: response_codec,
            spec,
            peer,
            response_headers,
            trailers: SimpleHeaders::new(),
            got_trailers: false,
            deadline: call_ctx.request.deadline,
            ctx: call_ctx.ctx.clone(),
            cancel: CancelGuard::new(call_ctx.cancel_signal().clone()),
        })
    }

    /// Client-streaming RPC: stream of requests, one response.
    ///
    /// # Errors
    /// A trace on transport failure, capability mismatch, or protocol setup failure.
    pub async fn client_stream(
        &self,
        ctx: RpcCtx,
        reqs: impl Stream<Item = Req>,
    ) -> ConnectResult<Response<Res>> {
        with_deadline(&ctx, self.client_stream_inner(&ctx, reqs)).await
    }

    async fn client_stream_inner(
        &self,
        caller: &RpcCtx,
        reqs: impl Stream<Item = Req>,
    ) -> ConnectResult<Response<Res>> {
        let spec = Spec {
            stream_type: StreamType::ClientStream,
            procedure: String::new(),
            is_client: true,
            idempotency: self.idempotency,
        };
        let call_ctx = self.derive_call_ctx(caller, spec);
        let spec = call_ctx.spec().clone();
        let url = build_url(&self.config.url, &spec.procedure);

        // Validate streaming capability.
        let reqs_needed = requirements(self.config.protocol.to_kind(), StreamType::ClientStream);
        check_compatible(&reqs_needed, &self.transport.capabilities())?;

        // Build headers.
        let mut headers = SimpleHeaders::new();
        self.protocol.write_request_headers(
            StreamType::ClientStream,
            &mut headers,
            &self.config.codec_name,
            self.config.send_compression.as_deref(),
        );
        add_timeout_header(
            self.config.protocol,
            &mut headers,
            call_ctx.remaining_timeout(),
        );
        // Merge client-wide extra headers (applies to streaming calls that take a
        // request stream rather than a `Request`, e.g. BuildKit session headers).
        for (key, values) in &self.config.extra_headers {
            for value in values {
                headers.entry(key.clone()).or_default().push(value.clone());
            }
        }

        let desc = build_request_descriptor(&url, &headers, SimpleMethod::POST)?;
        let transport_stream = self.transport.open(desc).map_err(ConnectError::from)?;

        let exchange =
            self.build_streaming_exchange(&spec, headers, transport_stream, &call_ctx)?;
        spawn_reader_writer(
            exchange.reader_task,
            exchange.writer_task,
            call_ctx.cancel_signal(),
        )?;

        let (mut sender, mut receiver) = exchange.conn.split();
        let request_codec = self.codecs.for_request(&self.config.codec_name)?;
        let response_codec = self.codecs.for_response(&self.config.codec_name)?;

        // Send all request messages.
        let mut pinned_reqs = Box::pin(reqs);
        while let Some(msg) = pinned_reqs.next().await {
            let frame = request_codec.marshal(&msg)?;
            sender.send(frame).await?;
        }
        sender.close_send().await?;

        // Receive the single response.
        let frame = receiver
            .receive()
            .await?
            .ok_or_else(|| ConnectError::internal("client stream: expected a response frame"))?;
        let decoded = response_codec.unmarshal(frame)?;

        let trailers = receiver.response_trailers().await?.clone();
        let resp_headers = receiver.response_headers().await?.clone();

        let mut response = Response::new(decoded);
        for (h, vals) in resp_headers {
            response.headers_mut().insert(h, vals);
        }
        for (h, vals) in trailers {
            response.trailers_mut().insert(h, vals);
        }

        Ok(response)
    }

    /// Bidirectional streaming RPC.
    ///
    /// The returned [`BidiStream`] is the only writer: send with
    /// [`BidiStream::send`] (or [`BidiStream::split`] and use the sender half).
    ///
    /// # Errors
    /// A trace on transport failure, capability mismatch, or protocol setup failure.
    pub async fn bidi_stream(&self, ctx: RpcCtx) -> ConnectResult<BidiStream<Req, Res>> {
        // The async block keeps the setup lazy, so a past deadline fails before
        // `bidi_stream_inner` opens the transport.
        with_deadline(&ctx, async { self.bidi_stream_inner(&ctx) }).await
    }

    fn bidi_stream_inner(&self, caller: &RpcCtx) -> ConnectResult<BidiStream<Req, Res>> {
        let spec = Spec {
            stream_type: StreamType::BidiStream,
            procedure: String::new(),
            is_client: true,
            idempotency: self.idempotency,
        };
        let call_ctx = self.derive_call_ctx(caller, spec);
        let spec = call_ctx.spec().clone();
        let peer = call_ctx.peer().clone();
        let url = build_url(&self.config.url, &spec.procedure);

        // Bidi requires full duplex.
        let reqs_needed = requirements(self.config.protocol.to_kind(), StreamType::BidiStream);
        check_compatible(&reqs_needed, &self.transport.capabilities())?;

        // Build headers.
        let mut headers = SimpleHeaders::new();
        self.protocol.write_request_headers(
            StreamType::BidiStream,
            &mut headers,
            &self.config.codec_name,
            self.config.send_compression.as_deref(),
        );
        add_timeout_header(
            self.config.protocol,
            &mut headers,
            call_ctx.remaining_timeout(),
        );
        // Merge client-wide extra headers (applies to streaming calls that take a
        // request stream rather than a `Request`, e.g. BuildKit session headers).
        for (key, values) in &self.config.extra_headers {
            for value in values {
                headers.entry(key.clone()).or_default().push(value.clone());
            }
        }

        let desc = build_request_descriptor(&url, &headers, SimpleMethod::POST)?;
        let transport_stream = self.transport.open(desc).map_err(ConnectError::from)?;

        let exchange =
            self.build_streaming_exchange(&spec, headers, transport_stream, &call_ctx)?;
        spawn_reader_writer(
            exchange.reader_task,
            exchange.writer_task,
            call_ctx.cancel_signal(),
        )?;

        let (sender, receiver) = exchange.conn.split();
        let request_codec = self.codecs.for_request(&self.config.codec_name)?;
        let response_codec = self.codecs.for_response(&self.config.codec_name)?;

        Ok(BidiStream {
            send: sender,
            recv: receiver,
            request_codec,
            response_codec,
            spec,
            peer,
            deadline: call_ctx.request.deadline,
            ctx: call_ctx.ctx.clone(),
            cancel: CancelGuard::new(call_ctx.cancel_signal().clone()),
        })
    }

    // ════════════════════════════════════════════════════════════════════════
    // Internal helpers
    // ════════════════════════════════════════════════════════════════════════

    /// Derive a per-call [`RpcCtx`] from the caller's context (Decision 07 per-call
    /// `RpcCtx` contract table).
    fn derive_call_ctx(&self, caller: &RpcCtx, spec: Spec) -> RpcCtx {
        let now = caller.ctx.clock().now();
        let deadline = match (
            caller.request.deadline,
            self.config.default_timeout,
            self.config.default_deadline,
        ) {
            // A caller deadline wins over the client's default deadline. The
            // existing `default_timeout` still merges with it as `min`.
            (Some(caller_deadline), Some(default), _) => Some(caller_deadline.min(now + default)),
            (Some(caller_deadline), None, _) => Some(caller_deadline),
            // No caller deadline: use the earlier of the two client defaults.
            (None, Some(timeout), Some(default_deadline)) => {
                Some((now + timeout).min(now + default_deadline))
            }
            (None, Some(timeout), None) => Some(now + timeout),
            (None, None, Some(default_deadline)) => Some(now + default_deadline),
            (None, None, None) => None,
        };

        let peer = Peer {
            addr: self.config.url.clone(),
            protocol: self.config.protocol.peer_protocol_name().to_string(),
        };

        RpcCtx {
            ctx: caller.ctx.clone(),
            request: RequestContext::for_dispatch(
                spec,
                peer,
                SimpleHeaders::new(),
                deadline,
                caller.request.extensions.clone(),
                caller.request.connection.clone(),
                CancelSignal::linked(caller.cancel_signal()),
                caller.ctx.clone(),
            ),
        }
    }

    /// Build a [`ClientExchange`] with the cancel signal linked to the call ctx.
    fn build_streaming_exchange(
        &self,
        spec: &Spec,
        headers: SimpleHeaders,
        stream: TransportStream,
        ctx: &RpcCtx,
    ) -> ConnectResult<ClientExchange> {
        // Resolve the outgoing compressor once per call. `None` and `identity`
        // mean that no frame is compressed. An unknown name is an internal
        // configuration error, the same one the unary path reports.
        let send_compressor = match self.config.send_compression.as_deref() {
            None | Some("identity") => None,
            Some(name) => Some(self.config.compression.get(name).cloned().ok_or_else(|| {
                ConnectError::internal(format!("unknown compression algorithm: {name}"))
            })?),
        };
        self.protocol.new_conn(
            spec,
            headers,
            stream,
            ClientExchangeConfig {
                cancel: CancelSignal::linked(ctx.cancel_signal()),
                read_max_bytes: self.config.limits.read_max_bytes,
                compression: &self.config.compression,
                send_compressor,
                compress_min_bytes: self.config.limits.compress_min_bytes,
                send_max_bytes: self.config.limits.send_max_bytes,
            },
        )
    }

    // ── POST unary ─────────────────────────────────────────────────────────

    /// POST-based unary call implementation.
    async fn call_unary_post(
        &self,
        ctx: RpcCtx,
        request: Request<Req>,
        url: String,
    ) -> ConnectResult<Response<Res>> {
        with_deadline(&ctx, self.call_unary_post_inner(&ctx, request, url)).await
    }

    /// Prepare a marshaled unary request body for the wire.
    ///
    /// Applies the same limits as the streaming envelope writer:
    /// `compress_min_bytes` decides whether to compress at all, and
    /// `send_max_bytes` caps the payload after compression. Without these
    /// checks a unary call ignored both limits.
    fn prepare_unary_body(&self, encoded: &Bytes) -> ConnectResult<(Vec<u8>, Option<String>)> {
        let send_compression = self.config.send_compression.as_deref();
        let want_compress = matches!(send_compression, Some(algo) if algo != "identity")
            && (self.config.limits.compress_min_bytes == 0
                || encoded.len() >= self.config.limits.compress_min_bytes);
        let (body, content_encoding) = if want_compress {
            compress_if(&self.config.compression, send_compression, encoded)?
        } else {
            (encoded.to_vec(), None)
        };
        let send_max_bytes = self.config.limits.send_max_bytes;
        if send_max_bytes != 0 && body.len() > send_max_bytes {
            return Err(ConnectError::resource_exhausted(format!(
                "message of {} bytes exceeds send_max_bytes of {send_max_bytes}",
                body.len()
            ))
            .into());
        }
        Ok((body, content_encoding))
    }

    async fn call_unary_post_inner(
        &self,
        ctx: &RpcCtx,
        request: Request<Req>,
        url: String,
    ) -> ConnectResult<Response<Res>> {
        // Marshal the request, then apply the send limits.
        let request_codec = self.codecs.for_request(&self.config.codec_name)?;
        let encoded = request_codec.marshal(&request.msg)?;
        let (body, content_encoding) = self.prepare_unary_body(&encoded)?;

        // Build headers: caller's request headers + protocol headers.
        let mut headers = request.headers().clone();
        self.protocol.write_request_headers(
            StreamType::Unary,
            &mut headers,
            &self.config.codec_name,
            content_encoding.as_deref(),
        );
        add_timeout_header(self.config.protocol, &mut headers, ctx.remaining_timeout());
        // Merge client-wide extra headers — same contract as the streaming
        // paths (e.g. BuildKit's `buildkit-controlapi-buildid` gateway routing
        // header rides every unary call of a gateway client).
        for (key, values) in &self.config.extra_headers {
            for value in values {
                headers.entry(key.clone()).or_default().push(value.clone());
            }
        }

        // Protocol-specific wire framing. Connect = bare bytes; gRPC/gRPC-Web =
        // single envelope frame. The compression flag (if any) is already in
        // `content_encoding`; the protocol sets the corresponding wire flag.
        let wire_body = self
            .protocol
            .encode_unary_request(&body, content_encoding.is_some());

        // gRPC requires HTTP/2; Connect and gRPC-Web are HTTP/1.1.
        let proto = match self.config.protocol {
            ProtocolSelection::Grpc => Proto::HTTP20,
            _ => Proto::HTTP11,
        };

        let call = UnaryCall {
            headers: headers.clone(),
            codec_name: self.config.codec_name.clone(),
            frame: wire_body,
        };

        // Build the inner unary function.
        let transport = Arc::clone(&self.transport);
        let read_max_bytes = self.config.limits.read_max_bytes;
        let error_body_max_bytes = self.config.limits.error_body_max_bytes;
        let url_clone = url.clone();
        // Proto is Clone but not Copy; clone it into each closure scope.
        let proto_for_inner = proto.clone();

        let inner: UnaryFunc = Arc::new(move |_ctx: RpcCtx, call: UnaryCall| {
            let transport = Arc::clone(&transport);
            let url = url_clone.clone();
            let proto = proto_for_inner.clone();
            Box::pin(async move {
                let uri = Uri::parse(&url)
                    .map_err(|e| ConnectError::internal(format!("invalid URL: {e}")))?;
                let desc = RequestDescriptor {
                    proto,
                    request_url: SimpleUrl::url_only(&url),
                    request_uri: uri,
                    headers: call.headers,
                    method: SimpleMethod::POST,
                };

                // Execute via transport.open + push + close + read.
                do_unary_round_trip(
                    &*transport,
                    desc,
                    call.frame,
                    read_max_bytes,
                    error_body_max_bytes,
                )
                .await
            })
        });

        let reply = self.run_unary_interceptors(ctx, inner, call).await?;

        // Decode response: protocol-level unwrap first, then codec unmarshal.
        // gRPC/gRPC-Web envelop their unary response; Connect returns bare bytes.
        let wire = self.protocol.decode_unary_response(
            &reply.headers,
            reply.frame,
            &self.config.compression,
            self.config.limits.read_max_bytes,
        )?;
        let response_codec = self.codecs.for_response(&self.config.codec_name)?;
        let msg = response_codec.unmarshal(wire)?;

        let mut response = Response::new(msg);
        for (h, vals) in &reply.headers {
            response.headers_mut().insert(h.clone(), vals.clone());
        }
        for (h, vals) in &reply.trailers {
            response.trailers_mut().insert(h.clone(), vals.clone());
        }

        Ok(response)
    }

    /// Run the unary call through the interceptor chain, then apply every
    /// protocol check that must run before the body is decoded: the gRPC-Web
    /// trailer lift, the gRPC status mapping, and the empty-message rule.
    async fn run_unary_interceptors(
        &self,
        ctx: &RpcCtx,
        inner: UnaryFunc,
        call: UnaryCall,
    ) -> ConnectResult<UnaryReply> {
        // Apply interceptor chain.
        let chain = InterceptorChain::new(self.config.interceptors.clone());
        let wrapped = chain.wrap_unary(inner);
        let mut reply = wrapped(ctx.clone(), call).await?;

        // gRPC-Web carries its trailers (grpc-status/grpc-message) in an
        // in-body `0x80` trailer frame after the message, or alone for an
        // error. Lift them into `reply.trailers` so the status check below
        // sees them, and keep only the message envelope as the frame —
        // otherwise an error's trailer frame is decoded as the message.
        if matches!(self.config.protocol, ProtocolSelection::GrpcWeb) {
            let (message, trailers) = split_grpc_web_unary(&reply.frame)?;
            for (name, values) in trailers {
                reply.trailers.insert(name, values);
            }
            reply.frame = message;
        }

        // Check gRPC error metadata before decoding. Errors ride grpc-status in
        // the trailing HEADERS — or, for Trailers-Only responses (errors raised
        // before any message), in the response headers themselves.
        if let Some(grpc_err) = parse_grpc_error_trailer(&reply.trailers)
            .or_else(|| parse_grpc_error_trailer(&reply.headers))
        {
            return Err(grpc_err);
        }

        // gRPC unary success MUST carry exactly one message. An empty body with
        // grpc-status 0 (or missing) would otherwise silently decode into
        // `Default::default()` — turn it into the protocol error it is.
        if reply.frame.is_empty()
            && matches!(
                self.config.protocol,
                ProtocolSelection::Grpc | ProtocolSelection::GrpcWeb
            )
        {
            return Err(ConnectError::internal(
                "gRPC unary response carried no message and no grpc-status",
            )
            .into());
        }

        Ok(reply)
    }

    // ── HTTP GET + fallback ───────────────────────────────────────────────

    /// HTTP GET for idempotent unary, with automatic POST fallback (Decision 07).
    ///
    /// Builds a GET URL with the message in the query string, executes it, and
    /// falls back to POST in the two deterministic cases:
    /// 1. URL too long → pre-flight fallback (never sent).
    /// 2. Server rejected with 405/415 → safe single retry as POST (rejection
    ///    precedes execution; no double-send risk).
    ///
    /// Transport errors are NOT retried (the GET may have executed).
    async fn call_unary_get_or_fallback(
        &self,
        ctx: RpcCtx,
        request: Request<Req>,
        url: String,
    ) -> ConnectResult<Response<Res>> {
        // Encode the message into a GET URL.
        let request_url = self.build_get_url(&url, &request)?;

        // Case 1: Pre-flight URL length check.
        if self.config.get_url_max_bytes > 0 && request_url.len() > self.config.get_url_max_bytes {
            return self.call_unary_post(ctx, request, url).await;
        }

        // Build and issue the GET.
        let uri = Uri::parse(&request_url)
            .map_err(|e| ConnectError::internal(format!("invalid GET URL: {e}")))?;
        let desc = RequestDescriptor {
            proto: Proto::HTTP11,
            request_url: SimpleUrl::url_only(&request_url),
            request_uri: uri,
            headers: request.headers().clone(),
            method: SimpleMethod::GET,
        };

        let result = with_deadline(
            &ctx,
            do_unary_round_trip(
                &*self.transport,
                desc,
                Bytes::new(),
                self.config.limits.read_max_bytes,
                self.config.limits.error_body_max_bytes,
            ),
        )
        .await;

        match result {
            Ok(reply) => {
                let response_codec = self.codecs.for_response(&self.config.codec_name)?;
                let msg = response_codec.unmarshal(reply.frame)?;
                let mut response = Response::new(msg);
                for (h, vals) in &reply.headers {
                    response.headers_mut().insert(h.clone(), vals.clone());
                }
                for (h, vals) in &reply.trailers {
                    response.trailers_mut().insert(h.clone(), vals.clone());
                }
                Ok(response)
            }
            Err(e) => {
                // Case 2: 405/415 → safe single retry as POST.
                if self.config.get_use_fallback && is_method_not_allowed(&e) {
                    self.call_unary_post(ctx, request, url).await
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Build a GET URL with the message encoded in the query string (Decision 07).
    fn build_get_url(&self, base_url: &str, request: &Request<Req>) -> ConnectResult<String> {
        use crate::shared::protocol::connect::encode_get_query;

        let procedure = if request.spec().procedure.is_empty() {
            "Method"
        } else {
            &request.spec().procedure
        };

        let request_codec = self.codecs.for_request(&self.config.codec_name)?;
        let stable = request_codec.marshal_stable(&request.msg)?;

        let compressor = self
            .config
            .send_compression
            .as_ref()
            .and_then(|name| self.config.compression.get(name).cloned());

        let query = encode_get_query(
            &self.config.codec_name,
            &stable,
            request_codec.is_binary(),
            compressor.as_ref(),
        )?;

        Ok(format!("{base_url}/{procedure}?{query}"))
    }
}

// ============================================================================
// CancelGuard
// ============================================================================

/// Keeps a streaming call alive while any of its facades is alive.
///
/// WHY: the call's [`CancelSignal`] is shared by every clone, so one `cancel()`
/// stops the reader and writer tasks of the whole call. A caller can split a
/// bidi stream, or keep more than one facade of the same call. Dropping one
/// value must not cancel the call under a live sibling.
///
/// WHAT: a cloneable handle over the call's signal and a shared count of live
/// facades.
///
/// HOW: [`Clone`] adds one to the count and [`Drop`] subtracts one. The drop
/// that reaches zero fires the signal. Firing is idempotent, takes no lock on
/// an already-fired signal and allocates nothing, so a drop during unwinding is
/// safe. Every facade holds exactly one guard, so the count is the number of
/// live facade values.
struct CancelGuard {
    /// The call's cancel signal. Firing it stops the call's reader and writer
    /// tasks and closes the transport stream.
    signal: CancelSignal,
    /// Number of live facades that hold this call. The path is not shared with
    /// any other call, because each call makes its own guard.
    live: Arc<AtomicUsize>,
}

impl CancelGuard {
    /// Start the count at one: the caller makes a guard for one facade.
    fn new(signal: CancelSignal) -> Self {
        Self {
            signal,
            live: Arc::new(AtomicUsize::new(1)),
        }
    }

    /// The call's cancel signal, for a wait that must fire it on a timeout.
    fn signal(&self) -> &CancelSignal {
        &self.signal
    }
}

impl Clone for CancelGuard {
    fn clone(&self) -> Self {
        self.live.fetch_add(1, Ordering::Relaxed);
        Self {
            signal: self.signal.clone(),
            live: Arc::clone(&self.live),
        }
    }
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        // `Relaxed` on the add is enough: the count only guards the fire, and
        // `AcqRel` on the subtract makes the last drop see every earlier one.
        if self.live.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.signal.cancel();
        }
    }
}

// ============================================================================
// ServerStream<Res>
// ============================================================================

/// Client's view of a server-streaming RPC (one request, many responses).
/// Holds only the receiver half — the sender was consumed when the request was
/// sent and closed (Decision 07).
pub struct ServerStream<Res> {
    recv: Box<dyn ClientReceiver>,
    codec: Arc<dyn CodecFor<Res>>,
    spec: Spec,
    peer: Peer,
    response_headers: SimpleHeaders,
    trailers: SimpleHeaders,
    got_trailers: bool,
    /// Absolute call deadline, copied from the call `RpcCtx`. Every later wait
    /// fails once it passes.
    deadline: Option<Instant>,
    /// The context whose clock measures the remaining timeout.
    ctx: Ctx,
    /// Call lifetime guard. It fires the call's cancel signal when this value
    /// (the last live facade of the call) drops, which stops the reader task
    /// and closes the transport stream.
    cancel: CancelGuard,
}

impl<Res: Send + 'static> ServerStream<Res> {
    /// Receive the next decoded response message. Returns `Ok(None)` at end of
    /// stream (clean EOF). A terminal error returns `Err`.
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC fails or the call deadline passes.
    pub async fn receive(&mut self) -> ConnectResult<Option<Res>> {
        with_deadline_at(
            self.deadline,
            Some(self.cancel.signal()),
            self.ctx.clone(),
            async {
                match self.recv.receive().await? {
                    Some(frame) => Ok(Some(self.codec.unmarshal(frame)?)),
                    None => Ok(None),
                }
            },
        )
        .await
    }

    /// Response headers (available immediately after the call is opened).
    #[must_use]
    pub fn response_headers(&self) -> &SimpleHeaders {
        &self.response_headers
    }

    /// Response trailers (available after end of stream).
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC fails or the call deadline passes.
    pub async fn response_trailers(&mut self) -> ConnectResult<&SimpleHeaders> {
        if !self.got_trailers {
            let deadline = self.deadline;
            self.trailers = with_deadline_at(
                deadline,
                Some(self.cancel.signal()),
                self.ctx.clone(),
                self.recv.response_trailers(),
            )
            .await?
            .clone();
            self.got_trailers = true;
        }
        Ok(&self.trailers)
    }

    /// The procedure spec for this call.
    #[must_use]
    pub fn spec(&self) -> &Spec {
        &self.spec
    }

    /// The remote peer for this call.
    #[must_use]
    pub fn peer(&self) -> &Peer {
        &self.peer
    }

    /// Non-blocking close of the receive side (connection reuse).
    pub fn close(self) {
        // Drop the receiver half; the transport handles cleanup.
    }
}

// ============================================================================
// ClientStream<Req, Res>
// ============================================================================

/// Client's view of a client-streaming RPC (many requests, one response).
pub struct ClientStream<Req, Res> {
    send: Box<dyn ClientSender>,
    recv: Box<dyn ClientReceiver>,
    request_codec: Arc<dyn CodecFor<Req>>,
    response_codec: Arc<dyn CodecFor<Res>>,
}

impl<Req: Send + 'static, Res: Send + 'static> ClientStream<Req, Res> {
    /// Mutable request headers (valid before the first send).
    pub fn request_headers_mut(&mut self) -> &mut SimpleHeaders {
        self.send.request_headers_mut()
    }

    /// Encode and send one request message.
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC fails.
    pub async fn send(&mut self, msg: &Req) -> ConnectResult<()> {
        let frame = self.request_codec.marshal(msg)?;
        self.send.send(frame).await
    }

    /// Half-close the request and await the single response.
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC fails.
    pub async fn close_and_receive(self) -> ConnectResult<Response<Res>> {
        // Close send half.
        let send = Box::new(self.send);
        send.close_send().await?;

        // Receive the response.
        let mut recv = self.recv;
        match recv.receive().await? {
            Some(frame) => {
                let msg = self.response_codec.unmarshal(frame)?;
                let mut response = Response::new(msg);
                let hdrs = recv.response_headers().await?.clone();
                for (h, vals) in hdrs {
                    response.headers_mut().insert(h, vals);
                }
                let trls = recv.response_trailers().await?.clone();
                for (h, vals) in trls {
                    response.trailers_mut().insert(h, vals);
                }
                Ok(response)
            }
            None => Err(ConnectError::internal("client stream: no response").into()),
        }
    }
}

// ============================================================================
// BidiStream<Req, Res>
// ============================================================================

/// Client's view of a bidirectional streaming RPC.
pub struct BidiStream<Req, Res> {
    send: Box<dyn ClientSender>,
    recv: Box<dyn ClientReceiver>,
    request_codec: Arc<dyn CodecFor<Req>>,
    response_codec: Arc<dyn CodecFor<Res>>,
    spec: Spec,
    peer: Peer,
    /// Absolute call deadline, copied from the call `RpcCtx`. Every later wait
    /// fails once it passes.
    deadline: Option<Instant>,
    /// The context whose clock measures the remaining timeout.
    ctx: Ctx,
    /// Call lifetime guard, shared by both halves after [`Self::split`]. It
    /// fires the call's cancel signal when the last live facade drops.
    cancel: CancelGuard,
}

impl<Req: Send + 'static, Res: Send + 'static> BidiStream<Req, Res> {
    /// Mutable request headers (valid before the first send).
    pub fn request_headers_mut(&mut self) -> &mut SimpleHeaders {
        self.send.request_headers_mut()
    }

    /// Encode and send one request message.
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC fails or the call deadline passes.
    pub async fn send(&mut self, msg: &Req) -> ConnectResult<()> {
        with_deadline_at(
            self.deadline,
            Some(self.cancel.signal()),
            self.ctx.clone(),
            async {
                let frame = self.request_codec.marshal(msg)?;
                self.send.send(frame).await
            },
        )
        .await
    }

    /// Header-only send (no body) — flush request headers.
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC fails or the call deadline passes.
    pub async fn send_headers(&mut self) -> ConnectResult<()> {
        let deadline = self.deadline;
        with_deadline_at(
            deadline,
            Some(self.cancel.signal()),
            self.ctx.clone(),
            self.send.flush_headers(),
        )
        .await
    }

    /// Half-close the request direction (receiver stays live).
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC fails or the call deadline passes.
    pub async fn close_request(&mut self) -> ConnectResult<()> {
        let deadline = self.deadline;
        let old = std::mem::replace(&mut self.send, Box::new(NoopClientSender));
        with_deadline_at(
            deadline,
            Some(self.cancel.signal()),
            self.ctx.clone(),
            old.close_send(),
        )
        .await
    }

    /// Receive the next decoded response message.
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC fails or the call deadline passes.
    pub async fn receive(&mut self) -> ConnectResult<Option<Res>> {
        with_deadline_at(
            self.deadline,
            Some(self.cancel.signal()),
            self.ctx.clone(),
            async {
                match self.recv.receive().await? {
                    Some(frame) => Ok(Some(self.response_codec.unmarshal(frame)?)),
                    None => Ok(None),
                }
            },
        )
        .await
    }

    /// Response headers (valid once the server starts sending).
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC fails or the call deadline passes.
    pub async fn response_headers(&mut self) -> ConnectResult<&SimpleHeaders> {
        let deadline = self.deadline;
        with_deadline_at(
            deadline,
            Some(self.cancel.signal()),
            self.ctx.clone(),
            self.recv.response_headers(),
        )
        .await
    }

    /// Response trailers (available after end of stream).
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC fails or the call deadline passes.
    pub async fn response_trailers(&mut self) -> ConnectResult<&SimpleHeaders> {
        let deadline = self.deadline;
        with_deadline_at(
            deadline,
            Some(self.cancel.signal()),
            self.ctx.clone(),
            self.recv.response_trailers(),
        )
        .await
    }

    /// Split into independently-owned typed halves for concurrent send/receive.
    #[must_use]
    pub fn split(self) -> (BidiSender<Req>, BidiReceiver<Res>) {
        // Both halves hold the same guard, so the call stays alive until the
        // last of them drops. One clone adds a live handle; the original moves
        // into the other half.
        let sender = BidiSender {
            send: self.send,
            codec: self.request_codec,
            deadline: self.deadline,
            ctx: self.ctx.clone(),
            cancel: self.cancel.clone(),
        };
        let receiver = BidiReceiver {
            recv: self.recv,
            codec: self.response_codec,
            spec: self.spec,
            peer: self.peer,
            deadline: self.deadline,
            ctx: self.ctx.clone(),
            cancel: self.cancel,
        };
        (sender, receiver)
    }

    /// The procedure spec for this call.
    #[must_use]
    pub fn spec(&self) -> &Spec {
        &self.spec
    }

    /// The remote peer for this call.
    #[must_use]
    pub fn peer(&self) -> &Peer {
        &self.peer
    }
}

// ============================================================================
// BidiSender / BidiReceiver
// ============================================================================

/// Typed send half of a bidi stream — for concurrent send/receive.
pub struct BidiSender<Req> {
    send: Box<dyn ClientSender>,
    codec: Arc<dyn CodecFor<Req>>,
    /// Absolute call deadline, copied from the call `RpcCtx`.
    deadline: Option<Instant>,
    /// The context whose clock measures the remaining timeout.
    ctx: Ctx,
    /// Call lifetime guard from the split stream.
    cancel: CancelGuard,
}

impl<Req: Send + 'static> BidiSender<Req> {
    /// Encode and send one request message.
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC fails or the call deadline passes.
    pub async fn send(&mut self, msg: &Req) -> ConnectResult<()> {
        with_deadline_at(
            self.deadline,
            Some(self.cancel.signal()),
            self.ctx.clone(),
            async {
                let frame = self.codec.marshal(msg)?;
                self.send.send(frame).await
            },
        )
        .await
    }

    /// Header-only send (flush headers).
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC fails or the call deadline passes.
    pub async fn send_headers(&mut self) -> ConnectResult<()> {
        let deadline = self.deadline;
        with_deadline_at(
            deadline,
            Some(self.cancel.signal()),
            self.ctx.clone(),
            self.send.flush_headers(),
        )
        .await
    }

    /// Half-close the request direction.
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC fails or the call deadline passes.
    pub async fn close_request(self) -> ConnectResult<()> {
        let deadline = self.deadline;
        with_deadline_at(
            deadline,
            Some(self.cancel.signal()),
            self.ctx.clone(),
            Box::new(self.send).close_send(),
        )
        .await
    }
}

/// Typed receive half of a bidi stream — for concurrent send/receive.
pub struct BidiReceiver<Res> {
    recv: Box<dyn ClientReceiver>,
    codec: Arc<dyn CodecFor<Res>>,
    spec: Spec,
    peer: Peer,
    /// Absolute call deadline, copied from the call `RpcCtx`.
    deadline: Option<Instant>,
    /// The context whose clock measures the remaining timeout.
    ctx: Ctx,
    /// Call lifetime guard from the split stream.
    cancel: CancelGuard,
}

impl<Res: Send + 'static> BidiReceiver<Res> {
    /// Receive the next decoded response message.
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC fails or the call deadline passes.
    pub async fn receive(&mut self) -> ConnectResult<Option<Res>> {
        with_deadline_at(
            self.deadline,
            Some(self.cancel.signal()),
            self.ctx.clone(),
            async {
                match self.recv.receive().await? {
                    Some(frame) => Ok(Some(self.codec.unmarshal(frame)?)),
                    None => Ok(None),
                }
            },
        )
        .await
    }

    /// Response headers.
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC fails or the call deadline passes.
    pub async fn response_headers(&mut self) -> ConnectResult<&SimpleHeaders> {
        let deadline = self.deadline;
        with_deadline_at(
            deadline,
            Some(self.cancel.signal()),
            self.ctx.clone(),
            self.recv.response_headers(),
        )
        .await
    }

    /// Response trailers.
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC fails or the call deadline passes.
    pub async fn response_trailers(&mut self) -> ConnectResult<&SimpleHeaders> {
        let deadline = self.deadline;
        with_deadline_at(
            deadline,
            Some(self.cancel.signal()),
            self.ctx.clone(),
            self.recv.response_trailers(),
        )
        .await
    }

    /// The procedure spec for this call.
    #[must_use]
    pub fn spec(&self) -> &Spec {
        &self.spec
    }

    /// The remote peer for this call.
    #[must_use]
    pub fn peer(&self) -> &Peer {
        &self.peer
    }
}

// ============================================================================
// NoopClientSender (placeholder for close_request)
// ============================================================================

struct NoopClientSender;

impl ClientSender for NoopClientSender {
    fn request_headers_mut(&mut self) -> &mut SimpleHeaders {
        unreachable!("NoopClientSender has no headers")
    }
    fn flush_headers(&mut self) -> BoxFuture<'_, ConnectResult<()>> {
        Box::pin(async { Ok(()) })
    }
    fn send(&mut self, _frame: Bytes) -> BoxFuture<'_, ConnectResult<()>> {
        Box::pin(async { Ok(()) })
    }
    fn close_send(self: Box<Self>) -> BoxFuture<'static, ConnectResult<()>> {
        Box::pin(async { Ok(()) })
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Run `call` until it finishes or `deadline` passes.
///
/// WHY: `RpcCtx::with_deadline` only records a timestamp. A wait that parks on a
/// read must fail on its own deadline, not on the peer's idle timeout.
///
/// WHAT: when `deadline` is `Some`, race `call` against a valtron sleep for the
/// time left. If the sleep wins, optionally fire `cancel`, drop the call (and
/// the response stream it owns) and return `DeadlineExceeded`. If the deadline
/// already passed, fail before the call is polled, so it sends no request and
/// reads no response.
///
/// HOW: `foundation_core::valtron::sleep_async` is the only timer, so the same
/// code runs on native and on wasm (no thread and no OS timer). Call setup and
/// every later facade wait use this helper, so a new transport or facade
/// cannot forget the bound.
async fn with_deadline_at<T>(
    deadline: Option<Instant>,
    cancel: Option<&CancelSignal>,
    ctx: Ctx,
    call: impl Future<Output = ConnectResult<T>>,
) -> ConnectResult<T> {
    let Some(deadline) = deadline else {
        return call.await;
    };

    let remaining = deadline.saturating_duration_since(ctx.clock().now());
    if remaining.is_zero() {
        // Fail before the call runs, so a past deadline sends nothing to the
        // peer and reads nothing from it.
        if let Some(cancel) = cancel {
            cancel.cancel();
        }
        return Err(ConnectError::deadline_exceeded("call deadline exceeded").into());
    }

    let call = Box::pin(call);
    let sleeper = Box::pin(valtron::sleep_async(remaining, &ctx));
    match futures::future::select(call, sleeper).await {
        futures::future::Either::Left((result, _sleeper)) => result,
        futures::future::Either::Right(((), call)) => {
            // Dropping the call drops the transport stream and response stream
            // it owns; the cancel signal also stops the spawned reader/writer
            // tasks of a streaming call.
            drop(call);
            if let Some(cancel) = cancel {
                cancel.cancel();
            }
            Err(ConnectError::deadline_exceeded("call deadline exceeded").into())
        }
    }
}

/// Run `call` until it finishes or the call's deadline passes.
///
/// WHY: the call setup owns the caller's `RpcCtx`, so the deadline and the cancel
/// signal stay together here. A past deadline must fail before the transport
/// opens and keep the distinct "sent nothing" message from feature 123.
///
/// WHAT: reject a past deadline first, then race `call` with
/// [`with_deadline_at`].
async fn with_deadline<T>(
    ctx: &RpcCtx,
    call: impl Future<Output = ConnectResult<T>>,
) -> ConnectResult<T> {
    let past = ctx.request.deadline.is_some_and(|deadline| {
        deadline
            .saturating_duration_since(ctx.clock().now())
            .is_zero()
    });
    if past {
        // Fail before the transport opens, so a call with a past deadline
        // sends nothing to the peer.
        ctx.cancel_signal().cancel();
        return Err(ConnectError::deadline_exceeded(
            "call deadline passed before the request was sent",
        )
        .into());
    }
    with_deadline_at(
        ctx.request.deadline,
        Some(ctx.cancel_signal()),
        ctx.ctx.clone(),
        call,
    )
    .await
}

/// Build the full procedure URL from the base URL and procedure name.
fn build_url(base_url: &str, procedure: &str) -> String {
    if procedure.is_empty() {
        return base_url.trim_end_matches('/').to_string();
    }
    let base = base_url.trim_end_matches('/');
    let proc_path = procedure.trim_start_matches('/');
    format!("{base}/{proc_path}")
}

/// Build a [`RequestDescriptor`] for a transport `open` call.
///
/// `Uri::parse` accepts both an absolute URL and a path-only URL, so a parse
/// failure is a real error: return it as `Internal` instead of panicking on a
/// fallback.
fn build_request_descriptor(
    url: &str,
    headers: &SimpleHeaders,
    method: SimpleMethod,
) -> ConnectResult<RequestDescriptor> {
    let uri = Uri::parse(url)
        .map_err(|e| ConnectError::internal(format!("invalid request URL {url:?}: {e}")))?;
    Ok(RequestDescriptor {
        proto: Proto::HTTP11,
        request_url: SimpleUrl::url_only(url),
        request_uri: uri,
        headers: headers.clone(),
        method,
    })
}

/// Execute a unary round-trip: open transport, push body, close send, collect
/// response head + body, decode into [`UnaryReply`].
///
/// WHY: `Transport::open` returns a [`TransportStream`] directly (sync with
/// spawned pump). This helper drives the three await points (send, head, body)
/// and maps transport errors into [`ConnectError`].
/// If the trailers contain a non-zero `grpc-status`, build a `ConnectError`
/// from it. Returns `None` for success trailers (grpc-status: 0, or absent).
async fn do_unary_round_trip(
    transport: &dyn Transport,
    desc: RequestDescriptor,
    body: Bytes,
    read_max_bytes: usize,
    error_body_max_bytes: usize,
) -> ConnectResult<UnaryReply> {
    let mut stream = transport.open(desc).map_err(ConnectError::from)?;

    // Push request body.
    if !body.is_empty() {
        stream.send_body.try_send(body).map_err(|()| {
            ConnectError::internal("failed to push request body into transport pipe")
        })?;
    }
    stream.send_body.close();

    // Await response head.
    let (status, resp_headers) = stream
        .head
        .next()
        .await
        .ok_or_else(|| ConnectError::unavailable("transport closed without response head"))?
        .map_err(ConnectError::from)?;

    // Check the status before touching the body. A non-OK body only becomes an
    // error message: read a bounded prefix, then drop the stream. A large or
    // slow error body then neither uses unbounded memory nor delays the error
    // (issue #410).
    if status != Status::OK {
        let mut prefix = Vec::new();
        while prefix.len() < error_body_max_bytes {
            let Some(chunk) = stream.recv_body.next().await else {
                break;
            };
            let chunk = chunk.map_err(ConnectError::from)?;
            let remaining = error_body_max_bytes - prefix.len();
            let take = remaining.min(chunk.len());
            prefix.extend_from_slice(&chunk[..take]);
        }
        // `stream` drops here, so the response body closes early.
        // A Connect unary error body is a JSON `WireError` carrying the exact
        // code; the HTTP status alone is ambiguous (aborted and already_exists
        // are both 409, invalid_argument and failed_precondition both 400).
        // The bounded prefix holds the whole body for any realistic error.
        if let Ok(wire) = serde_json::from_slice::<crate::shared::error::WireError>(&prefix) {
            if let Some(code) = Code::from_str(&wire.code) {
                return Err(ConnectError::wire(code, wire.message.unwrap_or_default()).into());
            }
        }
        let error_body = if prefix.is_empty() {
            format!("HTTP {status}")
        } else {
            String::from_utf8_lossy(&prefix).to_string()
        };
        let code = code_from_http_status(&status);
        let status_code = u16::try_from(status.into_usize()).unwrap_or_default();
        return Err(ConnectError::from_http_status(status_code, code, error_body).into());
    }

    // Collect the response body (one message), never past `read_max_bytes`.
    let mut body_buf = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body_buf.extend_from_slice(&chunk.map_err(ConnectError::from)?);
        if read_max_bytes != 0 && body_buf.len() > read_max_bytes {
            return Err(ConnectError::resource_exhausted(format!(
                "response body exceeds read_max_bytes {read_max_bytes}"
            ))
            .into());
        }
    }

    // Read trailers (h2 trailing HEADERS for gRPC grpc-status; empty for Connect).
    let trailers = stream.trailers.receive().await.unwrap_or_default();

    Ok(UnaryReply {
        headers: resp_headers,
        trailers,
        frame: Bytes::from(body_buf),
    })
}

/// Split a gRPC-Web unary body into its message envelope (header kept, as
/// `decode_unary_response` expects) and the headers of its `0x80` trailer
/// frame.
fn split_grpc_web_unary(body: &Bytes) -> ConnectResult<(Bytes, SimpleHeaders)> {
    const HEADER: usize = 5;
    let mut message = Bytes::new();
    let mut trailers = SimpleHeaders::new();
    let mut offset = 0;
    while offset < body.len() {
        if body.len() - offset < HEADER {
            return Err(ConnectError::invalid_argument("truncated gRPC-Web frame header").into());
        }
        let flags = body[offset];
        let len = u32::from_be_bytes([
            body[offset + 1],
            body[offset + 2],
            body[offset + 3],
            body[offset + 4],
        ]) as usize;
        let end = offset + HEADER + len;
        if end > body.len() {
            return Err(ConnectError::invalid_argument("truncated gRPC-Web frame").into());
        }
        if flags & grpc_web_consts::TRAILER_FLAG != 0 {
            trailers = crate::shared::protocol::grpc_web::parse_trailer_frame_body(
                &body[offset + HEADER..end],
            );
        } else if message.is_empty() {
            message = body.slice(offset..end);
        }
        offset = end;
    }
    Ok((message, trailers))
}

/// Spawn the reader and writer tasks on the valtron pool.
/// Spawn a streaming call's reader and writer tasks. Both end as soon as
/// `cancel` fires: they own the transport's response and request halves, so
/// dropping them closes the stream, and the transport tells the server (h2
/// sends `RST_STREAM(CANCEL)`). Before, a cancelled call kept its transport
/// stream open and the server kept streaming into it.
fn spawn_reader_writer(
    reader_task: BoxFuture<'static, ConnectResult<()>>,
    writer_task: BoxFuture<'static, ConnectResult<()>>,
    cancel: &CancelSignal,
) -> ConnectResult<()> {
    valtron::send(valtron::from_future(until_cancelled(
        reader_task,
        cancel.clone(),
    )))
    .map_err(|e| ConnectError::internal(format!("failed to spawn reader task: {e}")))?;
    valtron::send(valtron::from_future(until_cancelled(
        writer_task,
        cancel.clone(),
    )))
    .map_err(|e| ConnectError::internal(format!("failed to spawn writer task: {e}")))?;
    Ok(())
}

/// Run `task` until it ends or `cancel` fires; on cancel the task (and what it
/// owns) is dropped.
async fn until_cancelled(
    task: BoxFuture<'static, ConnectResult<()>>,
    cancel: CancelSignal,
) -> ConnectResult<()> {
    let cancelled = Box::pin(async move { cancel.cancelled().await });
    match futures::future::select(task, cancelled).await {
        futures::future::Either::Left((result, _)) => result,
        futures::future::Either::Right(((), _task)) => {
            Err(ConnectError::canceled("call canceled").into())
        }
    }
}

/// Optionally compress an already-marshaled frame.
fn compress_if(
    registry: &CompressionRegistry,
    algo: Option<&str>,
    data: &[u8],
) -> ConnectResult<(Vec<u8>, Option<String>)> {
    match algo {
        Some(name) if name != "identity" => {
            let compressor = registry.get(name).ok_or_else(|| {
                ConnectError::internal(format!("unknown compression algorithm: {name}"))
            })?;
            let compressed = compressor.compress(data)?;
            Ok((compressed, Some(name.to_string())))
        }
        _ => Ok((data.to_vec(), None)),
    }
}

/// Add a protocol-appropriate timeout header to the outgoing request headers.
fn add_timeout_header(
    protocol: ProtocolSelection,
    headers: &mut SimpleHeaders,
    timeout: Option<Duration>,
) {
    let Some(d) = timeout else {
        return;
    };
    match protocol {
        ProtocolSelection::Connect => {
            headers.insert(
                SimpleHeader::from(connect_consts::HEADER_TIMEOUT.to_string()),
                vec![d.as_millis().to_string()],
            );
        }
        ProtocolSelection::Grpc | ProtocolSelection::GrpcWeb => {
            headers.insert(
                SimpleHeader::from(grpc_web_consts::HEADER_TIMEOUT.to_string()),
                vec![encode_grpc_timeout(d)],
            );
        }
    }
}

/// Check if an error trace carries a 405 or 415 HTTP status that can be retried
/// as a POST (Decision 07 fallback case 2).
///
/// The error keeps the status it came from, so this check does not scan the
/// message text. A message that happens to contain "405" must not trigger a
/// retry.
fn is_method_not_allowed(err: &ErrorTrace<ConnectError>) -> bool {
    matches!(err.current_context().http_status(), Some(405 | 415))
}

/// Map an HTTP status to an RPC error [`Code`] (Decision 03).
pub(crate) fn code_from_http_status(status: &Status) -> Code {
    let n: usize = status.clone().into();
    match n {
        400 => Code::InvalidArgument,
        401 | 403 => Code::Unauthenticated,
        404 => Code::NotFound,
        408 | 504 => Code::DeadlineExceeded,
        429 => Code::ResourceExhausted,
        499 => Code::Canceled,
        500 => Code::Internal,
        501 => Code::Unimplemented,
        503 => Code::Unavailable,
        _ => Code::Unknown,
    }
}
