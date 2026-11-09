//! The `wello` example app: serves the embedded `public/index.html` at the root.

use std::sync::Arc;

use foundation_http::native::serve::{Serve, ServeFactory};
use foundation_http::native::writer::{send_html, send_not_found};
use foundation_http::shared::{
    app::HttpApp,
    context::ContextBag,
    serve::ConnectionResult,
};
use foundation_http::{RawStream, SharedByteBufferStream, SimpleIncomingRequest, SimpleMethod};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "public/"]
#[prefix = "static/"]
struct Assets;

/// Serves the embedded `index.html` at the application root.
pub struct IndexHandler;

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

/// Build the app with the `/` route registered.
#[must_use]
pub fn app() -> HttpApp<Arc<dyn Serve>> {
    let mut app = HttpApp::new_serve();
    app.route::<IndexHandler>(SimpleMethod::GET, "/");
    app
}
