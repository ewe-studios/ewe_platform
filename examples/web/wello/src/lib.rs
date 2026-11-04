//! The `wello` example app: serves the embedded `public/index.html` at the root.

use std::sync::Arc;

use foundation_http::shared::{
    app::HttpApp,
    context::ContextBag,
    serve::{respond, ConnectionResult, Serve, ServeFactory},
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
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        match Assets::get("static/index.html") {
            Some(html_data) => {
                let content =
                    String::from_utf8(html_data.data.to_vec()).expect("should generate str");
                respond::html(&mut conn, 200, &content)
                    .map_or(ConnectionResult::Close(None), |_| ConnectionResult::Keep)
            }
            None => respond::not_found(&mut conn)
                .map_or(ConnectionResult::Close(None), |_| ConnectionResult::Keep),
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
