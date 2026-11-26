//! Select and finalize a view response for one request.
use crate::{html, http, Html};

use foundation_signals::MultiThreadedRuntime;
use foundation_ui_runtime::action::{encode_updates, Context};
use foundation_ui_runtime::view::{full_view, ViewIds};

use foundation_ui_wire::writer::{ClientCaps, FrameHeader, WriterConfig};
use foundation_ui_wire::{read_frame, OwnedFrame, StreamFrameWriter};
use serde::Serialize;
use std::any::{type_name, Any};
use std::fmt;
use std::io::{self, Write};
use std::sync::{Arc, OnceLock};

/// A failure to accept the request, run the handler, or encode its result.
#[derive(Debug)]
pub enum ViewError {
    BadRequest(String),
    PayloadTooLarge,
    UnsupportedVocabulary,
    MissingService(&'static str),
    Render(String),
    Write(io::Error),
}

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PayloadTooLarge => f.write_str("The request exceeds the configured limit"),
            Self::Write(error) => error.fmt(f),
            Self::BadRequest(message) | Self::Render(message) => f.write_str(message),
            Self::UnsupportedVocabulary => {
                f.write_str("The client must update its view vocabulary.")
            }
            Self::MissingService(name) => write!(f, "Missing application service {name}"),
        }
    }
}

impl std::error::Error for ViewError {}

/// The middleware context and the incoming request.
pub struct ViewRequest {
    session: ViewSession,
    pub request: http::SimpleIncomingRequest,
}

/// Output settings and services for one view request.
#[derive(Clone)]
pub struct ViewSession {
    ctx: http::Ctx,
    caps: ClientCaps,
    writer: WriterConfig,
    signals: Arc<OnceLock<RequestSignals>>,
}

struct RequestSignals {
    runtime: Arc<MultiThreadedRuntime>,
    scope: foundation_signals::shared::Scope,
}

impl ViewRequest {
    /// Reads the client hello or uses capabilities supplied by middleware.
    ///
    /// # Errors
    /// Returns an error for an invalid hello or an unsupported vocabulary.
    pub fn new(ctx: &http::Ctx, request: http::SimpleIncomingRequest) -> Result<Self, ViewError> {
        let writer = ctx.get_cloned::<WriterConfig>().unwrap_or_default();
        let caps = match ctx.get_cloned::<ClientCaps>() {
            Some(caps) => caps,
            None => client_caps(&request, &writer)?,
        };
        if ctx.get::<ViewIds>().is_none() {
            drop(ctx.get_or_insert(ViewIds::default()));
        }
        let ctx = ctx.fork();
        Ok(Self {
            session: ViewSession {
                ctx,
                caps,
                writer,
                signals: Arc::default(),
            },
            request,
        })
    }

    /// Returns the service installed by the host or its middleware.
    ///
    /// # Errors
    /// Returns the name of a missing service.
    pub fn require<T: Any + Send + Sync>(&self) -> Result<Arc<T>, ViewError> {
        self.session
            .ctx
            .get::<T>()
            .ok_or(ViewError::MissingService(type_name::<T>()))
    }

    /// Keeps the request settings until the response is ready.
    #[must_use]
    pub fn session(&self) -> ViewSession {
        self.session.clone()
    }

    /// Creates a receiver for targeted view operations.
    #[must_use]
    pub fn render_context(&self) -> Context {
        Context::for_render_on(self.session.ctx.clone(), self.session.caps.mount)
    }

    /// Returns the request's reactive scope. The body writer keeps the scope alive.
    #[must_use]
    pub fn signals(&self) -> &foundation_signals::shared::Scope {
        &self
            .session
            .signals
            .get_or_init(|| {
                let runtime = Arc::new(MultiThreadedRuntime::new());
                let scope = runtime.scope();
                RequestSignals { runtime, scope }
            })
            .scope
    }
}

fn client_caps(
    request: &http::SimpleIncomingRequest,
    writer: &WriterConfig,
) -> Result<ClientCaps, ViewError> {
    let Some(hello) = crate::action_route::hello_header(request, writer.wire_limits())? else {
        return Ok(ClientCaps::web_current());
    };
    foundation_ui_runtime::dispatch::Connection::from_verified_hello(hello.view(), 0)
        .map_err(|e| ViewError::BadRequest(e.to_string()))?
        .map(|connection| connection.caps().clone())
        .ok_or(ViewError::UnsupportedVocabulary)
}

/// A tree, a complete wire frame, or the queued operations from one render.
pub enum FrameBody {
    View(Html),
    Stateful(Html, Box<dyn ViewState>),
    Frame(foundation_ui_wire::FrameT),
    Updates(Context),
    Encoded(OwnedFrame),
    Stream(OwnedFrame),
}

/// A component state that the writer signs when it emits the tree.
pub trait ViewState: Send + Sync {
    /// Queues the complete tree with this state.
    ///
    /// # Errors
    /// Returns a rendering or serialization error.
    fn render(&self, ctx: &Context, tree: &Html) -> Result<(), ViewError>;
}
impl<S: Serialize + Send + Sync> ViewState for S {
    fn render(&self, ctx: &Context, tree: &Html) -> Result<(), ViewError> {
        full_view(ctx, ctx.receiver(), tree, Some(self))
            .map_err(|e| ViewError::Render(e.to_string()))
    }
}
impl<S: Serialize + Send + Sync + 'static> From<(Html, S)> for FrameBody {
    fn from((tree, state): (Html, S)) -> Self {
        Self::Stateful(tree, Box::new(state))
    }
}

impl From<OwnedFrame> for FrameBody {
    fn from(frame: OwnedFrame) -> Self {
        Self::Encoded(frame)
    }
}

impl From<Html> for FrameBody {
    fn from(html: Html) -> Self {
        Self::View(html)
    }
}
impl From<Context> for FrameBody {
    fn from(ctx: Context) -> Self {
        Self::Updates(ctx)
    }
}
impl From<foundation_ui_wire::FrameT> for FrameBody {
    fn from(frame: foundation_ui_wire::FrameT) -> Self {
        Self::Frame(frame)
    }
}

/// A typed JSON value that can write its own document.
pub trait JsonBody: Any + Send + Sync {
    fn value(&self) -> &dyn Any;
    /// Writes this value once.
    ///
    /// # Errors
    /// Returns the serializer or writer error.
    fn write_json(&self, writer: &mut dyn Write) -> Result<(), ViewError>;
}

impl<T: Serialize + Any + Send + Sync> JsonBody for T {
    fn value(&self) -> &dyn Any {
        self
    }
    fn write_json(&self, writer: &mut dyn Write) -> Result<(), ViewError> {
        serde_json::to_writer(writer, self).map_err(|error| {
            if error.is_io() {
                ViewError::Write(error.into())
            } else {
                ViewError::Render(error.to_string())
            }
        })
    }
}

/// The original payload selected by the handler.
pub enum Response {
    Frame(FrameBody),
    Html(Html),
    Json(Box<dyn JsonBody>),
}

impl Response {
    /// Selects one frame for the requesting mount.
    #[must_use]
    pub fn frame(body: impl Into<FrameBody>) -> Self {
        Self::Frame(body.into())
    }

    /// Selects an HTML document.
    #[must_use]
    pub fn html(html: Html) -> Self {
        Self::Html(html)
    }

    /// Keeps the original typed value until the writer requests the body.
    #[must_use]
    pub fn json(value: impl JsonBody) -> Self {
        Self::Json(Box::new(value))
    }

    #[must_use]
    pub fn media_type(&self) -> &'static str {
        match self {
            Self::Html(_) => "text/html; charset=utf-8",
            Self::Json(_) => "application/json",
            Self::Frame(FrameBody::Stream(_)) => "application/primal-frame",
            Self::Frame(_) => "application/vnd.ewe.frame",
        }
    }
}

/// The response and its request scope stay together until the write ends.
pub struct ViewBody {
    session: ViewSession,
    response: Response,
}

impl ViewSession {
    pub(crate) fn for_action(ctx: http::Ctx) -> Self {
        Self {
            ctx,
            caps: ClientCaps::web_current(),
            writer: WriterConfig::default(),
            signals: Arc::default(),
        }
    }
    fn full(&self, tree: &Html, state: Option<&dyn ViewState>) -> Result<Vec<u8>, ViewError> {
        if self.caps.vocabulary_version < 6 {
            return Err(ViewError::UnsupportedVocabulary);
        }
        let view = Context::for_render_on(self.ctx.clone(), self.caps.mount);
        if let Some(state) = state {
            state.render(&view, tree)?;
        } else {
            full_view(&view, view.receiver(), tree, None::<&()>)
                .map_err(|e| ViewError::Render(e.to_string()))?;
        }
        self.encode(view)
    }

    fn encode(&self, view: Context) -> Result<Vec<u8>, ViewError> {
        let header = FrameHeader {
            stream_id: 0,
            sequence_id: 0,
            ack: 0,
        };
        encode_updates(
            &self.ctx,
            view.into_receiver().drain(),
            &self.caps,
            header,
            &self.writer,
        )
        .map_err(|e| ViewError::Render(e.to_string()))
    }

    /// Retains the response and its request scope for a body writer.
    #[must_use]
    pub fn body(self, response: Response) -> ViewBody {
        ViewBody {
            session: self,
            response,
        }
    }
}

impl ViewBody {
    /// Borrows the original response.
    #[must_use]
    pub fn response(&self) -> &Response {
        &self.response
    }

    /// Runs pending signal work and writes the selected response to the sink.
    ///
    /// # Errors
    /// Returns an error from the tree, signer, serializer, or destination writer.
    pub fn write_body(self, out: &mut dyn Write) -> Result<(), ViewError> {
        let session = self.session;
        if let Some(signals) = session.signals.get() {
            signals.runtime.stabilize();
        }
        match self.response {
            Response::Html(tree) => {
                let tokens = session
                    .ctx
                    .get::<foundation_ui_wire::vocab::TokenTable>()
                    .unwrap_or_default();
                html::write(&tree, &tokens, out)
            }
            Response::Json(value) => value.write_json(out),
            Response::Frame(body) => {
                let bytes = match body {
                    FrameBody::Encoded(frame) => {
                        return StreamFrameWriter::raw(out)
                            .write_frame(&frame)
                            .map_err(ViewError::Write)
                    }
                    FrameBody::Stream(frame) => {
                        return StreamFrameWriter::length_prefixed(out)
                            .write_frame(&frame)
                            .map_err(ViewError::Write)
                    }
                    FrameBody::Frame(frame) => {
                        let bytes = foundation_ui_wire::finish_frame(&frame)
                            .map_err(|e| ViewError::Render(e.to_string()))?;
                        read_frame(&bytes, session.writer.wire_limits())
                            .map_err(|e| ViewError::Render(e.to_string()))?;
                        bytes
                    }
                    FrameBody::View(tree) => session.full(&tree, None)?,
                    FrameBody::Stateful(tree, state) => {
                        session.full(&tree, Some(state.as_ref()))?
                    }
                    FrameBody::Updates(view) => session.encode(view)?,
                };
                out.write_all(&bytes).map_err(ViewError::Write)
            }
        }
    }
}
/// Converts a handler return value into an explicit view response.
pub trait IntoViewResponse {
    /// # Errors
    /// Propagates errors from the handler.
    fn into_view_response(self) -> Result<Response, ViewError>;
}
impl IntoViewResponse for Response {
    fn into_view_response(self) -> Result<Response, ViewError> {
        Ok(self)
    }
}
impl<E: Into<Box<dyn std::error::Error + Send + Sync>>> IntoViewResponse for Result<Response, E> {
    fn into_view_response(self) -> Result<Response, ViewError> {
        self.map_err(|error| {
            let error = error.into();
            match error.downcast::<ViewError>() {
                Ok(error) => *error,
                Err(error) => ViewError::Render(error.to_string()),
            }
        })
    }
}
