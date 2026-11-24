//! Register the browser and mobile routes for the todo application.
use crate::TodoStore;
use foundation_context::Ctx;
use foundation_http::{
    get,
    native::serve::ServeFactory,
    shared::{app::HttpApp, router::Router},
    Serve,
};
use foundation_ui_http::live_stream::StreamHandler;
use foundation_ui_http::{ActionRegistryRouteHandler, Response, ViewRequest};
use foundation_wasm_ui::server::LiveChannel;

use foundation_http::native::handlers::static_file::StaticFileHandler;
use std::sync::Arc;

foundation_resources::packaged_resources!(PackagedResources);
mod screens;
type Error = Box<dyn std::error::Error + Send + Sync>;

mod services;
pub use services::Todos;
pub(crate) use services::TodoFeed;

#[foundation_ui_http::get("/initial")]
fn initial(req: ViewRequest) -> Result<Response, Error> {
    let feed = req.require::<TodoFeed>()?;
    let (state, cursor, retired_ids) = req.require::<TodoStore>()?.published_snapshot(&feed.0);
    let ctx = req.render_context();
    let page = screens::page(&state, feed.0.stream_id(), cursor);
    foundation_ui_runtime::view::full_view(&ctx, ctx.receiver(), &page, None::<&()>)?;
    crate::TodoDelivery::Snapshot {
        position: cursor,
        items: state.items,
        retired_ids,
    }
    .send(&ctx)?;
    Ok(Response::frame(ctx))
}

#[foundation_ui_http::get("/open-count")]
fn count_screen(req: ViewRequest) -> Result<Response, Error> {
    let state = req.require::<TodoStore>()?.snapshot();
    let count = state.items.iter().filter(|item| !item.done).count();
    Ok(Response::frame(screens::starter(count)))
}

/// Starts the todo server with a supplied list and its existing identifiers.
///
/// # Errors
/// Returns an error for duplicate identifiers, or if route setup fails.
pub fn app_with_todos(items: Vec<crate::Todo>) -> Result<HttpApp<Arc<dyn Serve>>, Error> {
    app_with_store(TodoStore::with_items(items)?, services::channel(1))
}

/// Build the application with a prepared store and live channel.
///
/// # Errors
/// Returns an error when service installation or route registration fails.
pub fn app_with_store(
    store: TodoStore,
    channel: LiveChannel,
) -> Result<HttpApp<Arc<dyn Serve>>, Error> {
    let mut app = HttpApp::new_serve();
    let ctx = app.context().clone();
    let channels = services::install(&ctx, channel, store)?;
    register(&mut app.router, &ctx, channels)?;
    app.router
        .mount(StaticFileHandler::from_resources(PackagedResources))?;
    Ok(app)
}

fn register(
    router: &mut Router<Arc<dyn Serve>>,
    ctx: &Ctx,
    channels: TodoFeed,
) -> Result<(), Error> {
    let actions = ActionRegistryRouteHandler::new(crate::server_registry()?);
    let mut rpc = foundation_connectrpc::Router::new();
    foundation_ui_connectrpc::v1::register_actions(
        &mut rpc,
        Arc::new(foundation_ui_connectrpc::ActionService::new(
            actions.dispatcher(),
        )),
    );
    crate::todo_rpc::register_todos(&mut rpc, Arc::new(services::Todos));
    let handler = Arc::new(rpc.into_handler());
    for path in [
        foundation_ui_connectrpc::v1::procedure::EXCHANGE,
        crate::todo_rpc::procedure::GET_TODO,
    ] {
        router.at(
            path,
            foundation_http::post(foundation_connectrpc::ConnectRpcServe::from_arc(
                handler.clone(),
            )),
        )?;
    }
    router.mount(foundation_ui_http::ActionSocketRoute::new(
        actions.dispatcher(),
    ))?;
    let dispatcher = actions.dispatcher();
    router.mount(actions)?;
    router.mount(initial)?;
    router.mount(count_screen)?;
    let stream = StreamHandler::create(ctx).with_channel(channels.0);
    let events = foundation_ui_http::EventDelivery::new(dispatcher, stream);
    router.at("/live", get(events.clone()))?;
    router.at("/live", foundation_http::post(events))?;
    Ok(())
}
