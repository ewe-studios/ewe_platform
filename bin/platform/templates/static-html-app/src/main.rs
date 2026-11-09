use std::sync::Arc;

use foundation_core::valtron::valtron;
use foundation_http::native::serve::{Serve, ServeFactory};
use foundation_http::native::writer::{send_html, send_not_found};
use foundation_http::shared::{
    app::HttpApp,
    context::ContextBag,
    serve::ConnectionResult,
};
use foundation_http::{RawStream, SharedByteBufferStream, ShutdownSignal, SimpleIncomingRequest, SimpleMethod};
use rust_embed::Embed;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Embed)]
#[folder = "public/"]
#[prefix = "static/"]
struct Assets;

/// Serves the embedded `index.html` at the application root.
struct IndexHandler;

impl ServeFactory for IndexHandler {
    fn create(_bag: &ContextBag) -> Self {
        Self
    }
}

impl Serve for IndexHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        match Assets::get("static/index.html") {
            Some(html_data) => {
                let content =
                    String::from_utf8(html_data.data.to_vec()).expect("should generate str");
                send_html(conn, 200, &content)
            }
            None => send_not_found(conn),
        }
    }
}

#[valtron(seed = 10, threads = 10)]
fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let mut app = HttpApp::new_serve();
    app.route::<IndexHandler>(SimpleMethod::GET, "/");

    let server = app.server("127.0.0.1:3000");
    let shutdown = Arc::new(ShutdownSignal::new());
    server.serve(&shutdown);
}
