//! Validate the wire request before calling the registered handler.

use std::future::Future;

use foundation_context::Ctx;
use foundation_ui_wire::{read_frame, ActionRequestT, FrameRequest, SCHEMA_VERSION};

use super::{errors::Failure, Connection, Dispatcher};
use crate::action::{invoke_action, prepare_messages, ActionRef, ErasedAction, LookupError};

impl Dispatcher {
    /// Runs one action and returns one message frame with its acknowledgement.
    pub async fn dispatch(&self, ctx: &Ctx, connection: &Connection, request: &[u8]) -> Vec<u8> {
        self.submit(ctx, connection, request).await
    }

    /// Claims the request sequence before the transport returns a future to its caller.
    pub(super) fn submit<'a>(
        &'a self,
        ctx: &'a Ctx,
        connection: &'a Connection,
        bytes: &[u8],
    ) -> impl Future<Output = Vec<u8>> + 'a {
        let mut ack = 0;
        let selected = read_frame(bytes, self.config.wire_limits())
            .map_err(|_| Failure::bad_request())
            .and_then(|frame| {
                ack = frame.sequence_id();
                self.select(
                    connection,
                    ack,
                    frame.schema_version(),
                    frame.kind_as_frame_request(),
                )
            });
        async move {
            let result = match selected {
                Ok((entry, request)) => {
                    let root = (self.target_lookup)(ctx, entry.name(), request.node_id);
                    invoke_action(ctx, entry, connection.surface(), root, request)
                        .await
                        .map_err(Failure::from)
                        .and_then(|items| {
                            prepare_messages(ctx, items).map_err(|error| {
                                tracing::error!(?error, "Could not prepare the action reply");
                                Failure::action_failed()
                            })
                        })
                }
                Err(error) => Err(error),
            };
            let header = connection.next_header(ack);
            let delivered = result.and_then(|messages| {
                self.delivery
                    .messages(messages, connection.caps(), header)
                    .map_err(|error| {
                        tracing::error!(?error, "Could not encode the action reply");
                        Failure::action_failed()
                    })
            });
            delivered.unwrap_or_else(|failure| failure.encode(header, &self.config))
        }
    }

    fn select(
        &self,
        connection: &Connection,
        sequence: u64,
        schema_version: u16,
        request: Option<FrameRequest<'_>>,
    ) -> Result<(&dyn ErasedAction, ActionRequestT), Failure> {
        let action = request
            .filter(|request| request.hello().is_none())
            .and_then(|request| request.action())
            .ok_or_else(Failure::bad_request)?;
        if schema_version != SCHEMA_VERSION {
            return Err(Failure::UpdateRequired(schema_version));
        }
        if !connection.claim_request(sequence) {
            return Err(Failure::bad_request());
        }
        let reference = ActionRef::parse(action.action().ok_or_else(Failure::bad_request)?)?;
        if reference.origin != self.accept {
            return Err(LookupError::UnavailableOrigin.into());
        }
        let entry = self
            .registry
            .get(reference.origin, reference.name, connection.surface())?;
        if entry.requires_state() && action.state().is_none() {
            return Err(Failure::bad_request());
        }
        Ok((entry, action.unpack()))
    }
}
