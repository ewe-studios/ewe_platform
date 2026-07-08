//! Client core (F24): typed `Client<Req,Res>`, `ClientOptions`, per-call `Ctx`,
//! and the four stream facades (Decision 07).
//!
//! WHY: Every RPC invocation flows through the Client's typed surface. The Client
//! owns the transport, protocol, codecs, and configuration, and derives a per-call
//! Ctx for every invocation (deadline merge, linked cancel, spec/peer overwritten).
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

use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use foundation_core::url::Uri;
use foundation_core::valtron;
use foundation_errstacks::ErrorTrace;
use foundation_netio::simple_http::shared::{
    Proto, RequestDescriptor, SimpleHeader, SimpleHeaders, SimpleMethod, SimpleUrl, Status,
};
use futures::{Stream, StreamExt};

use crate::codec::{CodecFor, ProcedureCodecs};
use crate::compression::{CompressionRegistry, Compressor};
use crate::context::{CancelSignal, Ctx, IdempotencyLevel, Peer, RequestContext, Spec, StreamType};
use crate::envelope::encode_grpc_timeout;
use crate::error::{Code, ConnectError, ConnectResult};
use crate::interceptor::{Interceptor, InterceptorChain, UnaryCall, UnaryFunc, UnaryReply};
use crate::message::{Request, Response};
use crate::protocol::connect::constants as connect_consts;
use crate::protocol::grpc_web::constants as grpc_web_consts;
use crate::protocol::{ClientExchange, ProtocolClient};
use crate::ConnectClient;
use crate::GrpcWebClient;
use crate::transport::{
    capabilities::{check_compatible, requirements, ProtocolKind},
    conn::{ClientReceiver, ClientSender},
    frame::BoxFuture,
    Transport, TransportStream, DEFAULT_PIPE_DEPTH,
};

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
    fn peer_protocol_name(&self) -> &'static str {
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
#[derive(Debug, Clone)]
pub struct SizeLimits {
    /// Maximum response body size (`0` = unlimited).
    pub read_max_bytes: usize,
    /// Maximum request body size (`0` = unlimited).
    pub send_max_bytes: usize,
    /// Minimum payload size for compression (`0` = always compress).
    pub compress_min_bytes: usize,
}

impl Default for SizeLimits {
    fn default() -> Self {
        Self {
            read_max_bytes: 0,
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
    /// GET→POST fallback URL length threshold (default 8 KiB). `0` disables the
    /// preflight check (Decision 07 P14).
    pub get_url_max_bytes: usize,
    /// Retry a GET that was rejected with 405/415 as POST (Decision 07).
    pub get_use_fallback: bool,
    /// Preferred HTTP version for capability matching (Decision 11 T12).
    pub preferred_http_version: Option<Proto>,
    /// Seam pipe depth (frames in flight per direction).
    pub pipe_depth: usize,
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
            get_url_max_bytes: 8192,
            get_use_fallback: true,
            preferred_http_version: None,
            pipe_depth: DEFAULT_PIPE_DEPTH,
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

    /// Maximum response body size in bytes (`0` = unlimited).
    #[must_use]
    pub fn with_read_max_bytes(mut self, n: usize) -> Self {
        self.config.limits.read_max_bytes = n;
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
/// every RPC invocation derives a fresh per-call [`Ctx`] (deadline merge, linked
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
            ClientConfig { url: url.to_string(), ..config }
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
            ProtocolSelection::Grpc => {
                return Err(
                    ConnectError::unimplemented("gRPC requires HTTP/2, not yet available").into(),
                );
            }
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
    pub async fn unary(&self, ctx: Ctx, request: Request<Req>) -> ConnectResult<Response<Res>> {
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
            return self.call_unary_get_or_fallback(call_ctx, request, url).await;
        }

        self.call_unary_post(call_ctx, request, url).await
    }

    /// Server-streaming RPC: one request, stream of responses.
    ///
    /// # Errors
    /// A trace on transport failure, capability mismatch, or protocol setup failure.
    pub async fn server_stream(
        &self,
        ctx: Ctx,
        request: Request<Req>,
    ) -> ConnectResult<ServerStream<Res>> {
        let spec = Spec {
            stream_type: StreamType::ServerStream,
            procedure: String::new(),
            is_client: true,
            idempotency: self.idempotency,
        };
        let call_ctx = self.derive_call_ctx(&ctx, spec);
        let spec = call_ctx.spec().clone();
        let peer = call_ctx.peer().clone();
        let url = build_url(&self.config.url, &spec.procedure);

        // Validate streaming capability.
        let reqs = requirements(self.config.protocol.to_kind(), StreamType::ServerStream);
        check_compatible(&reqs, &self.transport.capabilities())?;

        // Marshal the request.
        let req_codec = self.codecs.for_request(&self.config.codec_name)?;
        let encoded = req_codec.marshal(&request.msg)?;
        let (body, content_encoding) =
            compress_if(&self.config.compression, &self.config.send_compression, &encoded)?;

        // Build headers.
        let mut headers = request.headers().clone();
        self.protocol.write_request_headers(
            StreamType::ServerStream,
            &mut headers,
            &self.config.codec_name,
            content_encoding.as_deref(),
        );
        add_timeout_header(
            &self.config.protocol,
            &mut headers,
            call_ctx.remaining_timeout(),
        );

        // Build transport request.
        let desc = build_request_descriptor(&url, &headers, SimpleMethod::POST);
        let transport_stream = self
            .transport
            .open(desc)
            .map_err(|e| ConnectError::from(e))?;

        // Build the ClientExchange with linked cancel.
        let exchange = self.build_streaming_exchange(&spec, headers, transport_stream, &call_ctx)?;

        // Spawn reader/writer tasks.
        spawn_reader_writer(exchange.reader_task, exchange.writer_task)?;

        let (mut sender, mut receiver) = exchange.conn.split();

        // Send the request frame and close.
        sender.send(Bytes::from(body)).await?;
        sender.close_send().await?;

        // Await the response head so the caller's `response_headers()` is sync-OK.
        let response_headers = receiver.response_headers().await?.clone();
        let res_codec = self.codecs.for_response(&self.config.codec_name)?;

        Ok(ServerStream {
            recv: receiver,
            codec: res_codec,
            spec,
            peer,
            response_headers,
            trailers: SimpleHeaders::new(),
            got_trailers: false,
        })
    }

    /// Client-streaming RPC: stream of requests, one response.
    ///
    /// # Errors
    /// A trace on transport failure, capability mismatch, or protocol setup failure.
    pub async fn client_stream(
        &self,
        ctx: Ctx,
        reqs: impl Stream<Item = Req>,
    ) -> ConnectResult<Response<Res>> {
        let spec = Spec {
            stream_type: StreamType::ClientStream,
            procedure: String::new(),
            is_client: true,
            idempotency: self.idempotency,
        };
        let call_ctx = self.derive_call_ctx(&ctx, spec);
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
            &self.config.protocol,
            &mut headers,
            call_ctx.remaining_timeout(),
        );

        let desc = build_request_descriptor(&url, &headers, SimpleMethod::POST);
        let transport_stream = self
            .transport
            .open(desc)
            .map_err(|e| ConnectError::from(e))?;

        let exchange = self.build_streaming_exchange(&spec, headers, transport_stream, &call_ctx)?;
        spawn_reader_writer(exchange.reader_task, exchange.writer_task)?;

        let (mut sender, mut receiver) = exchange.conn.split();
        let req_codec = self.codecs.for_request(&self.config.codec_name)?;
        let res_codec = self.codecs.for_response(&self.config.codec_name)?;

        // Send all request messages.
        let mut pinned_reqs = Box::pin(reqs);
        while let Some(msg) = pinned_reqs.next().await {
            let frame = req_codec.marshal(&msg)?;
            sender.send(frame).await?;
        }
        sender.close_send().await?;

        // Receive the single response.
        let frame = receiver
            .receive()
            .await?
            .ok_or_else(|| ConnectError::internal("client stream: expected a response frame"))?;
        let decoded = res_codec.unmarshal(frame)?;

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
    /// # Errors
    /// A trace on transport failure, capability mismatch, or protocol setup failure.
    pub async fn bidi_stream(
        &self,
        ctx: Ctx,
        _reqs: impl Stream<Item = Req>,
    ) -> ConnectResult<BidiStream<Req, Res>> {
        let spec = Spec {
            stream_type: StreamType::BidiStream,
            procedure: String::new(),
            is_client: true,
            idempotency: self.idempotency,
        };
        let call_ctx = self.derive_call_ctx(&ctx, spec);
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
            &self.config.protocol,
            &mut headers,
            call_ctx.remaining_timeout(),
        );

        let desc = build_request_descriptor(&url, &headers, SimpleMethod::POST);
        let transport_stream = self
            .transport
            .open(desc)
            .map_err(|e| ConnectError::from(e))?;

        let exchange = self.build_streaming_exchange(&spec, headers, transport_stream, &call_ctx)?;
        spawn_reader_writer(exchange.reader_task, exchange.writer_task)?;

        let (sender, receiver) = exchange.conn.split();
        let req_codec = self.codecs.for_request(&self.config.codec_name)?;
        let res_codec = self.codecs.for_response(&self.config.codec_name)?;

        Ok(BidiStream {
            send: sender,
            recv: receiver,
            req_codec,
            res_codec,
            spec,
            peer,
        })
    }

    // ════════════════════════════════════════════════════════════════════════
    // Internal helpers
    // ════════════════════════════════════════════════════════════════════════

    /// Derive a per-call [`Ctx`] from the caller's context (Decision 07 per-call
    /// Ctx contract table).
    fn derive_call_ctx(&self, caller: &Ctx, spec: Spec) -> Ctx {
        let deadline = match (caller.request.deadline, self.config.default_timeout) {
            (Some(caller_deadline), Some(default)) => {
                let earliest = Instant::now() + default;
                Some(caller_deadline.min(earliest))
            }
            (Some(caller_deadline), None) => Some(caller_deadline),
            (None, Some(default)) => Some(Instant::now() + default),
            (None, None) => None,
        };

        let peer = Peer {
            addr: self.config.url.clone(),
            protocol: self.config.protocol.peer_protocol_name().to_string(),
        };

        Ctx {
            bag: caller.bag.clone(),
            request: RequestContext::for_dispatch(
                spec,
                peer,
                SimpleHeaders::new(),
                deadline,
                caller.request.extensions.clone(),
                caller.request.connection.clone(),
                CancelSignal::linked(caller.cancel_signal()),
            ),
        }
    }

    /// Build a [`ClientExchange`] with the cancel signal linked to the call ctx.
    fn build_streaming_exchange(
        &self,
        spec: &Spec,
        headers: SimpleHeaders,
        stream: TransportStream,
        ctx: &Ctx,
    ) -> ConnectResult<ClientExchange> {
        self.protocol.new_conn(spec, headers, stream, CancelSignal::linked(ctx.cancel_signal()))
    }

    // ── POST unary ─────────────────────────────────────────────────────────

    /// POST-based unary call implementation.
    async fn call_unary_post(
        &self,
        ctx: Ctx,
        request: Request<Req>,
        url: String,
    ) -> ConnectResult<Response<Res>> {
        // Marshal the request.
        let req_codec = self.codecs.for_request(&self.config.codec_name)?;
        let encoded = req_codec.marshal(&request.msg)?;
        let (body, content_encoding) =
            compress_if(&self.config.compression, &self.config.send_compression, &encoded)?;

        // Build headers: caller's request headers + protocol headers.
        let mut headers = request.headers().clone();
        self.protocol.write_request_headers(
            StreamType::Unary,
            &mut headers,
            &self.config.codec_name,
            content_encoding.as_deref(),
        );
        add_timeout_header(&self.config.protocol, &mut headers, ctx.remaining_timeout());

        let call = UnaryCall {
            headers: headers.clone(),
            codec_name: self.config.codec_name.clone(),
            frame: Bytes::from(body.clone()),
        };

        // Build the inner unary function.
        let transport = Arc::clone(&self.transport);
        let url_clone = url.clone();

        let inner: UnaryFunc = Arc::new(move |_ctx: Ctx, call: UnaryCall| {
            let transport = Arc::clone(&transport);
            let url = url_clone.clone();
            Box::pin(async move {
                let uri = Uri::parse(&url)
                    .map_err(|e| ConnectError::internal(format!("invalid URL: {e}")))?;
                let desc = RequestDescriptor {
                    proto: Proto::HTTP11,
                    request_url: SimpleUrl::url_only(&url),
                    request_uri: uri,
                    headers: call.headers,
                    method: SimpleMethod::POST,
                };

                // Execute via transport.open + push + close + read.
                do_unary_round_trip(&*transport, desc, call.frame).await
            })
        });

        // Apply interceptor chain.
        let chain = InterceptorChain::new(self.config.interceptors.clone());
        let wrapped = chain.wrap_unary(inner);
        let reply = wrapped(ctx.clone(), call).await?;

        // Decode response.
        let res_codec = self.codecs.for_response(&self.config.codec_name)?;
        let msg = res_codec.unmarshal(reply.frame)?;

        let mut response = Response::new(msg);
        for (h, vals) in &reply.headers {
            response.headers_mut().insert(h.clone(), vals.clone());
        }
        for (h, vals) in &reply.trailers {
            response.trailers_mut().insert(h.clone(), vals.clone());
        }

        Ok(response)
    }

    // ── HTTP GET + fallback ───────────────────────────────────────────────

    /// HTTP GET for idempotent unary, with automatic POST fallback (Decision 07).
    ///
    /// Builds a GET URL with the message in the query string, executes it, and
    /// falls back to POST in the two deterministic cases:
    /// 1. URL too long → pre-flight fallback (never sent).
    /// 2. Server rejected with 405/415 → safe single retry as POST (rejection
    ///    precedes execution; no double-send risk).
    /// Transport errors are NOT retried (the GET may have executed).
    async fn call_unary_get_or_fallback(
        &self,
        ctx: Ctx,
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

        let result = do_unary_round_trip(&*self.transport, desc, Bytes::new()).await;

        match result {
            Ok(reply) => {
                let res_codec = self.codecs.for_response(&self.config.codec_name)?;
                let msg = res_codec.unmarshal(reply.frame)?;
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
        use crate::protocol::connect::encode_get_query;

        let procedure = if request.spec().procedure.is_empty() {
            "Method"
        } else {
            &request.spec().procedure
        };

        let req_codec = self.codecs.for_request(&self.config.codec_name)?;
        let stable = req_codec.marshal_stable(&request.msg)?;

        let compressor = self.config.send_compression.as_ref().and_then(|name| {
            self.config.compression.get(name).cloned()
        });

        let query = encode_get_query(
            &self.config.codec_name,
            &stable,
            req_codec.is_binary(),
            compressor.as_ref(),
        )?;

        Ok(format!("{base_url}/{procedure}?{query}"))
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
}

impl<Res: Send + 'static> ServerStream<Res> {
    /// Receive the next decoded response message. Returns `Ok(None)` at end of
    /// stream (clean EOF). A terminal error returns `Err`.
    pub async fn receive(&mut self) -> ConnectResult<Option<Res>> {
        match self.recv.receive().await? {
            Some(frame) => Ok(Some(self.codec.unmarshal(frame)?)),
            None => Ok(None),
        }
    }

    /// Response headers (available immediately after the call is opened).
    #[must_use]
    pub fn response_headers(&self) -> &SimpleHeaders {
        &self.response_headers
    }

    /// Response trailers (available after end of stream).
    pub async fn response_trailers(&mut self) -> ConnectResult<&SimpleHeaders> {
        if !self.got_trailers {
            self.trailers = self.recv.response_trailers().await?.clone();
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
    req_codec: Arc<dyn CodecFor<Req>>,
    res_codec: Arc<dyn CodecFor<Res>>,
}

impl<Req: Send + 'static, Res: Send + 'static> ClientStream<Req, Res> {
    /// Mutable request headers (valid before the first send).
    pub fn request_headers_mut(&mut self) -> &mut SimpleHeaders {
        self.send.request_headers_mut()
    }

    /// Encode and send one request message.
    pub async fn send(&mut self, msg: &Req) -> ConnectResult<()> {
        let frame = self.req_codec.marshal(msg)?;
        self.send.send(frame).await
    }

    /// Half-close the request and await the single response.
    pub async fn close_and_receive(self) -> ConnectResult<Response<Res>> {
        // Close send half.
        let send = Box::new(self.send);
        send.close_send().await?;

        // Receive the response.
        let mut recv = self.recv;
        match recv.receive().await? {
            Some(frame) => {
                let msg = self.res_codec.unmarshal(frame)?;
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
    req_codec: Arc<dyn CodecFor<Req>>,
    res_codec: Arc<dyn CodecFor<Res>>,
    spec: Spec,
    peer: Peer,
}

impl<Req: Send + 'static, Res: Send + 'static> BidiStream<Req, Res> {
    /// Mutable request headers (valid before the first send).
    pub fn request_headers_mut(&mut self) -> &mut SimpleHeaders {
        self.send.request_headers_mut()
    }

    /// Encode and send one request message.
    pub async fn send(&mut self, msg: &Req) -> ConnectResult<()> {
        let frame = self.req_codec.marshal(msg)?;
        self.send.send(frame).await
    }

    /// Header-only send (no body) — flush request headers.
    pub async fn send_headers(&mut self) -> ConnectResult<()> {
        self.send.flush_headers().await
    }

    /// Half-close the request direction (receiver stays live).
    pub async fn close_request(&mut self) -> ConnectResult<()> {
        let old = std::mem::replace(&mut self.send, Box::new(NoopClientSender));
        old.close_send().await
    }

    /// Receive the next decoded response message.
    pub async fn receive(&mut self) -> ConnectResult<Option<Res>> {
        match self.recv.receive().await? {
            Some(frame) => Ok(Some(self.res_codec.unmarshal(frame)?)),
            None => Ok(None),
        }
    }

    /// Response headers (valid once the server starts sending).
    pub async fn response_headers(&mut self) -> ConnectResult<&SimpleHeaders> {
        self.recv.response_headers().await
    }

    /// Response trailers (available after end of stream).
    pub async fn response_trailers(&mut self) -> ConnectResult<&SimpleHeaders> {
        self.recv.response_trailers().await
    }

    /// Split into independently-owned typed halves for concurrent send/receive.
    pub fn split(self) -> (BidiSender<Req>, BidiReceiver<Res>) {
        let sender = BidiSender {
            send: self.send,
            codec: self.req_codec,
        };
        let receiver = BidiReceiver {
            recv: self.recv,
            codec: self.res_codec,
            spec: self.spec,
            peer: self.peer,
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
}

impl<Req: Send + 'static> BidiSender<Req> {
    /// Encode and send one request message.
    pub async fn send(&mut self, msg: &Req) -> ConnectResult<()> {
        let frame = self.codec.marshal(msg)?;
        self.send.send(frame).await
    }

    /// Header-only send (flush headers).
    pub async fn send_headers(&mut self) -> ConnectResult<()> {
        self.send.flush_headers().await
    }

    /// Half-close the request direction.
    pub async fn close_request(self) -> ConnectResult<()> {
        Box::new(self.send).close_send().await
    }
}

/// Typed receive half of a bidi stream — for concurrent send/receive.
pub struct BidiReceiver<Res> {
    recv: Box<dyn ClientReceiver>,
    codec: Arc<dyn CodecFor<Res>>,
    spec: Spec,
    peer: Peer,
}

impl<Res: Send + 'static> BidiReceiver<Res> {
    /// Receive the next decoded response message.
    pub async fn receive(&mut self) -> ConnectResult<Option<Res>> {
        match self.recv.receive().await? {
            Some(frame) => Ok(Some(self.codec.unmarshal(frame)?)),
            None => Ok(None),
        }
    }

    /// Response headers.
    pub async fn response_headers(&mut self) -> ConnectResult<&SimpleHeaders> {
        self.recv.response_headers().await
    }

    /// Response trailers.
    pub async fn response_trailers(&mut self) -> ConnectResult<&SimpleHeaders> {
        self.recv.response_trailers().await
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
fn build_request_descriptor(url: &str, headers: &SimpleHeaders, method: SimpleMethod) -> RequestDescriptor {
    let uri = Uri::parse(url).unwrap_or_else(|_| {
        // Fallback: construct a minimal URI from just the path portion.
        Uri::parse(&format!("http://localhost{url}")).expect("fallback URI")
    });
    RequestDescriptor {
        proto: Proto::HTTP11,
        request_url: SimpleUrl::url_only(url),
        request_uri: uri,
        headers: headers.clone(),
        method,
    }
}

/// Execute a unary round-trip: open transport, push body, close send, collect
/// response head + body, decode into [`UnaryReply`].
///
/// WHY: `Transport::open` returns a [`TransportStream`] directly (sync with
/// spawned pump). This helper drives the three await points (send, head, body)
/// and maps transport errors into [`ConnectError`].
async fn do_unary_round_trip(
    transport: &dyn Transport,
    desc: RequestDescriptor,
    body: Bytes,
) -> ConnectResult<UnaryReply> {
    let mut stream = transport.open(desc).map_err(|e| ConnectError::from(e))?;

    // Push request body.
    if !body.is_empty() {
        stream.send_body.try_send(body).map_err(|_| {
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
        .map_err(|e| ConnectError::from(e))?;

    // Collect response body.
    let mut body_buf = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body_buf.extend_from_slice(&chunk.map_err(|e| ConnectError::from(e))?);
    }

    if status == Status::OK {
        Ok(UnaryReply {
            headers: resp_headers,
            trailers: SimpleHeaders::new(),
            frame: Bytes::from(body_buf),
        })
    } else {
        let error_body = if body_buf.is_empty() {
            format!("HTTP {status}")
        } else {
            String::from_utf8_lossy(&body_buf).to_string()
        };
        Err(ConnectError::new(code_from_http_status(&status), error_body).into())
    }
}

/// Spawn the reader and writer tasks on the valtron pool.
fn spawn_reader_writer(
    reader_task: BoxFuture<'static, ConnectResult<()>>,
    writer_task: BoxFuture<'static, ConnectResult<()>>,
) -> ConnectResult<()> {
    valtron::send(valtron::from_future(reader_task))
        .map_err(|e| ConnectError::internal(format!("failed to spawn reader task: {e}")))?;
    valtron::send(valtron::from_future(writer_task))
        .map_err(|e| ConnectError::internal(format!("failed to spawn writer task: {e}")))?;
    Ok(())
}

/// Optionally compress an already-marshaled frame.
fn compress_if(
    registry: &CompressionRegistry,
    algo: &Option<String>,
    data: &[u8],
) -> ConnectResult<(Vec<u8>, Option<String>)> {
    match algo {
        Some(name) if name != "identity" => {
            let compressor = registry.get(name).ok_or_else(|| {
                ConnectError::internal(format!("unknown compression algorithm: {name}"))
            })?;
            let compressed = compressor.compress(data)?;
            Ok((compressed, Some(name.clone())))
        }
        _ => Ok((data.to_vec(), None)),
    }
}

/// Add a protocol-appropriate timeout header to the outgoing request headers.
fn add_timeout_header(
    protocol: &ProtocolSelection,
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

/// Check if an error trace indicates a 405 or 415 HTTP response that can be
/// retried as a POST (Decision 07 fallback case 2).
fn is_method_not_allowed(err: &ErrorTrace<ConnectError>) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("405") || msg.contains("method not allowed")
        || msg.contains("415") || msg.contains("unsupported media type")
}

/// Map an HTTP status to an RPC error [`Code`] (Decision 03).
fn code_from_http_status(status: &Status) -> Code {
    let n: usize = status.clone().into();
    match n {
        400 => Code::InvalidArgument,
        401 | 403 => Code::Unauthenticated,
        404 => Code::NotFound,
        408 => Code::DeadlineExceeded,
        429 => Code::ResourceExhausted,
        499 => Code::Canceled,
        500 => Code::Internal,
        501 => Code::Unimplemented,
        503 => Code::Unavailable,
        504 => Code::DeadlineExceeded,
        _ => Code::Unknown,
    }
}
