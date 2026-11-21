//! Register the browser and mobile routes for the todo application.
use crate::TodoStore;
use foundation_browser::test::{stream::StreamHandler, LiveChannel};
use foundation_context::Ctx;
use foundation_http::{
    get,
    native::serve::ServeFactory,
    shared::{app::HttpApp, router::Router},
    Serve,
};
use foundation_ui_http::{ActionRegistryRouteHandler, Response, ViewRequest};

use std::{path::Path, sync::Arc};

mod assets;
mod screens;
type Error = Box<dyn std::error::Error + Send + Sync>;
const COUNT_NODE: u32 = 44;

mod services;
pub(crate) use services::TodoChannels;

#[foundation_ui_http::get("/initial")]
fn initial(req: ViewRequest) -> Result<Response, Error> {
    let state = req.require::<TodoStore>()?.snapshot();
    Ok(Response::frame(screens::web_page(state)))
}

#[foundation_ui_http::get("/native-initial")]
fn native_initial(req: ViewRequest) -> Result<Response, Error> {
    let channel = req.require::<TodoChannels>()?;
    let cursor = channel.native.last_sequence();
    let state = req.require::<TodoStore>()?.snapshot();
    Ok(Response::frame(screens::mobile_page(
        state,
        channel.native.stream_id(),
        cursor,
    )))
}

#[foundation_ui_http::get("/native-starter")]
fn native_starter(req: ViewRequest) -> Result<Response, Error> {
    let state = req.require::<TodoStore>()?.snapshot();
    let count = state.items.iter().filter(|item| !item.done).count();
    Ok(Response::frame(screens::starter(count)))
}

/// Builds the application with a fresh count stream.
///
/// # Errors
/// Returns an error when route or asset setup fails.
pub fn app(module: &Path) -> Result<HttpApp<Arc<dyn Serve>>, Error> {
    app_with_channel(module, services::channel(1))
}

/// Builds the application with a host-supplied stream for replay tests.
///
/// # Errors
/// Returns an error when the channel, routes, signer or assets are invalid.
pub fn app_with_channel(
    module: &Path,
    channel: LiveChannel,
) -> Result<HttpApp<Arc<dyn Serve>>, Error> {
    let mut app = HttpApp::new_serve();
    let ctx = app.context().clone();
    let channels = services::install(&ctx, channel)?;
    register(&mut app.router, &ctx, channels)?;
    assets::register(&mut app.router, module)?;
    Ok(app)
}

fn register(
    router: &mut Router<Arc<dyn Serve>>,
    ctx: &Ctx,
    channels: TodoChannels,
) -> Result<(), Error> {
    router.mount(ActionRegistryRouteHandler::new(crate::server_registry()?))?;
    router.mount(initial)?;
    router.mount(native_initial)?;
    router.mount(native_starter)?;
    router.at(
        "/live",
        get(StreamHandler::create(ctx).with_channel(channels.web)),
    )?;
    router.at(
        "/native-live",
        get(StreamHandler::native(ctx).with_channel(channels.native)),
    )?;
    Ok(())
}
