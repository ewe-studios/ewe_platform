//! `HttpApp` takes its clock and RNG from the caller (spec 72 feature 01, issue #234).
//!
//! WHY: the plain constructors called `ContextBag::from_ctx(&Ctx::host())`, so a
//! deterministic host could not inject its clock or its seeded RNG. The host had
//! to replace `app.ctx` after construction, and a forgotten step failed
//! silently.
//!
//! WHAT: `HttpApp::with_ctx` and `HttpApp::with_clock` build the app from an
//! explicit valtron `Ctx`. `HttpApp::with_clock` is the deterministic-test entry
//! point; the plain constructors keep their signature and stay the host
//! boundary.
//!
//! HOW: a `SimulatedClock` that the test advances, and a `Ctx` whose shared RNG
//! stream the test draws from before building the app.

use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{Ctx, SimulatedClock};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::ServeWriter;
use rand_chacha::rand_core::Rng;

/// A writer app is enough to prove the constructor: the bag is built before any
/// handler is registered.
type WriterApp = HttpApp<Arc<dyn ServeWriter>>;

#[test]
fn with_clock_keeps_the_injected_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::from_secs(100)));
    let app: WriterApp = HttpApp::with_clock(clock.clone());

    assert_eq!(
        app.context().clock().elapsed(),
        Duration::from_secs(100),
        "the app must read the injected clock, not the real host clock"
    );

    clock.advance(Duration::from_secs(5));

    assert_eq!(
        app.context().clock().elapsed(),
        Duration::from_secs(105),
        "advancing the simulated clock must move the app's clock"
    );
}

#[test]
fn with_ctx_keeps_the_injected_rng() {
    let ctx = Ctx::new(Arc::new(SimulatedClock::new(Duration::ZERO)));

    // Move the shared stream off seed 0, then snapshot it. A fresh
    // `ContextBag` starts at seed 0, so this value proves the app read the
    // injected state.
    let _ = ctx.rng().lock().expect("RNG mutex poisoned").next_u64();
    let expected = ctx
        .rng()
        .lock()
        .expect("RNG mutex poisoned")
        .clone()
        .next_u64();

    let app: WriterApp = HttpApp::with_ctx(ctx.clone());

    assert!(
        Arc::ptr_eq(&app.context().rng(), &ctx.rng()),
        "the app must share the injected RNG stream, not a fresh one"
    );

    let from_app = app
        .context()
        .rng()
        .lock()
        .expect("RNG mutex poisoned")
        .next_u64();
    assert_eq!(
        from_app, expected,
        "the app's first draw must continue the injected stream"
    );

    // Two independent contexts built with the same seed produce the same value.
    let app_a: WriterApp =
        HttpApp::with_ctx(Ctx::new(Arc::new(SimulatedClock::new(Duration::ZERO))));
    let app_b: WriterApp =
        HttpApp::with_ctx(Ctx::new(Arc::new(SimulatedClock::new(Duration::ZERO))));
    assert_eq!(
        app_a
            .context()
            .rng()
            .lock()
            .expect("RNG mutex poisoned")
            .next_u64(),
        app_b
            .context()
            .rng()
            .lock()
            .expect("RNG mutex poisoned")
            .next_u64(),
        "two apps with the same seed must draw the same value"
    );
}

#[test]
fn with_context_shares_the_callers_bag() {
    let clock = Arc::new(SimulatedClock::new(Duration::from_secs(7)));
    let bag = Arc::new(ContextBag::new(clock));
    let app: WriterApp = HttpApp::with_context(bag.clone());

    assert!(
        Arc::ptr_eq(app.context(), &bag),
        "the app must store the caller's bag, not a copy"
    );
    assert_eq!(
        app.context().clock().elapsed(),
        Duration::from_secs(7),
        "the app must read the caller's clock"
    );
}

#[test]
fn plain_constructors_still_build() {
    let _: HttpApp<()> = HttpApp::new();
    let serve: HttpApp<Arc<dyn foundation_http::native::serve::Serve>> = HttpApp::new_serve();
    let writer: WriterApp = HttpApp::new_writer();
    let web: HttpApp<Arc<dyn foundation_http::shared::serve_web::WebServe>> = HttpApp::new_web();

    // The engine-minimum elements are present on every plain constructor.
    serve.context().assert_minimum();
    writer.context().assert_minimum();
    web.context().assert_minimum();
}
