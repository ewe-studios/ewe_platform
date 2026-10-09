//! In-memory static asset handler — serve supplied bytes with a content type and
//! optional extra headers.
//!
//! WHY: Plenty of routes serve a FIXED in-memory blob (an embedded JS runtime, a
//! bundled library, a generated manifest) rather than a file on disk. Without
//! this each one becomes a bespoke `Serve` impl. [`StaticAssetHandler`] is the
//! reusable counterpart to [`StaticFileHandler`](super::static_file::StaticFileHandler):
//! you hand it the content + headers, register it on a path, done.
//!
//! WHAT: [`StaticAssetHandler`] — `new(body, content_type)` plus `with_header`;
//! every request to its route gets `200 OK` with that body and headers.
//!
//! HOW: Holds the body as `Arc<[u8]>` (cheap clones across worker threads) and
//! writes a `SendSafeBody::Bytes` response per request.

use std::sync::Arc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{
    Http11, RenderHttp, SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleOutgoingResponse,
    Status,
};

use crate::shared::context::ContextBag;
use crate::shared::serve::{ConnectionResult, Serve};

/// Serves a fixed in-memory asset (bytes + content type + optional headers) on
/// the route it is registered for. Register it as an `Arc<dyn Serve>`:
///
/// ```ignore
/// let js: Arc<dyn Serve> =
///     Arc::new(StaticAssetHandler::new(RUNTIME_JS, "text/javascript; charset=utf-8"));
/// app.router.add_route_any("/app.js", &js);
/// ```
pub struct StaticAssetHandler {
    body: Arc<[u8]>,
    content_type: String,
    headers: Vec<(String, String)>,
}

impl StaticAssetHandler {
    /// A handler serving `body` with the given `Content-Type`.
    #[must_use]
    pub fn new(body: impl Into<Arc<[u8]>>, content_type: impl Into<String>) -> Self {
        Self { body: body.into(), content_type: content_type.into(), headers: Vec::new() }
    }

    /// Attach an extra response header (e.g. `Cache-Control`). Chainable.
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

impl Serve for StaticAssetHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let mut builder = SimpleOutgoingResponse::builder()
            .with_status(Status::OK)
            .add_header(SimpleHeader::CONTENT_TYPE, self.content_type.clone());
        for (name, value) in &self.headers {
            builder = builder.add_header(SimpleHeader::custom(name), value);
        }
        let Ok(response) = builder.with_body(SendSafeBody::Bytes(self.body.to_vec())).build() else {
            return ConnectionResult::Close(None);
        };
        Http11::response(response)
            .http_render_to_writer(&mut conn)
            .map_or(ConnectionResult::Close(None), |_| ConnectionResult::Keep)
    }
}
