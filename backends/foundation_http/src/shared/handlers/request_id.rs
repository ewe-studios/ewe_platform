//! Request ID middleware — propagates or generates X-Request-Id.

use foundation_netio::shared::http::{SimpleHeader, SimpleIncomingRequest};

use crate::shared::context::Ctx;
use crate::shared::middleware::{MiddlewareResult, RequestMiddleware};

/// Middleware that ensures every request has an X-Request-Id header.
///
/// If the client sends X-Request-Id, it's propagated. Otherwise, a new
/// UUID v4 is generated from [`foundation_compact::uuid::new_v4`]. That function
/// takes its bytes from the platform entropy source that `foundation_compact`
/// selects for the target (the operating system source on native,
/// `crypto.getRandomValues` on wasm32), so this crate needs no `uuid` randomness
/// backend of its own. The ID is stored in the request for downstream use.
pub struct RequestIdMiddleware;

impl RequestMiddleware for RequestIdMiddleware {
    fn handle(&self, _ctx: &Ctx, req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        let has_id = req
            .headers
            .iter()
            .any(|(k, _)| format!("{k}").eq_ignore_ascii_case("x-request-id"));

        if !has_id {
            match foundation_compact::uuid::new_v4() {
                Ok(id) => {
                    req.headers
                        .insert(SimpleHeader::custom("X-Request-Id"), vec![id.to_string()]);
                }
                // A request without an ID is still a valid request, so this is a
                // warning, not a failure. The cause is an entropy source that the
                // platform cannot supply; the caller cannot fix that per request.
                Err(e) => {
                    tracing::warn!(error = %e, "Cannot generate a request ID");
                }
            }
        }

        MiddlewareResult::Continue
    }
}
