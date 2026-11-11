use std::sync::Arc;

use foundation_core::valtron::valtron;
use foundation_http::ShutdownSignal;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[valtron(seed = 10, threads = 10)]
fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let app = wello::app();
    let server = app.server("127.0.0.1:3000");
    let shutdown = Arc::new(ShutdownSignal::new());
    server.serve(&shutdown);
}
