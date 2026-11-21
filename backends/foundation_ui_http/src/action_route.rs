//! Serve the negotiated action registry on the base path and its named aliases.
use crate::{http, FrameBody, Response, ViewError};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use foundation_ui_runtime::{
    action::{ActionError, ActionRef, ActionRegistry, Origin},
    dispatch::{Connection, Dispatcher, DispatcherBuilder, HelloOutcome},
};
use foundation_ui_wire::{ActionRequest, AsyncFrameReader, ErrorCode, OwnedFrame, WireLimits};
use std::sync::Arc;

/// A host policy that runs after frame validation and before the action.
pub trait ActionRequestPolicy: Send + Sync + 'static {
    /// # Errors
    /// Returns a protocol error when this request cannot run.
    fn check(&self, ctx: &http::Ctx, request: ActionRequest<'_>) -> Result<(), ActionError>;
}

/// A shared server dispatcher for action requests.
#[derive(Clone)]
pub struct ActionRegistryRouteHandler(Arc<Dispatcher>);

impl ActionRegistryRouteHandler {
    /// Reads an action request and returns a verified reply for the originating connection.
    ///
    /// # Errors
    /// Returns malformed input and negotiation errors before dispatch.
    pub async fn respond(
        &self,
        ctx: &http::Ctx,
        request: http::SimpleIncomingRequest,
    ) -> Result<Response, ViewError> {
        action(ctx, request, &self.0)
            .await
            .map(|frame| Response::Frame(FrameBody::Stream(frame)))
    }

    #[must_use]
    pub fn new(registry: ActionRegistry) -> Self {
        Self::from_dispatcher(DispatcherBuilder::new(Origin::Server, Arc::new(registry)).build())
    }
    #[must_use]
    pub fn from_dispatcher(dispatcher: Dispatcher) -> Self {
        Self(Arc::new(dispatcher))
    }
}

impl http::Route for ActionRegistryRouteHandler {
    const METHOD: http::SimpleMethod = http::SimpleMethod::POST;
    const PATH: &'static str = "/action";
    const ALIASES: &'static [&'static str] = &["/action/*"];
}

impl http::Serve for ActionRegistryRouteHandler {
    fn serve(
        &self,
        ctx: http::Ctx,
        req: http::SimpleIncomingRequest,
        conn: http::SharedByteBufferStream<http::RawStream>,
    ) -> http::ConnectionResult {
        let handler = self.clone();
        // Action negotiation must also accept an outdated hello and return UpdateRequired.
        let session = crate::response::ViewSession::for_action(ctx.clone());
        http::ConnectionResult::Async(Box::pin(async move {
            match handler.respond(&ctx, req).await {
                Ok(response) => crate::delivery::send_body(ctx, conn, session.body(response)).await,
                Err(error) => crate::delivery::error_response(&ctx, conn, error).await,
            }
        }))
    }
}

pub(crate) fn hello_header(
    request: &http::SimpleIncomingRequest,
    limits: &WireLimits,
) -> Result<Option<OwnedFrame>, ViewError> {
    let mut values = request
        .headers
        .iter()
        .filter(|(name, _)| name.to_string().eq_ignore_ascii_case("x-ewe-hello"))
        .flat_map(|(_, values)| values);
    let Some(value) = values.next() else {
        return Ok(None);
    };
    let invalid = || ViewError::BadRequest("Invalid client hello".into());
    if values.next().is_some()
        || value.len()
            > base64::encoded_len(
                limits.action_request_bytes().min(limits.frame_bytes()),
                false,
            )
            .unwrap_or(usize::MAX)
    {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(value).map_err(|_| invalid())?;
    OwnedFrame::new(bytes, limits)
        .map(Some)
        .map_err(|_| invalid())
}

async fn action(
    ctx: &http::Ctx,
    mut req: http::SimpleIncomingRequest,
    dispatcher: &Dispatcher,
) -> Result<OwnedFrame, ViewError> {
    let limits = dispatcher.wire_limits();
    let hello = hello_header(&req, limits)?;
    let input = req
        .take_async_body()
        .ok_or_else(|| ViewError::BadRequest("Missing action body".into()))?;
    let request_limits =
        (*limits).max_frame_bytes(limits.action_request_bytes().min(limits.frame_bytes()));
    let mut reader = AsyncFrameReader::raw(input, request_limits);
    let frame = reader
        .next_frame()
        .await
        .map_err(|e| match e {
            foundation_ui_wire::FramingError::Limit { .. } => ViewError::PayloadTooLarge,
            other => ViewError::BadRequest(other.to_string()),
        })?
        .ok_or_else(|| ViewError::BadRequest("Missing action frame".into()))?;
    let verified = frame.view();
    let request = verified
        .kind_as_frame_request()
        .ok_or_else(|| ViewError::BadRequest("Expected a request frame".into()))?;
    let path = req.request_url.url.split('?').next().unwrap_or("");
    let named = path
        .strip_prefix("/action/")
        .filter(|suffix| !suffix.is_empty());
    let negotiate = |hello| -> Result<Vec<u8>, ViewError> {
        match dispatcher
            .hello_verified(ctx, hello)
            .map_err(|e| ViewError::BadRequest(e.to_string()))?
        {
            HelloOutcome::Accepted { caps_frame, .. }
            | HelloOutcome::UpdateRequired { caps_frame } => Ok(caps_frame),
        }
    };
    let bytes = if request.hello().is_some() && request.action().is_none() {
        if named.is_some() {
            return Err(ViewError::BadRequest(
                "A named action path requires an action".into(),
            ));
        }
        negotiate(verified)?
    } else {
        let hello = hello.ok_or_else(|| ViewError::BadRequest("Missing client hello".into()))?;
        let Some(connection) = Connection::from_verified_hello(hello.view(), verified.stream_id())
            .map_err(|e| ViewError::BadRequest(e.to_string()))?
        else {
            return OwnedFrame::new(negotiate(hello.view())?, limits)
                .map_err(|e| ViewError::Render(e.to_string()));
        };
        let action = request
            .action()
            .ok_or_else(|| ViewError::BadRequest("Missing action".into()))?;
        let check = if named.is_some_and(|name| {
            action
                .action()
                .and_then(|v| ActionRef::parse(v).ok())
                .is_none_or(|r| r.name != name)
        }) {
            Err(ActionError::new(
                ErrorCode::BadRequest,
                "Action name differs from the route",
            ))
        } else if let Some(policy) = ctx.get::<Arc<dyn ActionRequestPolicy>>() {
            policy.check(ctx, action)
        } else {
            Ok(())
        };
        match check {
            Ok(()) => {
                dispatcher
                    .dispatch_verified(ctx, &connection, verified)
                    .await
            }
            Err(error) => dispatcher.reject(&connection, verified.sequence_id(), error),
        }
    };
    OwnedFrame::new(bytes, limits).map_err(|e| ViewError::Render(e.to_string()))
}
