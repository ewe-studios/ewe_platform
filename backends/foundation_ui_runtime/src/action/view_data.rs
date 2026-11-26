//! Route typed MessagePack data to the receiving view.
use foundation_errstacks::ErrorTrace;
use foundation_ui_wire::ErrorCode;
use serde::{de::DeserializeOwned, Serialize};

use super::{ActionError, ActionResult, Context, RawInput};

/// A typed message for one application topic.
pub trait ViewData: Serialize + DeserializeOwned + 'static {
    /// The wire name shared by the sender and receiver.
    const TOPIC: &'static str;

    /// Send the value to the current view.
    ///
    /// # Errors
    /// Return an action error when the encoder rejects the value.
    fn send(&self, ctx: &Context) -> ActionResult {
        let bytes = rmp_serde::to_vec_named(&(Self::TOPIC, self)).map_err(|error| {
            ErrorTrace::new(ActionError::new(
                ErrorCode::ActionFailed,
                "Cannot encode view data",
            ))
            .attach(error)
        })?;
        ctx.receiver().data_in_view(bytes);
        Ok(())
    }

    /// Decode this topic from a host data event. Other message types return no value.
    ///
    /// # Errors
    /// Return an input error for invalid event bytes or an invalid value on this topic.
    fn decode_input(input: RawInput) -> Result<Option<Self>, ErrorTrace<ActionError>> {
        let bytes = input.decode::<Vec<u8>>()?;
        let mut message = bytes.as_slice();
        if rmp::decode::read_array_len(&mut message).ok() != Some(2) {
            return Ok(None);
        }
        let Ok((topic, mut payload)) = rmp::decode::read_str_from_slice(&message) else {
            return Ok(None);
        };
        if topic != Self::TOPIC {
            return Ok(None);
        }
        let value = rmp_serde::from_read(&mut payload).map_err(|error| {
            ErrorTrace::new(ActionError::new(
                ErrorCode::InvalidInput,
                "Invalid view data",
            ))
            .attach(error)
        })?;
        if !payload.is_empty() {
            return Err(ErrorTrace::new(ActionError::new(
                ErrorCode::InvalidInput,
                "Unexpected bytes after view data",
            )));
        }
        Ok(Some(value))
    }
}
