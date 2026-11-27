//! A Durable Streams server over the memory VFS.
//!
//! This is the target for the reference server conformance suite:
//!
//! ```text
//! cargo run -p foundation_http --features multi --example durable_server
//! npx @durable-streams/server-conformance-tests --run http://127.0.0.1:4437
//! ```
//!
//! `DURABLE_ADDR` changes the bind address.

#[cfg(not(unix))]
fn main() {
    eprintln!("the durable stream example serves only on a Unix target");
}

#[cfg(unix)]
fn main() {
    use foundation_core::durable::{DurableLimits, DurableStream, DurableStreamVfs, MemoryVfs};
    use foundation_core::valtron::initialize_pool;
    use foundation_event_stream::native::{NetioWebhookSender, SubscriptionHandler};
    use foundation_event_stream::shared::{
        SubscriptionConfig, SubscriptionEngine, WebhookSigningKey,
    };
    use foundation_http::native::durable::DurableHandler;
    use foundation_http::native::server::HttpServer;
    use foundation_http::shared::app::HttpApp;
    use foundation_http::shared::context::Ctx;
    use foundation_http::ShutdownSignal;
    use std::sync::Arc;

    // `DURABLE_LOG` turns on tracing. The value names the log level, for example
    // `DURABLE_LOG=warn` or `DURABLE_LOG=trace`. The server logs nothing unless
    // the variable is set.
    if let Ok(filter) = std::env::var("DURABLE_LOG") {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .init();
    }

    let _guard = initialize_pool(42, Some(4));
    let addr = std::env::var("DURABLE_ADDR").unwrap_or_else(|_| "127.0.0.1:4437".to_string());

    let ctx = Ctx::host();
    // The delivery worker bounds one webhook attempt with the timer of this environment. The
    // timer reads the injected clock.
    let _environment = foundation_core::env::install(&ctx, foundation_core::env::system)
        .expect("the host effect environment builds");
    let subscriptions = SubscriptionConfig::new("v1/stream");
    let vfs: Arc<dyn DurableStreamVfs> = Arc::new(MemoryVfs::new());
    let signing = WebhookSigningKey::generate(&ctx);
    let sender = Arc::new(NetioWebhookSender::new(&subscriptions));
    let engine = SubscriptionEngine::new(
        vfs.clone(),
        ctx.clone(),
        signing,
        sender,
        subscriptions.clone(),
    );
    let observed = engine.observed_vfs();
    let ticker = engine.clone();
    let _ = ctx.get_or_insert(DurableStream::with_ctx(observed, &ctx));
    let _ = ctx.get_or_insert(engine);
    let _ = ctx.get_or_insert(subscriptions);
    // The conformance suite waits for a long-poll timeout inside a 5 s test
    // limit. A 3 s wait fits, and the production default stays at 30 s.
    let _ = ctx.get_or_insert(DurableLimits {
        long_poll_timeout_ms: 3_000,
        ..DurableLimits::default()
    });

    let mut app = HttpApp::with_context(ctx);
    // The reserved `__ds` prefix routes before the stream fallback.
    app.route_any::<SubscriptionHandler>("/v1/stream/__ds/*");
    app.fallback::<DurableHandler>();

    let shutdown = Arc::new(ShutdownSignal::new());
    let shutdown_thread = shutdown.clone();
    let bind = addr.clone();
    let _ = std::thread::spawn(move || {
        HttpServer::new(app, &bind).serve(&shutdown_thread);
    });

    // Expire due leases and run due webhook retries. A production host runs the same call on its
    // scheduler; the example uses one thread for it.
    let tick_shutdown = shutdown.clone();
    let _ = std::thread::spawn(move || {
        while !tick_shutdown.probe() {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let ticker = ticker.clone();
            foundation_core::valtron::block_on_future(async move {
                ticker.tick().await;
            });
        }
    });

    println!("durable stream server listening on http://{addr}");
    loop {
        std::thread::park();
    }
}
