//! The rate limiter reads its window on the clock of the `Ctx` it is built from.
//!
//! WHAT: a `RateLimiter` takes a `Ctx` over a `SimulatedClock`. The test
//! refuses a second request in the window, then advances the clock past the
//! window and admits a request again.
//!
//! HOW: the limiter stores the `Ctx` and reads `ctx.clock()` on each request.
//! A limiter that kept a separate clock would not see the test's advance.

use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{Ctx, SimulatedClock};
use foundation_http::shared::handlers::RateLimiter;
use foundation_http::shared::middleware::{MiddlewareResult, RequestMiddleware};
use foundation_http::{SimpleIncomingRequest, SimpleMethod};

fn request() -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_plain_url("/")
        .with_method(SimpleMethod::GET)
        .build()
        .expect("valid request")
}

#[test]
fn rate_limit_window_moves_with_the_ctx_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(clock.clone());
    let limiter = RateLimiter::new(1, Duration::from_secs(10), ctx.clone());
    let mut req = request();

    assert!(
        matches!(limiter.handle(&ctx, &mut req), MiddlewareResult::Continue),
        "the first request in a window is admitted"
    );
    assert!(
        matches!(limiter.handle(&ctx, &mut req), MiddlewareResult::Response(_)),
        "a second request in the same window is refused"
    );

    clock.advance(Duration::from_secs(11));

    assert!(
        matches!(limiter.handle(&ctx, &mut req), MiddlewareResult::Continue),
        "a request after the window has passed is admitted again"
    );
}
