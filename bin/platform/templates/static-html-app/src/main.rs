use std::sync::Arc;
use foundation_core::valtron::valtron;
use foundation_http::shared::app::HttpApp;
use foundation_http::ShutdownSignal;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use foundation_http::native::handlers::static_file::StaticFileHandler;

foundation_resources::packaged_resources!(PackagedResources);

#[valtron(seed = 10, threads = 10)]
fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let mut app = HttpApp::new_serve();
    app.router.mount(StaticFileHandler::from_resources(PackagedResources))
        .expect("register application resources");

    let server = app.server("127.0.0.1:3000");
    let shutdown = Arc::new(ShutdownSignal::new());
    server.serve(&shutdown);
}
