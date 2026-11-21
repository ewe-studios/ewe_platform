//! Send Foundation UI frames through the Connect Actions service.
pub use foundation_connectrpc::shared::transport::platform::{
    PlatformExchange, PlatformHttpClient, PlatformHttpConfig, PlatformHttpTransport,
};
#[cfg(all(feature = "browser", target_family = "wasm"))]
pub mod browser;
use foundation_connectrpc::{Request, RpcCtx};
use foundation_ui_runtime::{
    action::{ActionTransport, ManagerError, TransportFuture},
    network::server_capabilities,
};
use foundation_ui_wire::{read_frame, writer::ClientCaps, WireLimits};
#[cfg(not(target_family = "wasm"))]
use std::sync::Arc;

/// Generated messages, service registration and client.
#[allow(
    clippy::all,
    clippy::pedantic,
    missing_docs,
    unused_qualifications,
    rustdoc::all
)]
pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/ewe.ui.v1.mod.rs"));
    include!(concat!(env!("OUT_DIR"), "/action_service.rs"));
}

/// Sends one complete request to the generated action service.
pub struct ConnectActionTransport {
    client: v1::ActionsClient,
    ctx: foundation_context::Ctx,
    caps: ClientCaps,
    limits: WireLimits,
    hello: std::sync::RwLock<Option<Vec<u8>>>,
}

impl ConnectActionTransport {
    #[must_use]
    pub fn new(
        client: v1::ActionsClient,
        ctx: foundation_context::Ctx,
        caps: ClientCaps,
        limits: WireLimits,
    ) -> Self {
        Self {
            client,
            ctx,
            caps,
            limits,
            hello: std::sync::RwLock::default(),
        }
    }
}

impl ActionTransport for ConnectActionTransport {
    fn send(&self, frame: Vec<u8>) -> TransportFuture<'_> {
        Box::pin(async move {
            let parsed = read_frame(&frame, &self.limits).map_err(ManagerError::Wire)?;
            let request = parsed
                .kind_as_frame_request()
                .ok_or_else(|| ManagerError::Transport("Expected a UI request".into()))?;
            let hello = request.hello().is_some();
            if hello == request.action().is_some() {
                return Err(ManagerError::Transport(
                    "Expected exactly one request body".into(),
                ));
            }
            let previous = if hello {
                None
            } else {
                Some(
                    self.hello
                        .read()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone()
                        .ok_or_else(|| {
                            ManagerError::Transport(
                                "Connect actions require a negotiated hello".into(),
                            )
                        })?,
                )
            };
            let offered = hello.then(|| frame.clone());
            let packet = v1::UiRequest {
                frame,
                hello: previous,
                ..Default::default()
            };
            let reply = self
                .client
                .exchange(RpcCtx::client(self.ctx.clone()), Request::new(packet))
                .await
                .map_err(|err| ManagerError::Transport(err.to_string()))?
                .msg
                .frame;
            match reply {
                Some(bytes) => {
                    if let Some(offered) = offered {
                        let caps = server_capabilities(&bytes, &self.limits, self.caps.mount)?;
                        *self
                            .hello
                            .write()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) =
                            (caps.status == foundation_ui_wire::CapsStatus::Ok).then_some(offered);
                    } else {
                        foundation_ui_runtime::network::reply_message(
                            &bytes,
                            &self.limits,
                            self.caps.mount,
                        )?;
                    }
                    Ok(Some(bytes))
                }
                None if hello => Err(ManagerError::Transport(
                    "Missing server capabilities".into(),
                )),
                None => Ok(None),
            }
        })
    }
}

/// Dispatches verified requests with the application context for the current call.
#[cfg(not(target_family = "wasm"))]
pub struct ActionService {
    dispatcher: Arc<foundation_ui_runtime::dispatch::Dispatcher>,
}

#[cfg(not(target_family = "wasm"))]
impl ActionService {
    #[must_use]
    pub fn new(dispatcher: Arc<foundation_ui_runtime::dispatch::Dispatcher>) -> Self {
        Self { dispatcher }
    }
}

#[cfg(not(target_family = "wasm"))]
impl v1::Actions for ActionService {
    async fn exchange(
        &self,
        ctx: RpcCtx,
        request: Request<v1::UiRequest>,
    ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<v1::UiReply>> {
        use foundation_connectrpc::{ConnectError, Response};
        use foundation_ui_runtime::dispatch::{Connection, HelloOutcome};
        let invalid = |err: String| ConnectError::invalid_argument(err);
        let limits = self.dispatcher.wire_limits();
        let frame =
            read_frame(&request.msg.frame, limits).map_err(|err| invalid(err.to_string()))?;
        let body = frame
            .kind_as_frame_request()
            .ok_or_else(|| invalid("Expected a request frame".into()))?;
        if body.hello().is_some() == body.action().is_some() {
            return Err(invalid("Expected exactly one request body".into()).into());
        }
        let reply = if body.hello().is_some() {
            if request.msg.hello.is_some() {
                return Err(invalid("Duplicate client hello".into()).into());
            }
            match self
                .dispatcher
                .hello_verified(&ctx.ctx, frame)
                .map_err(|err| invalid(err.to_string()))?
            {
                HelloOutcome::Accepted { caps_frame, .. }
                | HelloOutcome::UpdateRequired { caps_frame } => caps_frame,
            }
        } else {
            let hello = request
                .msg
                .hello
                .as_ref()
                .ok_or_else(|| invalid("Missing client hello".into()))?;
            let hello = read_frame(hello, limits).map_err(|err| invalid(err.to_string()))?;
            let conn = Connection::from_verified_hello(hello, frame.stream_id())
                .map_err(|err| invalid(err.to_string()))?
                .ok_or_else(|| invalid("Unsupported client versions".into()))?;
            self.dispatcher
                .dispatch_verified(&ctx.ctx, &conn, frame)
                .await
        };
        Ok(Response::new(v1::UiReply {
            frame: Some(reply),
            ..Default::default()
        }))
    }
}
