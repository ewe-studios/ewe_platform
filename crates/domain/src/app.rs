use crate::{domains, servicer};

pub fn create<App: domains::Domain>() -> (
    App,
    servicer::DServicer<App::Events, App::Requests, App::Platform>,
) {
    let app = App::default();
    let app_server = servicer::create(App::Platform::default());
    (app, app_server)
}
