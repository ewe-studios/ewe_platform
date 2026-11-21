//! Select one transport and send the event from the control that owns it.
use std::collections::BTreeSet;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use foundation_ui_wire::{
    finish_frame, ActionInputT, ActionRequestT, CapsStatus, FrameKindT, FrameRequestT, FrameT,
    ServerCapsT, WireError, WireLimits, SCHEMA_VERSION,
};

use super::{ActionRef, ActionRegistry, LookupError, Origin, Surface};

/// The renderer reads these values from the control that triggered the action.
#[derive(Debug)]
pub struct TriggerEvent {
    pub node_id: u32,
    pub input: ActionInputT,
    /// Copy the control's signed bytes. An empty vector means that the control has no state.
    pub state: Vec<u8>,
}

/// Some transports return one reply. Others deliver the reply on a separate receive path.
#[cfg(not(target_family = "wasm"))]
pub type TransportFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, ManagerError>> + Send + 'a>>;

#[cfg(target_family = "wasm")]
pub type TransportFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, ManagerError>> + 'a>>;

/// An adapter for the selected runtime.
pub trait ActionTransport {
    /// Submit requests in call order. The returned future waits for a reply or a socket handoff.
    /// Queue ordered streams here, before the future can yield. Return promptly.
    fn send(&self, frame: Vec<u8>) -> TransportFuture<'_>;
}

#[cfg(not(target_family = "wasm"))]
pub type Transport = dyn ActionTransport + Send + Sync;
#[cfg(target_family = "wasm")]
pub type Transport = dyn ActionTransport;

/// Reports a routing, encoding or transport failure to the caller.
#[derive(Debug)]
pub enum ManagerError {
    Lookup(LookupError),
    Wire(WireError),
    SequenceExhausted,
    Transport(String),
    SubmissionPoisoned,
    InvalidInput(String),
    UnsupportedInput,
}

impl fmt::Display for ManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lookup(error) => fmt::Display::fmt(error, f),
            Self::Wire(error) => fmt::Display::fmt(error, f),
            Self::SequenceExhausted => f.write_str("The stream used its last request number."),
            Self::Transport(detail) => write!(f, "Action transport failed. {detail}"),
            Self::InvalidInput(detail) => write!(f, "Invalid action input. {detail}"),
            Self::UnsupportedInput => {
                f.write_str("The peer did not negotiate structured action input.")
            }
            Self::SubmissionPoisoned => {
                f.write_str("An earlier request panicked during submission.")
            }
        }
    }
}

impl std::error::Error for ManagerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lookup(error) => Some(error),
            Self::Wire(error) => Some(error),
            _ => None,
        }
    }
}

impl From<LookupError> for ManagerError {
    fn from(error: LookupError) -> Self {
        Self::Lookup(error)
    }
}

struct ServerActions {
    names: BTreeSet<String>,
    structured: bool,
}

/// Shares the action declarations and assigns request numbers for one stream.
pub struct ActionManager {
    registry: Arc<ActionRegistry>,
    surface: Surface,
    stream_id: u64,
    last_sequence: Mutex<u64>,
    limits: WireLimits,
    server_actions: RwLock<Option<ServerActions>>,
    host: Option<Arc<Transport>>,
    server: Option<Arc<Transport>>,
}

impl ActionManager {
    #[must_use]
    pub fn new(
        registry: Arc<ActionRegistry>,
        surface: Surface,
        stream_id: u64,
        limits: WireLimits,
    ) -> Self {
        Self {
            registry,
            surface,
            stream_id,
            limits,
            last_sequence: Mutex::new(0),
            server_actions: RwLock::new(None),
            host: None,
            server: None,
        }
    }

    #[must_use]
    pub fn with_host(mut self, host: Arc<Transport>) -> Self {
        self.host = Some(host);
        self
    }

    #[must_use]
    pub fn with_server(mut self, server: Arc<Transport>) -> Self {
        self.server = Some(server);
        self
    }

    /// Replace the names advertised by the server for this surface.
    pub fn apply_caps(&self, caps: &ServerCapsT) {
        let mut names = BTreeSet::new();
        if caps.status == CapsStatus::Ok {
            names.extend(
                caps.actions
                    .iter()
                    .flatten()
                    .filter_map(|action| action.name.clone()),
            );
        }
        *self
            .server_actions
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Some(ServerActions {
            names,
            structured: caps.status == CapsStatus::Ok
                && caps.vocabulary_version >= foundation_ui_wire::vocab::STRUCTURED_INPUT_VERSION,
        });
    }

    fn route(&self, reference: &ActionRef<'_>) -> Result<(Origin, &Transport, bool), LookupError> {
        let advertised = self
            .server_actions
            .read()
            .unwrap_or_else(PoisonError::into_inner);
        let available = advertised
            .as_ref()
            .map(|caps| caps.names.contains(reference.name));
        let declaration = self
            .registry
            .lookup(reference.origin, reference.name, self.surface);
        let origin = match (reference.origin, declaration) {
            (Origin::Host, Ok(entry)) => {
                if entry.local {
                    Origin::Host
                } else if entry.origin.is_none() && available == Some(true) {
                    Origin::Server
                } else {
                    return Err(LookupError::UnavailableOrigin);
                }
            }
            (Origin::Server, Ok(_)) if available != Some(false) => Origin::Server,
            (Origin::Server, Ok(_)) => return Err(LookupError::UnavailableOrigin),
            (Origin::Server, Err(LookupError::UnknownAction)) if available == Some(true) => {
                Origin::Server
            }
            (_, Err(error)) => return Err(error),
        };
        let transport = match origin {
            Origin::Host => self.host.as_deref(),
            Origin::Server => self.server.as_deref(),
        };
        let structured =
            origin == Origin::Host || advertised.as_ref().is_some_and(|caps| caps.structured);
        Ok((
            origin,
            transport.ok_or(LookupError::UnavailableOrigin)?,
            structured,
        ))
    }

    /// Encode a structured value for an action. Text events use `trigger`.
    ///
    /// # Errors
    /// Returns an encoding, routing or transport error.
    pub async fn trigger_structured<E: serde::Serialize>(
        &self,
        reference: &str,
        mut event: TriggerEvent,
        value: &E,
    ) -> Result<Option<Vec<u8>>, ManagerError> {
        event.input.value = None;
        event.input.json =
            Some(serde_json::to_vec(value).map_err(|e| ManagerError::InvalidInput(e.to_string()))?);
        self.trigger(reference, event).await
    }

    /// Send one request. A socket can accept it before a reply exists.
    ///
    /// # Errors
    ///
    /// Returns a lookup error before submission for an invalid route.
    /// Reports encoding, sequence and transport failures separately.
    pub async fn trigger(
        &self,
        reference: &str,
        event: TriggerEvent,
    ) -> Result<Option<Vec<u8>>, ManagerError> {
        let reference = ActionRef::parse(reference)?;
        let (origin, transport, structured) = self.route(&reference)?;
        if event.input.json.is_some() {
            if event.input.value.is_some() {
                return Err(ManagerError::InvalidInput(
                    "The event supplied two input encodings".into(),
                ));
            }
            if !structured {
                return Err(ManagerError::UnsupportedInput);
            }
        }
        let prefix = match origin {
            Origin::Host => "host:",
            Origin::Server => "server:",
        };
        let limit = self
            .limits
            .frame_bytes()
            .min(self.limits.action_request_bytes());
        let content_len = [
            prefix.len(),
            reference.name.len(),
            event.state.len(),
            event.input.event_name.as_deref().unwrap_or_default().len(),
            event.input.value.as_deref().unwrap_or_default().len(),
            event.input.json.as_deref().unwrap_or_default().len(),
        ]
        .into_iter()
        .fold(0usize, usize::saturating_add);
        check_size(content_len, limit)?;

        let mut action = ActionRequestT::default();
        action.action = Some(format!("{prefix}{}", reference.name));
        action.input = Some(Box::new(event.input));
        if !event.state.is_empty() {
            action.state = Some(event.state);
        }
        action.node_id = event.node_id;
        let mut request = FrameRequestT::default();
        request.action = Some(Box::new(action));
        let mut frame = FrameT::default();
        frame.schema_version = SCHEMA_VERSION;
        frame.stream_id = self.stream_id;
        frame.kind = FrameKindT::FrameRequest(Box::new(request));

        let reply = {
            let mut last = self
                .last_sequence
                .lock()
                .map_err(|_| ManagerError::SubmissionPoisoned)?;
            frame.sequence_id = last.checked_add(1).ok_or(ManagerError::SequenceExhausted)?;
            let bytes = finish_frame(&frame).map_err(ManagerError::Wire)?;
            check_size(bytes.len(), limit)?;
            *last = frame.sequence_id;
            transport.send(bytes)
        };
        reply.await
    }
}

fn check_size(actual: usize, limit: usize) -> Result<(), ManagerError> {
    if actual > limit {
        return Err(ManagerError::Wire(WireError::TooLarge { actual, limit }));
    }
    Ok(())
}
