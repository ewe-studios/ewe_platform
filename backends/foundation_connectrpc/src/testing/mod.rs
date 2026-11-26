//! The in-memory network and transport for deterministic simulation tests.
//!
//! WHY: a deterministic simulation test (DST) must run the client and the
//! server in one process, with no socket and no real Input/Output (I/O).
//!
//! WHAT: [`MemNetwork`] (a `ConnectRPC` router reached in process),
//! [`MemTransport`] (a `Transport` over it) and the exchange that dispatches
//! one request and collects the response.
//!
//! HOW: the transport reads the complete request body, dispatches it through
//! the frozen router with [`ConnectRpcHandler::dispatch`], and serves the
//! collected response body in chunks. A streaming response is collected until
//! it ends, which is correct for a test and costs no socket.

use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex, PoisonError};

use bytes::Bytes;
use foundation_context::Ctx;
use foundation_netio::shared::http::{
    Proto, RequestDescriptor, SendSafeBody, SimpleHeaders, SimpleIncomingRequest, SimpleUrl, Status,
};
use futures::future::BoxFuture;

use crate::shared::router::{ConnectRpcHandler, Router};
use crate::shared::transport::platform::{
    PlatformExchange, PlatformHttpClient, PlatformHttpConfig, PlatformHttpTransport,
};
use crate::shared::transport::{Transport, TransportCapabilities, TransportError, TransportStream};

/// Default largest response chunk. 16 mebibytes (MiB): the chunk size pays no
/// network cost in process, so it is one large value that avoids a copy per
/// frame.
pub const DEFAULT_MEM_CHUNK_BYTES: usize = 16 * 1024 * 1024;

/// One in-process server: a `ConnectRPC` router with no socket.
pub struct MemNetwork {
    handler: Arc<ConnectRpcHandler>,
    ctx: Ctx,
    chunk_bytes: NonZeroUsize,
}

impl std::fmt::Debug for MemNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemNetwork").finish_non_exhaustive()
    }
}

impl MemNetwork {
    /// A network over `router`. Each request gets `ctx` as its application
    /// context.
    #[must_use]
    pub fn new(router: Router, ctx: Ctx) -> Self {
        Self {
            handler: Arc::new(router.into_handler()),
            ctx,
            chunk_bytes: NonZeroUsize::new(DEFAULT_MEM_CHUNK_BYTES)
                .expect("the default chunk size is positive"),
        }
    }

    /// Set the largest response chunk that [`MemTransport`] serves.
    #[must_use]
    pub fn with_chunk_bytes(mut self, chunk_bytes: NonZeroUsize) -> Self {
        self.chunk_bytes = chunk_bytes;
        self
    }

    /// The application context that each request gets.
    #[must_use]
    pub fn ctx(&self) -> &Ctx {
        &self.ctx
    }
}

impl PlatformHttpClient for MemNetwork {
    fn exchange(
        &self,
        request: RequestDescriptor,
    ) -> Result<Arc<dyn PlatformExchange>, TransportError> {
        Ok(Arc::new(MemExchange {
            handler: Arc::clone(&self.handler),
            ctx: self.ctx.clone(),
            request: Mutex::new(Some(request)),
            response: Mutex::new(None),
            cursor: Mutex::new(0),
        }))
    }
}

/// One in-process exchange.
struct MemExchange {
    handler: Arc<ConnectRpcHandler>,
    ctx: Ctx,
    request: Mutex<Option<RequestDescriptor>>,
    response: Mutex<Option<foundation_netio::shared::http::SimpleOutgoingResponse>>,
    cursor: Mutex<usize>,
}

impl PlatformExchange for MemExchange {
    fn start(
        &self,
        body: Vec<u8>,
    ) -> BoxFuture<'_, Result<(Status, SimpleHeaders), TransportError>> {
        Box::pin(async move {
            let request = self
                .request
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take()
                .ok_or_else(|| {
                    TransportError::Protocol("the in-memory exchange started twice".to_string())
                })?;
            let incoming = SimpleIncomingRequest::builder()
                .with_proto(request.proto)
                .with_method(request.method)
                .with_headers(request.headers)
                .with_url(request.request_url)
                .with_uri(request.request_uri)
                .with_body_bytes(body)
                .build()
                .map_err(|error| {
                    TransportError::Protocol(format!("the in-memory request: {error:?}"))
                })?;
            let response = self.handler.dispatch(self.ctx.clone(), incoming).await;
            let head = (response.status.clone(), response.headers.clone());
            *self.response.lock().unwrap_or_else(PoisonError::into_inner) = Some(response);
            *self.cursor.lock().unwrap_or_else(PoisonError::into_inner) = 0;
            Ok(head)
        })
    }

    fn read(&self, max_bytes: usize) -> BoxFuture<'_, Result<Option<Vec<u8>>, TransportError>> {
        Box::pin(async move {
            let response = self.response.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(response) = response.as_ref() else {
                return Err(TransportError::Protocol(
                    "the in-memory exchange read before it started".to_string(),
                ));
            };
            let bytes: &[u8] = match response.body.as_ref() {
                Some(SendSafeBody::Bytes(bytes)) => bytes,
                Some(SendSafeBody::Text(text)) => text.as_bytes(),
                _ => &[],
            };
            let mut cursor = self.cursor.lock().unwrap_or_else(PoisonError::into_inner);
            if *cursor >= bytes.len() {
                return Ok(None);
            }
            let end = cursor.saturating_add(max_bytes).min(bytes.len());
            let chunk = bytes[*cursor..end].to_vec();
            *cursor = end;
            Ok(Some(chunk))
        })
    }

    fn close(&self) {
        *self.response.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

/// A `Transport` over a [`MemNetwork`]. It opens no socket.
pub struct MemTransport(PlatformHttpTransport);

impl std::fmt::Debug for MemTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemTransport").finish_non_exhaustive()
    }
}

impl MemTransport {
    /// A transport over `network`.
    #[must_use]
    pub fn new(network: Arc<MemNetwork>) -> Self {
        let chunk_bytes = network.chunk_bytes;
        Self(
            PlatformHttpTransport::new(network).with_config(PlatformHttpConfig {
                chunk_bytes,
                ..PlatformHttpConfig::default()
            }),
        )
    }
}

impl Transport for MemTransport {
    fn capabilities(&self) -> TransportCapabilities {
        self.0.capabilities()
    }

    fn open(&self, request: RequestDescriptor) -> Result<TransportStream, TransportError> {
        self.0.open(request)
    }

    fn uses_fixed_request_body(&self) -> bool {
        self.0.uses_fixed_request_body()
    }

    fn open_with_body(
        &self,
        request: RequestDescriptor,
        body: Bytes,
    ) -> Result<TransportStream, TransportError> {
        self.0.open_with_body(request, body)
    }
}

/// A request descriptor for a `ConnectRPC` unary or server-streaming call.
///
/// # Errors
/// [`TransportError::Protocol`] when the URL does not parse.
pub fn request_descriptor(
    url: &str,
    headers: SimpleHeaders,
) -> Result<RequestDescriptor, TransportError> {
    let uri = foundation_core::url::Uri::parse(url)
        .map_err(|error| TransportError::Protocol(format!("the call URL {url:?}: {error:?}")))?;
    Ok(RequestDescriptor {
        proto: Proto::HTTP11,
        request_url: SimpleUrl::url_with_query(url),
        request_uri: uri,
        headers,
        method: foundation_netio::shared::http::SimpleMethod::POST,
    })
}
