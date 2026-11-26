//! Swift and Kotlin supply the network session for generated Connect clients.
use std::{num::NonZeroUsize, sync::Arc};

use foundation_connectrpc::{ClientOptions, TransportError};
use foundation_netio::shared::http::{RequestDescriptor, SimpleHeaders, Status};
use foundation_ui_connectrpc::{
    v1::ActionsClient, ConnectActionTransport, PlatformExchange, PlatformHttpClient,
    PlatformHttpConfig, PlatformHttpTransport,
};
use foundation_ui_runtime::action::{ActionTransport, ManagerError};
use foundation_ui_wire::{writer::ClientCaps, WireLimits};
use futures_util::future::BoxFuture;

use crate::{CoreError, HostTransport, NativePlatform, ServiceError};

#[derive(uniffi::Record)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}
#[derive(uniffi::Record)]
pub struct ResponseHead {
    pub status: u16,
    pub headers: Vec<HttpHeader>,
}

/// One client can serve multiple simultaneous exchanges with separate credentials per client.
#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait HostHttpClient: Send + Sync {
    /// # Errors
    /// Returns the native setup error.
    fn new_exchange(
        &self,
        method: String,
        url: String,
        headers: Vec<HttpHeader>,
    ) -> Result<u64, CoreError>;
    async fn start(&self, handle: u64, body: Vec<u8>) -> Result<ResponseHead, CoreError>;
    async fn read(&self, handle: u64, max_bytes: u32) -> Result<Option<Vec<u8>>, CoreError>;
    fn close(&self, handle: u64);
}

/// Holds the injected native network client for multiple generated services.
#[derive(uniffi::Object)]
pub struct NativeHttpClient {
    client: Arc<dyn HostHttpClient>,
}

#[uniffi::export]
impl NativeHttpClient {
    #[uniffi::constructor]
    #[must_use]
    pub fn new(client: Arc<dyn HostHttpClient>) -> Arc<Self> {
        Arc::new(Self { client })
    }
}

#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait HttpServiceProvider: Send + Sync {
    async fn http_client(&self, service: String) -> Result<Arc<NativeHttpClient>, ServiceError>;
}

struct Exchange {
    client: Arc<dyn HostHttpClient>,
    handle: u64,
}
fn transport_error(error: CoreError) -> TransportError {
    TransportError::Connect(Arc::new(error))
}

impl PlatformHttpClient for NativeHttpClient {
    fn exchange(
        &self,
        req: RequestDescriptor,
    ) -> Result<Arc<dyn PlatformExchange>, TransportError> {
        let headers = req
            .headers
            .into_iter()
            .flat_map(|(name, values)| {
                values.into_iter().map(move |value| HttpHeader {
                    name: name.to_string(),
                    value,
                })
            })
            .collect();
        let handle = self
            .client
            .new_exchange(req.method.to_string(), req.request_uri.to_string(), headers)
            .map_err(transport_error)?;
        Ok(Arc::new(Exchange {
            client: self.client.clone(),
            handle,
        }))
    }
}

impl PlatformExchange for Exchange {
    fn start(
        &self,
        body: Vec<u8>,
    ) -> BoxFuture<'_, Result<(Status, SimpleHeaders), TransportError>> {
        Box::pin(async move {
            let response = self
                .client
                .start(self.handle, body)
                .await
                .map_err(transport_error)?;
            if !(100..=599).contains(&response.status) {
                return Err(TransportError::Protocol("Invalid response status".into()));
            }
            let mut headers = SimpleHeaders::new();
            for entry in response.headers {
                headers
                    .entry(entry.name.into())
                    .or_default()
                    .push(entry.value);
            }
            Ok((Status::from(response.status.to_string()), headers))
        })
    }
    fn read(&self, max_bytes: usize) -> BoxFuture<'_, Result<Option<Vec<u8>>, TransportError>> {
        Box::pin(async move {
            let max = u32::try_from(max_bytes)
                .map_err(|_| TransportError::Protocol("Chunk size exceeds native limit".into()))?;
            self.client
                .read(self.handle, max)
                .await
                .map_err(transport_error)
        })
    }
    fn close(&self) {
        self.client.close(self.handle);
    }
}

#[derive(uniffi::Object)]
pub struct ConnectSender {
    sender: ConnectActionTransport,
}

#[uniffi::export]
impl ConnectSender {
    /// Selects the native session used by the generated action client.
    ///
    /// # Errors
    /// Returns a configuration error when the URL or size limits are invalid.
    #[uniffi::constructor]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "UniFFI passes owned strings across the native boundary."
    )]
    pub fn new(
        client: Arc<NativeHttpClient>,
        base_url: String,
        platform: NativePlatform,
        max_frame_bytes: u32,
        chunk_bytes: u32,
    ) -> Result<Arc<Self>, CoreError> {
        let max = max_frame_bytes as usize;
        let chunk_bytes = NonZeroUsize::new(chunk_bytes as usize)
            .ok_or_else(|| CoreError::write("Empty response chunk limit"))?;
        let size = max
            .checked_mul(2)
            .and_then(|n| n.checked_add(12))
            .filter(|_| max > 0)
            .ok_or_else(|| CoreError::write("Invalid frame limit"))?;
        let cfg = PlatformHttpConfig {
            request_bytes: size,
            chunk_bytes,
        };
        let network = PlatformHttpTransport::new(client).with_config(cfg);
        let opts = ClientOptions::new()
            .with_read_max_bytes(size)
            .with_send_max_bytes(size);
        let actions =
            ActionsClient::new(Arc::new(network), &base_url, opts).map_err(CoreError::write)?;
        let sender = ConnectActionTransport::new(
            actions,
            foundation_context::Ctx::host(),
            ClientCaps::native_current(platform.into()),
            WireLimits::default().max_frame_bytes(max),
        );
        Ok(Arc::new(Self { sender }))
    }

    /// Sends one action or negotiation request.
    ///
    /// # Errors
    /// Returns the transport or frame error.
    pub async fn exchange(&self, request: Vec<u8>) -> Result<Option<Vec<u8>>, CoreError> {
        self.sender.send(request).await.map_err(|err| match err {
            ManagerError::Wire(_) => CoreError::write(err),
            _ => CoreError::Transport {
                detail: err.to_string(),
            },
        })
    }
}

#[async_trait::async_trait]
impl HostTransport for ConnectSender {
    async fn send(&self, request: Vec<u8>) -> Result<Option<Vec<u8>>, CoreError> {
        self.exchange(request).await
    }
}

impl ConnectSender {
    /// Looks up a named service once and returns its action sender.
    ///
    /// # Errors
    /// Returns the native lookup error or the client configuration error.
    pub async fn from_provider(
        provider: &dyn HttpServiceProvider,
        service: String,
        base_url: String,
        platform: NativePlatform,
        max_frame_bytes: u32,
        chunk_bytes: u32,
    ) -> Result<Arc<Self>, ServiceError> {
        let network = provider.http_client(service).await?;
        Self::new(network, base_url, platform, max_frame_bytes, chunk_bytes).map_err(|err| {
            ServiceError::Unavailable {
                detail: err.to_string(),
            }
        })
    }
}
