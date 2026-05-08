//! Health check handler — responds to /health, /health/live, /health/ready.

use std::sync::Arc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::netcap::RawStream;
use foundation_core::wire::simple_http::SimpleIncomingRequest;
use serde_json::json;

use crate::context::ContextBag;
use crate::serve::{ConnectionResult, Serve, ServeFactory, respond};

/// Health check handler.
pub struct HealthHandler;

impl ServeFactory for HealthHandler {
    fn create(_bag: &ContextBag) -> Self {
        Self
    }
}

impl Serve for HealthHandler {

    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
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
