//! Native `Serve` impls for [`HealthHandler`].
//!
//! [`HealthHandler`] and its `ServeWriter` impl are target-agnostic and live in
//! [`crate::shared::handlers::health`]. This module adds the TCP `Serve` path,
//! which takes a `SharedByteBufferStream<RawStream>` and exists only on native
//! targets because `foundation_netio::netcap::RawStream` is native-only.

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::SimpleIncomingRequest;

use crate::native::serve::{Serve, ServeFactory};
use crate::shared::context::Ctx;
use crate::shared::handlers::health::health_body;
use crate::shared::handlers::HealthHandler;
use crate::shared::serve::{ConnectionResult, ServeWriterFactory};

/// Native Serve impl: writes through the shared `ResponseWriter`.
impl Serve for HealthHandler {
    fn serve(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let full_url = &req.request_url.url;
        let path = full_url.split('?').next().unwrap_or("/");
        crate::native::writer::send_json(conn, 200, &health_body(path), &ctx)
    }
}

impl ServeFactory for HealthHandler {
    fn create(ctx: &Ctx) -> Self {
        <Self as ServeWriterFactory>::create(ctx)
    }
}
