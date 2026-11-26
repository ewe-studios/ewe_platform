//! Map host errors to public control messages.

use foundation_ui_wire::writer::{write_messages, ClientCaps, FrameHeader, WriterConfig};
use foundation_ui_wire::{
    ActionErrorT, ControlT, ErrorCode, MessageT, StateExpiredT, UpdateRequiredT, SCHEMA_VERSION,
};

use crate::action::{ActionError, InvokeError, LookupError};
use crate::state::StateError;

pub(super) enum Failure {
    Error(ActionError),
    Expired,
    UpdateRequired(u16),
}

impl Failure {
    pub(super) fn bad_request() -> Self {
        Self::Error(ActionError::new(ErrorCode::BadRequest, "bad request"))
    }

    pub(super) fn action_failed() -> Self {
        Self::Error(ActionError::new(ErrorCode::ActionFailed, "action failed"))
    }

    pub(super) fn encode(self, header: FrameHeader, config: &WriterConfig) -> Vec<u8> {
        let mut control = ControlT::default();
        match self {
            Self::Expired => control.state_expired = Some(Box::new(StateExpiredT::default())),
            Self::UpdateRequired(client_version) => {
                let mut update = UpdateRequiredT::default();
                update.client_schema_version = client_version;
                update.server_schema_version = SCHEMA_VERSION;
                control.update_required = Some(Box::new(update));
            }
            Self::Error(error) => {
                let mut wire = ActionErrorT::default();
                wire.code = error.code;
                if error.message.len() <= config.wire_limits().frame_bytes() {
                    wire.message = Some(error.message);
                }
                control.error = Some(Box::new(wire));
            }
        }
        let mut message = MessageT::default();
        message.control = Some(Box::new(control));
        // Control messages use only schema fields. They work before vocabulary negotiation.
        let caps = ClientCaps::web_current();
        if let Ok((bytes, _)) = write_messages(vec![message.clone()], &caps, header, config) {
            return bytes;
        }
        if let Some(error) = message
            .control
            .as_mut()
            .and_then(|control| control.error.as_mut())
        {
            error.message = None;
        }
        // This scalar-only protocol reply fits the default limit even when the configured limit is one byte.
        write_messages(vec![message], &caps, header, &WriterConfig::default())
            .expect("one scalar control message fits the default frame limit")
            .0
    }
}

impl From<LookupError> for Failure {
    fn from(error: LookupError) -> Self {
        let message = match error {
            LookupError::InvalidReference => "bad request",
            LookupError::UnavailableOrigin => "origin not available",
            LookupError::UnknownAction => "unknown action",
            LookupError::UnsupportedAction => "action not supported on this surface",
        };
        Self::Error(ActionError::new(error.code(), message))
    }
}

impl From<InvokeError> for Failure {
    fn from(error: InvokeError) -> Self {
        match error {
            InvokeError::State(StateError::Expired) => Self::Expired,
            InvokeError::State(error) => {
                tracing::debug!(?error, "State verification failed");
                match error {
                    StateError::BadTag
                    | StateError::UnknownKey
                    | StateError::Malformed { .. }
                    | StateError::BodyTooLarge { .. } => {
                        Self::Error(ActionError::new(ErrorCode::StateRejected, "state rejected"))
                    }
                    _ => Self::action_failed(),
                }
            }
            InvokeError::Action(trace) => {
                tracing::error!(?trace, "The action failed");
                Self::Error(trace.current_context().clone())
            }
        }
    }
}
