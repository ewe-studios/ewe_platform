//! Health check handler — responds to /health, /health/live, /health/ready.

use foundation_netio::shared::http::SimpleIncomingRequest;
use serde_json::json;

use crate::shared::context::ContextBag;
use crate::shared::serve::{respond_to_buffer, ConnectionResult, ServeWriter, ServeWriterFactory};

/// The JSON body for a health `path`.
///
/// The native `Serve` impl in [`crate::native::handlers::health`] shares it.
pub(crate) fn health_body(path: &str) -> serde_json::Value {
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

        respond_to_buffer::json(&mut conn, 200, &health_body(path))
            .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep)
    }
}
