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

use std::{path::Path, sync::Arc};

mod assets;
mod screens;
type Error = Box<dyn std::error::Error + Send + Sync>;

mod services;
pub(crate) use services::TodoFeed;

#[foundation_ui_http::get("/initial")]
fn initial(req: ViewRequest) -> Result<Response, Error> {
    let feed = req.require::<TodoFeed>()?;
    let (state, cursor) = req.require::<TodoStore>()?.published_snapshot(&feed.0);
    Ok(Response::frame(screens::page(
        state,
        feed.0.stream_id(),
        cursor,
    )))
}

#[foundation_ui_http::get("/open-count")]
fn count_screen(req: ViewRequest) -> Result<Response, Error> {
    let state = req.require::<TodoStore>()?.snapshot();
    let count = state.items.iter().filter(|item| !item.done).count();
    Ok(Response::frame(screens::starter(count)))
}

/// Builds the application with a fresh count stream.
///
/// # Errors
/// Returns an error when route or asset setup fails.
pub fn app(module: &Path) -> Result<HttpApp<Arc<dyn Serve>>, Error> {
    app_with_todos(module, Vec::new())
}

/// Starts the todo server with a supplied list and its existing identifiers.
///
/// # Errors
/// Returns an error for duplicate or zero identifiers, or if route setup fails.
pub fn app_with_todos(
    module: &Path,
    items: Vec<crate::Todo>,
) -> Result<HttpApp<Arc<dyn Serve>>, Error> {
    let mut ids = std::collections::HashSet::with_capacity(items.len());
    if items
        .iter()
        .any(|item| item.id == 0 || !ids.insert(item.id))
    {
        return Err("Invalid restored todo identifiers".into());
    }
    app_with_store(module, services::channel(1), TodoStore::with_items(items))
}

/// Builds the application with a host-supplied stream for replay tests.
///
/// # Errors
/// Returns an error when the channel, routes, signer or assets are invalid.
pub fn app_with_channel(
    module: &Path,
    channel: LiveChannel,
) -> Result<HttpApp<Arc<dyn Serve>>, Error> {
    app_with_store(module, channel, TodoStore::default())
}

fn app_with_store(
    module: &Path,
    channel: LiveChannel,
    store: TodoStore,
) -> Result<HttpApp<Arc<dyn Serve>>, Error> {
    let mut app = HttpApp::new_serve();
    let ctx = app.context().clone();
    let channels = services::install(&ctx, channel, store)?;
    register(&mut app.router, &ctx, channels)?;
    assets::register(&mut app.router, module)?;
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

/// Starts the plain data service with one known todo. Only the generated data procedure is mounted.
///
/// # Errors
/// Returns the route registration error.
pub fn data_app() -> Result<HttpApp<Arc<dyn Serve>>, Error> {
    let mut app = HttpApp::new_serve();
    app.context().store(TodoStore::with_items(vec![crate::Todo {
        id: 42,
        text: "Buy milk".into(),
        done: false,
    }]));
    let mut rpc = foundation_connectrpc::Router::new();
    crate::todo_rpc::register_todos(&mut rpc, Arc::new(services::Todos));
    app.router.at(
        crate::todo_rpc::procedure::GET_TODO,
        foundation_http::post(foundation_connectrpc::ConnectRpcServe::new(
            rpc.into_handler(),
        )),
    )?;
    Ok(app)
}
