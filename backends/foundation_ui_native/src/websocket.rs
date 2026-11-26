//! Independent WebSocket connections for native host actions.
use crate::{CoreError, NativeCore};
use foundation_context::Ctx;
use std::sync::Arc;

/// One application message preserves its text or binary type.
#[derive(uniffi::Enum)]
pub enum SocketMessage {
    Text { value: String },
    Binary { value: Vec<u8> },
}

/// The host supplies one connection for each successful open.
#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait HostWebSocket: Send + Sync {
    /// Send one complete application message.
    ///
    /// # Errors
    /// Returns a transport error if the socket cannot accept the message.
    async fn send_message(&self, message: SocketMessage) -> Result<(), CoreError>;

    /// Read the next message. A normal peer close returns None.
    ///
    /// # Errors
    /// Returns a transport error if the receive fails.
    async fn receive_message(&self) -> Result<Option<SocketMessage>, CoreError>;

    /// Release this connection and wake pending readers. Repeated calls are safe.
    fn close_socket(&self);
}

/// A host service opens sockets outside the action dispatcher.
#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait HostWebSockets: Send + Sync {
    /// Open a dedicated connection to the given endpoint.
    ///
    /// # Errors
    /// Returns the connection or handshake error.
    async fn open_socket(&self, endpoint: String) -> Result<Arc<AppWebSocket>, CoreError>;
}

/// An application owns this socket until it closes or drops the handle.
#[derive(uniffi::Object)]
pub struct AppWebSocket(Arc<dyn HostWebSocket>);

impl AppWebSocket {
    /// Open a socket with the service from this host context.
    ///
    /// # Errors
    /// Returns an error for a missing service or a failed connection.
    pub async fn open(ctx: &Ctx, endpoint: String) -> Result<Arc<Self>, CoreError> {
        let service = ctx
            .get::<Arc<dyn HostWebSockets>>()
            .ok_or_else(|| CoreError::Transport {
                detail: "The host has no WebSocket service".into(),
            })?;
        service.open_socket(endpoint).await
    }
}

#[uniffi::export]
impl AppWebSocket {
    /// Wrap one connection from the platform.
    #[uniffi::constructor]
    pub fn new(socket: Arc<dyn HostWebSocket>) -> Arc<Self> {
        Arc::new(Self(socket))
    }

    /// Send one message through this connection.
    ///
    /// # Errors
    /// Returns the host send error.
    pub async fn send(&self, message: SocketMessage) -> Result<(), CoreError> {
        self.0.send_message(message).await
    }

    /// Receive one message or the end of the connection.
    ///
    /// # Errors
    /// Returns the host receive error.
    pub async fn receive(&self) -> Result<Option<SocketMessage>, CoreError> {
        self.0.receive_message().await
    }

    /// Close this connection.
    pub fn close_socket(&self) {
        self.0.close_socket();
    }
}

impl Drop for AppWebSocket {
    fn drop(&mut self) {
        self.close_socket();
    }
}

#[uniffi::export]
impl NativeCore {
    /// Install the platform service before the first action.
    pub fn install_web_sockets(&self, service: Arc<dyn HostWebSockets>) {
        self.graphics_ctx.store(service);
    }

    /// Open an application connection from the core context.
    ///
    /// # Errors
    /// Returns the service lookup or connection error.
    pub async fn open_web_socket(&self, endpoint: String) -> Result<Arc<AppWebSocket>, CoreError> {
        AppWebSocket::open(&self.graphics_ctx, endpoint).await
    }
}
