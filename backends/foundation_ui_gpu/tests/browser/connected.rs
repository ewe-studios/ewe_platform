//! Run the browser host callback with a server transport installed on the same core.
#![cfg(target_family = "wasm")]

use std::{
    rc::Rc,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use foundation_context::Ctx;
use foundation_ui_gpu::{
    browser_handoff::BrowserDeviceExchange,
    lease::{DeviceExchange, GraphicsReady, LeaseError, Notice, Selection},
};
use foundation_ui_runtime::{
    action::{
        Action, ActionFuture, ActionRegistry, ActionTransport, Context, Interaction, ManagerError,
        Origin, Scope, TransportFuture,
    },
    network::client_manager,
};
use foundation_ui_wire::{
    vocab::STRUCTURED_INPUT_VERSION,
    writer::{ClientCaps, WriterConfig},
    ServerActionT, ServerCapsT,
};
use js_sys::Function;
use wasm_bindgen::prelude::*;

#[derive(Default)]
struct Server(AtomicUsize);

impl ActionTransport for Server {
    fn send(&self, _: Vec<u8>) -> TransportFuture<'_> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Box::pin(async {
            Err(ManagerError::Transport(
                "Graphics action reached the server".into(),
            ))
        })
    }
}

struct EditorReady;

impl Action<Notice> for EditorReady {
    type State = ();
    const NAME: &'static str = "editor_ready";

    fn origin() -> Option<Origin> {
        Some(Origin::Host)
    }

    fn scope() -> Scope {
        Scope::Web
    }

    fn call<'a>(&'a self, ctx: &'a Context, event: Interaction<(), Notice>) -> ActionFuture<'a> {
        GraphicsReady.call(ctx, event)
    }
}

/// # Errors
/// Returns a graphics, routing, or test setup error.
#[wasm_bindgen]
pub async fn compute_with_server(host: Function, shader: String) -> Result<Vec<u32>, JsError> {
    let ctx = Ctx::try_host().map_err(|error| JsError::new(&error.to_string()))?;
    let mut registry = ActionRegistry::new();
    registry.register(EditorReady)?;
    let server = Arc::new(Server::default());
    let manager = client_manager(
        ctx.clone(),
        Arc::new(registry),
        &ClientCaps::web_current(),
        &WriterConfig::default(),
    )
    .with_server(server.clone());
    let mut remote_action = ServerActionT::default();
    remote_action.name = Some(EditorReady::NAME.into());
    let mut caps = ServerCapsT::default();
    caps.stream_id = 1;
    caps.vocabulary_version = STRUCTURED_INPUT_VERSION;
    caps.actions = Some(vec![remote_action]);
    manager.apply_caps(&caps);
    let browser = BrowserDeviceExchange::install(
        &ctx,
        Rc::new(manager),
        "host:editor_ready".into(),
        host,
        foundation_ui_gpu::GpuConfig::default().queue_capacity,
    )?;
    let exchange = ctx
        .get::<DeviceExchange>()
        .ok_or(LeaseError::Incompatible)?;
    let result = async {
        let lease = exchange
            .request(Selection {
                device: "editor".into(),
                surface: None,
            })?
            .wait()
            .await?;
        lease.compute(shader, vec![0, 2, 7], 3).await
    }
    .await;
    browser.close();
    if server.0.load(Ordering::Relaxed) != 0 {
        return Err(JsError::new("Graphics action reached the server"));
    }
    Ok(result?)
}
