//! Health check handler — responds to /health, /health/live, /health/ready.

use foundation_netio::simple_http::shared::SimpleIncomingRequest;
use serde_json::json;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

use crate::shared::context::ContextBag;
use crate::shared::serve::{ConnectionResult, ServeWriter, ServeWriterFactory, respond};

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

        let body = if path == "/health/live" || path == "/health/ready" {
            json!({ "status": "ok" })
        } else {
            json!({ "status": "ok", "path": path })
        };

        respond::json(&mut conn, 200, &body)
            .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep)
    }
}

/// Native Serve impl (delegates to ServeWriter).
#[cfg(not(target_arch = "wasm32"))]
impl crate::shared::serve::Serve for HealthHandler {
    fn serve(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        mut conn: foundation_core::io::ioutils::SharedByteBufferStream<foundation_netio::netcap::RawStream>,
    ) -> ConnectionResult {
        <Self as ServeWriter>::serve_writer(self, &bag, req, &mut conn)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl crate::shared::serve::ServeFactory for HealthHandler {
    fn create(bag: &ContextBag) -> Self {
        <Self as ServeWriterFactory>::create(bag)
    }
}
