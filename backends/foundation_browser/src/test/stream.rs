//! Routes for the browser test server.
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::native::serve::{Serve, ServeFactory};
use foundation_http::shared::context::Ctx;
use foundation_http::shared::serve::ConnectionResult;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::SimpleIncomingRequest;
pub use foundation_ui_http::live_stream::{SseTransportConfig, StreamHandler};

/// Test applications use this path for raw events.
pub const STREAM_PATH: &str = "/__primal/stream";

/// Serve raw text and binary events through the test broadcaster.
pub(crate) struct RawStreamHandler(StreamHandler);

impl ServeFactory for RawStreamHandler {
    fn create(ctx: &Ctx) -> Self {
        Self(StreamHandler::broadcaster(ctx))
    }
}

impl Serve for RawStreamHandler {
    fn serve(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        self.0.serve(ctx, req, conn)
    }
}
