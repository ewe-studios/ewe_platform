//! Rate limiter middleware — in-memory request rate limiting per IP.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use foundation_netio::simple_http::shared::{
    SimpleIncomingRequest, SimpleOutgoingResponse, SendSafeBody, Status,
    SimpleHeader,
};

use crate::shared::context::ContextBag;
use crate::shared::middleware::{MiddlewareResult, RequestMiddleware};
use crate::shared::client_ip::ClientIp;

/// In-memory rate limiter.
///
/// Tracks request counts per IP address within a sliding time window.
pub struct RateLimiter {
    requests_per_window: usize,
    window: Duration,
    // HashMap<IP, (count, window_start)>
    trackers: Arc<Mutex<HashMap<String, (usize, Instant)>>>,
}

impl RateLimiter {
    /// Create a new `RateLimiter`.
    ///
    /// * `requests_per_window` — maximum requests allowed per window
    /// * `window` — duration of the rate limiting window
    #[must_use]
    pub fn new(requests_per_window: usize, window: Duration) -> Self {
        Self {
            requests_per_window,
            window,
            trackers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn extract_ip(req: &SimpleIncomingRequest) -> &str {
        req.extensions
            .as_ref()
            .and_then(|ext| ext.get::<ClientIp>())
            .map_or("unknown", |cip| cip.0.as_str())
    }
}

impl RequestMiddleware for RateLimiter {
    fn handle(
        &self,
        _ctx: &Arc<ContextBag>,
        req: &mut SimpleIncomingRequest,
    ) -> MiddlewareResult {
        let ip = Self::extract_ip(req);
        let ip_owned = ip.to_string();
        let now = Instant::now();

        let mut trackers = self.trackers.lock().unwrap();
        let entry = trackers.entry(ip_owned).or_insert((0, now));

        // Reset window if expired
        if now.duration_since(entry.1) > self.window {
            entry.0 = 0;
            entry.1 = now;
        }

        entry.0 += 1;

        if entry.0 > self.requests_per_window {
            let retry_after = self.window.as_secs().to_string();
            let response = SimpleOutgoingResponse::builder()
                .with_status(Status::TooManyRequests)
                .add_header(SimpleHeader::RETRY_AFTER, &retry_after)
                .with_body(SendSafeBody::Text("Rate limit exceeded".into()))
                .build()
                .expect("valid 429 response");
            MiddlewareResult::Response(response)
        } else {
            MiddlewareResult::Continue
        }
    }
}
