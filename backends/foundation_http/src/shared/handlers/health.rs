//! Health check handler — responds to /health, /health/live, /health/ready.

use foundation_netio::shared::http::SimpleIncomingRequest;
use serde_json::json;
#[cfg(not(target_family = "wasm"))]
use std::sync::Arc;

use crate::shared::context::ContextBag;
use crate::shared::serve::{respond, ConnectionResult, ServeWriter, ServeWriterFactory};

/// The JSON body for a health `path`.
fn health_body(path: &str) -> serde_json::Value {
    if path == "/health/live" || path == "/health/ready" {
        json!({ "status": "ok" })
    } else {
        json!({ "status": "ok", "path": path })
    }
}

/// Health check handler.
pub struct HealthHandler;

impl ServeWriterFactory for HealthHandler {
    fn create(_bag: &ContextBag) -> Self {
        Self
    }
}

impl ServeWriter for HealthHandler {
    fn serve_writer(
        &self,
        _bag: &ContextBag,
        req: SimpleIncomingRequest,
        mut conn: &mut dyn std::io::Write,
    ) -> ConnectionResult {
        let full_url = &req.request_url.url;
        let path = full_url.split('?').next().unwrap_or("/");

        respond::json(&mut conn, 200, &health_body(path))
            .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep)
    }
}

/// Native Serve impl: writes through the shared `ResponseWriter`.
#[cfg(not(target_family = "wasm"))]
impl crate::shared::serve::Serve for HealthHandler {
    fn serve(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: foundation_core::io::ioutils::SharedByteBufferStream<
            foundation_netio::netcap::RawStream,
        >,
    ) -> ConnectionResult {
        let _ = bag;
        let full_url = &req.request_url.url;
        let path = full_url.split('?').next().unwrap_or("/");
        crate::native::writer::send_json(conn, 200, &health_body(path))
    }
}

#[cfg(not(target_family = "wasm"))]
impl crate::shared::serve::ServeFactory for HealthHandler {
    fn create(bag: &ContextBag) -> Self {
        <Self as ServeWriterFactory>::create(bag)
    }
}
