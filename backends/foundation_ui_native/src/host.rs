use foundation_ui_runtime::action::{
    ActionTransport, LookupError, ManagerError, TransportFuture, TriggerEvent,
};
use foundation_ui_wire::ActionInputT;
use std::sync::Arc;

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct EventInput {
    pub node_id: u32,
    pub event_name: String,
    pub value: String,
    pub checked: bool,
    pub parent_node: u32,
    pub state: Vec<u8>,
}

impl From<EventInput> for TriggerEvent {
    fn from(event: EventInput) -> Self {
        let mut input = ActionInputT::default();
        input.event_name = Some(event.event_name);
        input.value = Some(event.value);
        input.checked = event.checked;
        input.parent_node = event.parent_node;
        Self {
            node_id: event.node_id,
            input,
            state: event.state,
        }
    }
}

#[derive(uniffi::Error, Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoreError {
    #[error("Invalid action reference: {detail}")]
    BadReference { detail: String },
    #[error("Unknown action: {reference}")]
    UnknownAction { reference: String },
    #[error("Unsupported action: {reference}")]
    UnsupportedAction { reference: String },
    #[error("Unavailable origin: {origin}")]
    UnavailableOrigin { origin: String },
    #[error("{detail}")]
    Transport { detail: String },
    #[error("No frame listener")]
    NoListener,
    #[error("{detail}")]
    Write { detail: String },
}

impl CoreError {
    pub(crate) fn write(error: impl std::fmt::Display) -> Self {
        Self::Write {
            detail: error.to_string(),
        }
    }
    pub(crate) fn manager(error: ManagerError, reference: &str) -> Self {
        match error {
            ManagerError::Lookup(LookupError::InvalidReference) => Self::BadReference {
                detail: reference.into(),
            },
            ManagerError::Lookup(LookupError::UnknownAction) => Self::UnknownAction {
                reference: reference.into(),
            },
            ManagerError::Lookup(LookupError::UnsupportedAction) => Self::UnsupportedAction {
                reference: reference.into(),
            },
            ManagerError::Lookup(LookupError::UnavailableOrigin) => Self::UnavailableOrigin {
                origin: reference.split(':').next().unwrap_or(reference).into(),
            },
            ManagerError::Wire(_) | ManagerError::SequenceExhausted => Self::write(error),
            ManagerError::Transport(detail) => Self::Transport { detail },
            ManagerError::SubmissionPoisoned => Self::Transport {
                detail: error.to_string(),
            },
        }
    }
}

#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait HostTransport: Send + Sync {
    /// Sends one action request. Returns an optional immediate reply.
    ///
    /// # Errors
    /// Returns a transport error if the send or reply fails.
    async fn send(&self, request: Vec<u8>) -> Result<Option<Vec<u8>>, CoreError>;
}
#[uniffi::export(with_foreign)]
pub trait FrameListener: Send + Sync {
    fn on_frame(&self, frame: Vec<u8>);
}
/// Reports a failure to obtain the native sender for a named service.
#[derive(uniffi::Error, thiserror::Error, Debug)]
pub enum ServiceError {
    #[error("Unknown service {name}")]
    UnknownService { name: String },
    #[error("Cannot open service: {detail}")]
    Unavailable { detail: String },
}

/// Keeps a native action sender alive across the language boundary.
#[derive(uniffi::Object)]
pub struct NativeSender {
    pub(crate) transport: Arc<dyn HostTransport>,
}

#[uniffi::export]
impl NativeSender {
    #[uniffi::constructor]
    #[must_use]
    pub fn new(transport: Arc<dyn HostTransport>) -> Arc<Self> {
        Arc::new(Self { transport })
    }
}

/// Selects a native sender when the application sets up a screen.
#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait HostTransportProvider: Send + Sync {
    /// Returns the configured sender for this service.
    ///
    /// # Errors
    /// Returns a service lookup error.
    async fn action_transport(&self, service: String) -> Result<Arc<NativeSender>, ServiceError>;
}

/// Adapts a native sender for use with the shared Rust network client.
#[must_use]
pub fn action_sender(transport: Arc<dyn HostTransport>) -> Arc<dyn ActionTransport + Send + Sync> {
    Arc::new(Adapter(transport))
}

pub(crate) struct Adapter(pub Arc<dyn HostTransport>);
impl ActionTransport for Adapter {
    fn send(&self, frame: Vec<u8>) -> TransportFuture<'_> {
        Box::pin(async move {
            self.0
                .send(frame)
                .await
                .map_err(|error| ManagerError::Transport(error.to_string()))
        })
    }
}
