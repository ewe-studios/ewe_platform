//! Validate the wire request before calling the registered handler.

use std::future::Future;

use foundation_context::Ctx;
use foundation_ui_wire::{
    read_frame, ActionRequestT, Frame, FrameRequest, WireLimits, SCHEMA_VERSION,
};

use super::{errors::Failure, Connection, Dispatcher};
use crate::action::{invoke_action, prepare_messages, ActionRef, ErasedAction, LookupError};

pub(super) struct Admission {
    ack: u64,
    selected: Result<(std::sync::Arc<dyn ErasedAction>, ActionRequestT), Failure>,
}

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
        self.finish(ctx, connection, self.admit(ctx, connection, bytes))
    }

    pub(super) fn admit(&self, ctx: &Ctx, connection: &Connection, bytes: &[u8]) -> Admission {
        match read_frame(bytes, self.wire_limits()) {
            Ok(frame) => self.admit_verified(ctx, connection, frame),
            Err(_) => Admission {
                ack: 0,
                selected: Err(Failure::bad_request()),
            },
        }
    }

    /// Returns the verification limits for this dispatcher.
    #[must_use]
    pub fn wire_limits(&self) -> &WireLimits {
        self.config.wire_limits()
    }

    fn admit_verified(&self, ctx: &Ctx, connection: &Connection, frame: Frame<'_>) -> Admission {
        let ack = frame.sequence_id();
        let selected = self.select(
            ctx,
            connection,
            ack,
            frame.schema_version(),
            frame.kind_as_frame_request(),
        );
        Admission { ack, selected }
    }

    /// Runs a frame that the caller verified with this dispatcher's wire limits.
    pub async fn dispatch_verified(
        &self,
        ctx: &Ctx,
        connection: &Connection,
        frame: Frame<'_>,
    ) -> Vec<u8> {
        self.finish(ctx, connection, self.admit_verified(ctx, connection, frame))
            .await
    }

    /// Returns a protocol error for a request rejected by the host.
    pub fn reject(
        &self,
        connection: &Connection,
        sequence: u64,
        error: crate::action::ActionError,
    ) -> Vec<u8> {
        Failure::Error(error).encode(connection.next_header(sequence), &self.config)
    }

    pub(super) async fn finish(
        &self,
        ctx: &Ctx,
        connection: &Connection,
        admission: Admission,
    ) -> Vec<u8> {
        let result = match admission.selected {
            Ok((entry, request)) => {
                let root = (self.target_lookup)(ctx, entry.name(), request.node_id);
                invoke_action(ctx, entry.as_ref(), connection.surface(), root, request)
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
        let header = connection.next_header(admission.ack);
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

    fn select(
        &self,
        ctx: &Ctx,
        connection: &Connection,
        sequence: u64,
        schema_version: u16,
        request: Option<FrameRequest<'_>>,
    ) -> Result<(std::sync::Arc<dyn ErasedAction>, ActionRequestT), Failure> {
        let action = request
            .filter(|request| request.hello().is_none() && request.resume().is_none())
            .and_then(|request| request.action())
            .ok_or_else(Failure::bad_request)?;
        if schema_version != SCHEMA_VERSION {
            return Err(Failure::UpdateRequired(schema_version));
        }
        if !connection.claim_request(sequence) {
            return Err(Failure::bad_request());
        }
        if action.input().and_then(|input| input.json()).is_some()
            && connection.caps().vocabulary_version
                < foundation_ui_wire::vocab::STRUCTURED_INPUT_VERSION
        {
            return Err(Failure::Error(crate::action::ActionError::new(
                foundation_ui_wire::ErrorCode::UnsupportedAction,
                "The peer did not negotiate structured action input",
            )));
        }
        let reference = ActionRef::parse(action.action().ok_or_else(Failure::bad_request)?)?;
        if reference.origin != self.accept {
            return Err(LookupError::UnavailableOrigin.into());
        }
        let entry = self
            .registry
            .handle(reference.origin, reference.name, connection.surface())?;
        if entry.requires_state() && action.state().is_none() {
            return Err(Failure::bad_request());
        }
        if self.accept == crate::action::Origin::Server {
            if let Some(policy) = ctx.get::<std::sync::Arc<dyn super::ActionRequestPolicy>>() {
                policy.check(ctx, action).map_err(Failure::Error)?;
            }
        }
        Ok((entry.clone(), action.unpack()))
    }
}
