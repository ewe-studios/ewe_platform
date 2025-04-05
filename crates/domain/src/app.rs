use crate::{core, domains, servicer};

#[allow(clippy::type_complexity)]
pub fn create<App>() -> (
    core::CoreExecutor,
    servicer::DServicer<App, App::Events, App::Requests, App::Platform>,
)
where
    App: domains::Domain + 'static,
{
    let app_server = servicer::create::<App, App::Events, App::Requests, App::Platform>();

    let mut app_core = core::CoreExecutor::new();
    app_core.register(Box::new(app_server.clone()));

    (app_core, app_server)
}
