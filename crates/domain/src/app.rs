use crate::{core, domains, servicer};

pub fn create<App>() -> (
    core::CoreExecutor,
    Box<servicer::DServicer<App, App::Events, App::Requests, App::Platform>>,
)
where
    App: domains::Domain + 'static,
{
    let app_server = Box::new(servicer::create::<
        App,
        App::Events,
        App::Requests,
        App::Platform,
    >());

    let mut app_core = core::CoreExecutor::new();
    app_core.register(app_server.clone());

    (app_core, app_server)
}
