//! Server timeouts read the clock from the task `Ctx` (spec 70 feature 132).
//!
//! WHY: the protocol detector and the connection handlers read the real
//! monotonic clock, so a deterministic simulation test could not drive their
//! timeouts.
//!
//! WHAT: the protocol-detection timeout is measured on the engine clock. A test
//! advances a `SimulatedClock` past it and the task ends without a real wait.
//!
//! HOW: `SimulatedClock::advance` moves the clock by hand. Before this feature
//! the detector compared the real clock with `started`, so the simulated
//! advance did nothing and the task returned `Delayed` instead of ending.

#![cfg(all(unix, not(target_family = "wasm")))]

use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::synca::WaitGroup;
use foundation_core::valtron::{SimulatedClock, TaskIterator, TaskStatus};
use foundation_http::native::app::ServerApp;
use foundation_http::native::server::protocol_detect::ProtocolDetectHandler;
use foundation_http::native::server::KeepAliveConfig;
use foundation_http::shared::app::HttpApp;
use foundation_http::ShutdownSignal;
use foundation_netio::netcap::{ConnectionContext, RawStream};

#[test]
fn the_protocol_detection_timeout_fires_on_the_injected_clock() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let _client = TcpStream::connect(listener.local_addr().expect("addr")).expect("connect");
    let (server, _) = listener.accept().expect("accept");
    server.set_nonblocking(true).expect("non-blocking");
    let stream = SharedByteBufferStream::rwrite(RawStream::from_tcp(server).expect("raw stream"));

    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let waits = WaitGroup::new();
    waits.add(1);
    let mut handler = ProtocolDetectHandler::new(
        stream,
        ServerApp::Http1(Arc::new(HttpApp::new_serve())),
        "127.0.0.1:0".to_string(),
        Arc::new(ConnectionContext::default()),
        Arc::new(ShutdownSignal::new()),
        waits.guard(),
        KeepAliveConfig::default(),
        None,
        Ctx::fixed(clock.clone()),
    );

    let ctx = Ctx::fixed(clock.clone());

    assert!(
        matches!(
            handler.next_status(ctx.clone()),
            Some(TaskStatus::Delayed(_))
        ),
        "with a silent client the first poll arms the detection deadline and waits"
    );

    clock.advance(Duration::from_secs(11));

    assert_eq!(
        handler.next_status(ctx),
        None,
        "advancing the injected clock past the detection timeout must end the task"
    );
}
