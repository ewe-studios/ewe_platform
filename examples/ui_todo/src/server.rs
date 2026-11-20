//! Loopback HTTP host for the todo example.
use crate::{TodoStore, PAGE_ROOT};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use foundation_browser::test::{stream::StreamHandler, BacklogConfig, LiveChannel};
use foundation_context::Ctx;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::{
    native::{handlers::static_asset::StaticAssetHandler, serve::Serve},
    shared::{
        app::HttpApp,
        serve::{ConnectionResult, ServeOutcome},
    },
    ShutdownSignal,
};
use foundation_netio::{
    netcap::RawStream,
    shared::{
        client::body_reader::AsyncSendSafeBody,
        http::{
            Http11, RenderHttp, SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleMethod,
            SimpleOutgoingResponse, Status,
        },
    },
};
use foundation_ui_runtime::{
    action::{encode_updates, prepare_messages, Context, Origin},
    dispatch::{Connection, Dispatcher, DispatcherBuilder, HelloOutcome, RenderTarget},
    html_swap,
};
use foundation_ui_wire::{
    read_frame,
    writer::{ClientCaps, FrameHeader, WriterConfig},
    MountSurface, WireLimits,
};
use futures_lite::StreamExt;
use std::{net::TcpListener, path::Path, sync::Arc};

type Error = Box<dyn std::error::Error + Send + Sync>;
type Reply = (Status, &'static str, Vec<u8>);
const MIME: &str = "application/primal-frame";

fn encoded(ctx: &Ctx, render: Context) -> Result<Vec<u8>, Error> {
    encode_updates(
        ctx,
        render.into_receiver().drain(),
        &ClientCaps::web_current(),
        FrameHeader {
            stream_id: 0,
            sequence_id: 0,
            ack: 0,
        },
        &WriterConfig::default(),
    )
    .map_err(|e| format!("{e:?}").into())
}

fn http_frame(frame: &[u8]) -> Result<Reply, Error> {
    let size = u32::try_from(frame.len())?;
    let mut body = Vec::with_capacity(frame.len() + 4);
    body.extend_from_slice(&size.to_le_bytes());
    body.extend_from_slice(frame);
    Ok((Status::OK, MIME, body))
}

enum Endpoint {
    NativeInitial,
    Initial,
    Action,
}
impl Endpoint {
    async fn answer(&self, ctx: &Ctx, req: SimpleIncomingRequest) -> Result<Reply, Error> {
        let store = ctx.get::<TodoStore>().ok_or("Missing todo store")?;
        if matches!(self, Self::NativeInitial) {
            let view = Context::for_action(ctx.clone(), MountSurface::Native, Some(PAGE_ROOT));
            for root in [PAGE_ROOT, crate::DIALOG_ROOT] {
                let mut create = foundation_ui_wire::CreateNodeT::default();
                create.node_id = root;
                create.kind = foundation_ui_wire::NodeKind::STACK;
                let mut op = foundation_ui_wire::OperationT::default();
                op.ui = foundation_ui_wire::UiOperationT::CreateNode(Box::new(create));
                view.receiver().render_op(op);
                view.receiver()
                    .render_op(foundation_ui_wire::ops::append_child(1, root));
            }
            crate::app(&view, &store.snapshot()).map_err(|e| format!("{e:?}"))?;
            let bytes = encode_updates(
                ctx,
                view.into_receiver().drain(),
                &ClientCaps::native_current(foundation_ui_wire::Platform::Ios),
                FrameHeader {
                    stream_id: 0,
                    sequence_id: 0,
                    ack: 0,
                },
                &WriterConfig::default(),
            )
            .map_err(|e| format!("{e:?}"))?;
            return Ok((Status::OK, MIME, bytes));
        }
        if matches!(self, Self::Initial) {
            let view = Context::for_action(ctx.clone(), MountSurface::Web, Some(PAGE_ROOT));
            crate::web_app(&view, &store.snapshot()).map_err(|e| format!("{e:?}"))?;
            return Ok((Status::OK, MIME, encoded(ctx, view)?));
        }
        let dispatcher = ctx.get::<Dispatcher>().ok_or("Missing dispatcher")?;
        let limits = ctx.get::<WireLimits>().ok_or("Missing frame limits")?;
        let max = limits.action_request_bytes();
        let hello = req
            .headers
            .iter()
            .find_map(|(name, values)| {
                name.to_string()
                    .eq_ignore_ascii_case("x-ewe-hello")
                    .then_some(values)
            })
            .and_then(|v| v.first())
            .ok_or("Missing client hello")?;
        if hello.len() > max.saturating_mul(2) {
            return Err("Hello exceeds limit".into());
        }
        let hello = URL_SAFE_NO_PAD.decode(hello)?;
        let mut stream = AsyncSendSafeBody::from(req.body.ok_or("Missing request body")?);
        let mut body = Vec::new();
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| e.to_string())?;
            if bytes.len() > max.saturating_sub(body.len()) {
                return Err("Request exceeds limit".into());
            }
            body.extend_from_slice(&bytes);
        }
        let parsed = read_frame(&body, &limits)?;
        let handshake = dispatcher.hello(ctx, &hello)?;
        let (negotiated, caps) = match handshake {
            HelloOutcome::Accepted {
                connection,
                caps_frame,
            } => (connection, caps_frame),
            HelloOutcome::UpdateRequired { caps_frame } => return http_frame(&caps_frame),
        };
        let request = parsed.kind_as_frame_request().ok_or("Expected request")?;
        if request.hello().is_some() && request.action().is_none() {
            return http_frame(&caps);
        }
        let action = request.action().ok_or("Expected action")?;
        if action.state().is_some() {
            return Err("Business actions reject presentation state".into());
        }
        let connection = Connection::stateless(
            negotiated.caps().clone(),
            negotiated.surface(),
            parsed.stream_id(),
        );
        let reply = dispatcher.dispatch(ctx, &connection, &body).await;
        let response = read_frame(&reply, &limits)?;
        if response
            .kind_as_frame_message()
            .and_then(|m| m.messages())
            .is_some_and(|m| m.iter().all(|m| m.control().is_none()))
        {
            let count = store.snapshot().items.iter().filter(|i| !i.done).count();
            let update = Context::for_render(ctx.clone());
            count_update(&update, count).map_err(|e| format!("{e:?}"))?;
            ctx.get::<LiveChannel>()
                .ok_or("Missing live stream")?
                .sender()
                .push_messages(
                    prepare_messages(ctx, update.into_receiver().drain())
                        .map_err(|e| format!("{e:?}"))?,
                )?;
        }
        http_frame(&reply)
    }
}
fn count_update(ctx: &Context, count: usize) -> foundation_ui_runtime::action::ActionResult {
    let out = ctx.receiver();
    html_swap! {"#count-value",ctx,out,<span id="count-value">{count}</span>}
}
impl Serve for Endpoint {
    fn serve(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let endpoint = match self {
            Self::Initial => Self::Initial,
            Self::NativeInitial => Self::NativeInitial,
            Self::Action => Self::Action,
        };
        ConnectionResult::Async(Box::pin(async move {
            let (status, ctype, bytes) = endpoint.answer(&ctx, req).await.unwrap_or_else(|error| {
                eprintln!("Todo request failed: {error}");
                (
                    Status::BadRequest,
                    "text/plain",
                    error.to_string().into_bytes(),
                )
            });
            let response = SimpleOutgoingResponse::builder()
                .with_status(status)
                .add_header(SimpleHeader::CONTENT_TYPE, ctype)
                .with_body(SendSafeBody::Bytes(bytes))
                .build();
            if let Ok(response) = response {
                if Http11::response(response)
                    .http_render_to_writer(&mut conn)
                    .is_ok()
                {
                    return ServeOutcome::Keep;
                }
            }
            ServeOutcome::Close(None)
        }))
    }
}

/// Serves until the caller sets the shutdown signal.
///
/// # Errors
/// Returns an error for an invalid address, missing module, or invalid application setup.
pub fn serve(
    listener: &TcpListener,
    shutdown: &Arc<ShutdownSignal>,
    module: &Path,
) -> Result<(), Error> {
    serve_with_channel(
        listener,
        shutdown,
        module,
        LiveChannel::new(1, BacklogConfig::default(), WriterConfig::default()),
    )
}

/// Run the todo server with a shared count channel. The host can disconnect clients to test replay.
///
/// # Errors
/// Returns an error if the channel uses a different identifier or setup fails.
pub fn serve_with_channel(
    listener: &TcpListener,
    shutdown: &Arc<ShutdownSignal>,
    module: &Path,
    channel: LiveChannel,
) -> Result<(), Error> {
    if channel.stream_id() != 1 {
        return Err("The todo page listens to stream 1".into());
    }
    if !listener.local_addr()?.ip().is_loopback() {
        return Err("The single-user example requires a loopback address".into());
    }
    let mut app = HttpApp::new_serve();
    let ctx = app.context();
    crate::install_view_services(ctx, 100, (1 << 31) - 1)?;
    ctx.store(RenderTarget(PAGE_ROOT));
    ctx.store(TodoStore::default());
    ctx.store(WireLimits::default());
    ctx.store(channel);
    ctx.store(DispatcherBuilder::new(Origin::Server, Arc::new(crate::server_registry()?)).build());
    let module = std::fs::read(module)?;
    let initial: Arc<dyn Serve> = Arc::new(Endpoint::Initial);
    let action: Arc<dyn Serve> = Arc::new(Endpoint::Action);
    app.router.add_route_any("/initial", &initial);
    let native: Arc<dyn Serve> = Arc::new(Endpoint::NativeInitial);
    app.router.add_route_any("/native-initial", &native);
    app.router.add_route(SimpleMethod::POST, "/action", action);
    app.route_any::<StreamHandler>("/live");
    for (url, kind, bytes) in [
        (
            "/",
            "text/html; charset=utf-8",
            include_bytes!("../web/index.html").as_slice(),
        ),
        (
            "/app.js",
            "text/javascript",
            include_bytes!("../web/app.js").as_slice(),
        ),
        ("/module.wasm", "application/wasm", module.as_slice()),
        (
            "/foundation-wasm.js",
            "text/javascript",
            include_bytes!("../../../backends/foundation_wasm/runtime/foundation-wasm.js")
                .as_slice(),
        ),
        (
            "/foundation-wasm-ui.js",
            "text/javascript",
            include_bytes!("../../../backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js")
                .as_slice(),
        ),
        (
            "/foundation-ui-actions.js",
            "text/javascript",
            include_bytes!(
                "../../../backends/foundation_wasm_ui/runtimes/foundation-ui-actions.js"
            )
            .as_slice(),
        ),
        (
            "/foundation-ui-frames.js",
            "text/javascript",
            include_bytes!("../../../backends/foundation_wasm_ui/runtimes/foundation-ui-frames.js")
                .as_slice(),
        ),
        (
            "/foundation-ui-live.js",
            "text/javascript",
            include_bytes!("../../../backends/foundation_wasm_ui/runtimes/foundation-ui-live.js")
                .as_slice(),
        ),
        (
            "/foundation-ui-behaviours.js",
            "text/javascript",
            include_bytes!(
                "../../../backends/foundation_wasm_ui/runtimes/foundation-ui-behaviours.js"
            )
            .as_slice(),
        ),
        (
            "/foundation-ui-vocab.js",
            "text/javascript",
            include_bytes!("../../../backends/foundation_wasm_ui/runtimes/foundation-ui-vocab.js")
                .as_slice(),
        ),
        (
            "/foundation-ui-wire.js",
            "text/javascript",
            include_bytes!("../../../backends/foundation_wasm_ui/runtimes/foundation-ui-wire.js")
                .as_slice(),
        ),
    ] {
        let asset: Arc<dyn Serve> = Arc::new(StaticAssetHandler::new(bytes, kind));
        app.router.add_route_any(url, &asset);
    }
    app.server(&listener.local_addr()?.to_string())
        .serve_with_listener(listener, shutdown);
    Ok(())
}
