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
use foundation_netio::shared::http::{SimpleHeader, SimpleIncomingRequest};

use crate::native::writer::{render_head, write_response, ResponseWriter, WriterConfig};

use crate::native::serve::Serve;
use crate::shared::context::Ctx;
use crate::shared::serve::ConnectionResult;

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
    writer_config: WriterConfig,
}

impl StaticAssetHandler {
    /// A handler serving `body` with the given `Content-Type`.
    #[must_use]
    pub fn new(body: impl Into<Arc<[u8]>>, content_type: impl Into<String>) -> Self {
        Self {
            body: body.into(),
            content_type: content_type.into(),
            headers: Vec::new(),
            writer_config: WriterConfig::default(),
        }
    }

    /// Attach an extra response header (e.g. `Cache-Control`). Chainable.
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Set the write limits (`send_timeout`, the poll fallback). The default is
    /// [`WriterConfig::default`]. Chainable.
    #[must_use]
    pub fn with_writer_config(mut self, config: WriterConfig) -> Self {
        self.writer_config = config;
        self
    }
}

impl Serve for StaticAssetHandler {
    fn serve(
        &self,
        ctx: Ctx,
        _req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let mut headers = vec![(SimpleHeader::CONTENT_TYPE, self.content_type.clone())];
        for (name, value) in &self.headers {
            headers.push((SimpleHeader::custom(name), value.clone()));
        }
        // The body is written from the shared slice: it is never copied.
        let head = match render_head(200, &headers, self.body.len()) {
            Ok(head) => head,
            Err(e) => {
                tracing::error!("Failed to build static asset response: {e}");
                return ConnectionResult::Close(None);
            }
        };
        let writer = ResponseWriter::new(conn, self.writer_config, ctx);
        ConnectionResult::Async(Box::pin(write_response(
            writer,
            head,
            Arc::clone(&self.body),
        )))
    }
}
