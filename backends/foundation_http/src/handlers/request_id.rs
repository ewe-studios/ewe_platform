//! Request ID middleware — propagates or generates X-Request-Id.

use std::sync::Arc;

use foundation_core::wire::simple_http::{SimpleHeader, SimpleIncomingRequest};
use uuid::Uuid;

use crate::context::ContextBag;
use crate::middleware::{MiddlewareResult, RequestMiddleware};

/// Middleware that ensures every request has an X-Request-Id header.
///
/// If the client sends X-Request-Id, it's propagated. Otherwise, a new
/// UUID v4 is generated. The ID is stored in the request for downstream use.
pub struct RequestIdMiddleware;

impl RequestMiddleware for RequestIdMiddleware {
    fn handle(
        &self,
        _ctx: &Arc<ContextBag>,
        req: &mut SimpleIncomingRequest,
    ) -> MiddlewareResult {
        let has_id = req.headers.iter().any(|(k, _)| {
            format!("{k}").eq_ignore_ascii_case("x-request-id")
        });

        if !has_id {
            let id = Uuid::new_v4().to_string();
            req.headers.insert(
                SimpleHeader::custom("X-Request-Id"),
                vec![id],
            );
        }

        MiddlewareResult::Continue
    }
}
