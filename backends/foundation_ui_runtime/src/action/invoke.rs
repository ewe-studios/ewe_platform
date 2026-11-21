//! Call a selected action with the services and component root supplied by the host.

use std::fmt;

use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use foundation_ui_wire::{ActionRequestT, ErrorCode, MountSurface};

use super::{ActionError, Context, ErasedAction, RawInput, RawInteraction, ReceiverItem};
use crate::state::{verify_state, StateError};

/// The host can distinguish a failed signature check from a handler or decode error.
#[derive(Debug)]
pub enum InvokeError {
    State(StateError),
    Action(ErrorTrace<ActionError>),
}

impl fmt::Display for InvokeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State(error) => error.fmt(formatter),
            Self::Action(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for InvokeError {}

/// Verifies the request state, calls the selected handler and returns its pending output.
/// The caller authenticates the request and selects the entry before this call.
///
/// # Errors
///
/// Returns the verification error or the full action trace. A failed call discards its receiver.
pub async fn invoke_action(
    ctx: &Ctx,
    entry: &dyn ErasedAction,
    surface: MountSurface,
    state_root: Option<u32>,
    request: ActionRequestT,
) -> Result<Vec<ReceiverItem>, InvokeError> {
    let state_body = request
        .state
        .as_deref()
        .map(|signed| verify_state(ctx, signed))
        .transpose()
        .map_err(InvokeError::State)?;
    let input = request.input.unwrap_or_default();
    let event = RawInteraction {
        node_id: request.node_id,
        event_name: input.event_name.unwrap_or_default(),
        input_value: match (input.value, input.json) {
            (Some(value), None) => RawInput::Text(value),
            (None, Some(bytes)) => RawInput::Json(bytes),
            (None, None) => RawInput::Missing,
            (Some(_), Some(_)) => {
                return Err(InvokeError::Action(ErrorTrace::new(ActionError::new(
                    ErrorCode::InvalidInput,
                    "The event supplied two input encodings",
                ))))
            }
        },
        state_body,
    };
    let call = Context::for_action(ctx.clone(), surface, state_root);
    entry
        .call_raw(&call, event)
        .await
        .map_err(InvokeError::Action)?;
    Ok(call.into_receiver().drain())
}
