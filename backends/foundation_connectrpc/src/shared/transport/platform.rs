//! Use a platform network client for unary calls and server streams.
use std::sync::{Arc, Mutex, PoisonError};

use bytes::Bytes;
use foundation_core::valtron::Pipe;
use foundation_netio::shared::http::{Proto, RequestDescriptor, SimpleHeaders, Status};
use futures::{channel::oneshot, future::BoxFuture, stream, FutureExt};

use super::{
    base::{SendBody, SendBodyFuture},
    Transport, TransportCapabilities, TransportError, TransportStream,
};

/// One platform request. Creating it allocates a handle. Sending starts network I/O.
pub trait PlatformExchange: Send + Sync + 'static {
    /// Sends the complete bounded request and returns the response status and headers.
    fn start(
        &self,
        body: Vec<u8>,
    ) -> BoxFuture<'_, Result<(Status, SimpleHeaders), TransportError>>;
    /// Reads the next response chunk. End of body returns `None`.
    fn read(&self, max_bytes: usize) -> BoxFuture<'_, Result<Option<Vec<u8>>, TransportError>>;
    /// Releases the exchange. Cancels pending network operations.
    fn close(&self);
}

/// Selects the platform session. Each request receives its own handle.
pub trait PlatformHttpClient: Send + Sync + 'static {
    /// Creates the exchange for the method, address, and headers.
    ///
    /// # Errors
    /// Returns the native setup error.
    fn exchange(
        &self,
        request: RequestDescriptor,
    ) -> Result<Arc<dyn PlatformExchange>, TransportError>;
}

#[derive(Clone, Copy, Debug)]
pub struct PlatformHttpConfig {
    /// Maximum encoded request size. Defaults to the standard gRPC receive limit, four megabytes.
    pub request_bytes: usize,
    /// Maximum size per platform read. Defaults to the 16 KiB HTTP/2 data frame size.
    pub chunk_bytes: std::num::NonZeroUsize,
}

impl Default for PlatformHttpConfig {
    fn default() -> Self {
        Self {
            request_bytes: 4 * 1024 * 1024,
            chunk_bytes: std::num::NonZeroUsize::new(16 * 1024).expect("Positive chunk size"),
        }
    }
}

pub struct PlatformHttpTransport {
    client: Arc<dyn PlatformHttpClient>,
    config: PlatformHttpConfig,
}

impl PlatformHttpTransport {
    #[must_use]
    pub fn new(client: Arc<dyn PlatformHttpClient>) -> Self {
        Self {
            client,
            config: PlatformHttpConfig::default(),
        }
    }

    #[must_use]
    pub fn with_config(mut self, config: PlatformHttpConfig) -> Self {
        self.config = config;
        self
    }
}

type Ready = oneshot::Sender<Result<Vec<u8>, TransportError>>;
struct RequestBody {
    limit: usize,
    upload: Mutex<Option<(Ready, Vec<u8>)>>,
}

impl SendBody for RequestBody {
    fn try_send(&self, item: Bytes) -> Result<(), ()> {
        let mut upload = self.upload.lock().unwrap_or_else(PoisonError::into_inner);
        let (_, buffer) = upload.as_mut().ok_or(())?;
        if item.len() > self.limit.saturating_sub(buffer.len()) {
            if let Some((ready, _)) = upload.take() {
                // The response handler can already have canceled this exchange.
                drop(ready.send(Err(TransportError::Protocol(
                    "Request exceeds the platform upload limit".into(),
                ))));
            }
            return Err(());
        }
        buffer.extend_from_slice(&item);
        Ok(())
    }

    fn send_async(&self, item: Bytes) -> SendBodyFuture<'_> {
        Box::pin(std::future::ready(self.try_send(item)))
    }

    fn close(&self) {
        if let Some((ready, body)) = self
            .upload
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            drop(ready.send(Ok(body)));
        }
    }

    fn is_closed(&self) -> bool {
        self.upload
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_none()
    }
}

struct Guard(Arc<dyn PlatformExchange>);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.close();
    }
}

impl Transport for PlatformHttpTransport {
    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            request_streaming: false,
            full_duplex: false,
            h2_trailers: false,
            http_versions: &[Proto::HTTP11],
            multiplexed: false,
        }
    }

    fn open(&self, request: RequestDescriptor) -> Result<TransportStream, TransportError> {
        let exchange = Arc::new(Guard(self.client.exchange(request)?));
        let (ready, body) = oneshot::channel();
        let for_head = exchange.clone();
        let head = async move {
            let bytes = body.await.map_err(|_| TransportError::Canceled)??;
            for_head.0.start(bytes).await
        }
        .boxed()
        .shared();
        let for_body = head.clone();
        let max = self.config.chunk_bytes.get();
        let body = stream::try_unfold(
            (Some(for_body), exchange),
            move |(head, exchange)| async move {
                if let Some(head) = head {
                    head.await?;
                }
                match exchange.0.read(max).await? {
                    Some(chunk) if chunk.is_empty() || chunk.len() > max => Err(
                        TransportError::Protocol("Invalid platform response chunk size".into()),
                    ),
                    Some(chunk) => Ok(Some((Bytes::from(chunk), (None, exchange)))),
                    None => Ok(None),
                }
            },
        );
        let (end, trailers) = Pipe::with_depth(1);
        drop(end);
        Ok(TransportStream {
            send_body: Arc::new(RequestBody {
                limit: self.config.request_bytes,
                upload: Mutex::new(Some((ready, Vec::new()))),
            }),
            head: Box::pin(stream::once(head)),
            recv_body: Box::pin(body),
            trailers,
        })
    }
}
