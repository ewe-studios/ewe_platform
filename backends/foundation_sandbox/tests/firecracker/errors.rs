//! The error mapping and the call time limit.

use std::sync::Arc;
use std::time::Duration;

use foundation_sandbox::native::firecracker::generated::vsock::Vsock;
use foundation_sandbox::shared::error::SandboxError;

use super::support::{self, bad_request, response};

#[test]
fn fault_message_is_surfaced() {
    let (ctx, _clock) = support::simulated_ctx(11);
    let server = support::MockApi::start("fault", |_| bad_request(br#"{"fault_message":"bad"}"#));
    let client = support::client(&ctx, &server.socket, Duration::from_secs(5), 64 * 1024);
    let body = Vsock {
        guest_cid: 4,
        uds_path: "/run/vsock.sock".to_string(),
        ..Default::default()
    };

    let error =
        support::block_on(client.put_vsock(&body)).expect_err("the server rejects the call");
    let SandboxError::SubstrateApi { status, message } = error.current_context() else {
        panic!("wrong error: {error:?}");
    };
    assert_eq!(*status, 400);
    assert_eq!(message, "bad");
}

#[test]
fn error_body_is_bounded() {
    let limit = 1024;
    // Ten mebibytes of text that is not JSON. The client must read about
    // `limit` bytes and then stop.
    let body = vec![b'x'; 10 * 1024 * 1024];
    let (ctx, _clock) = support::simulated_ctx(12);
    let server = support::MockApi::start("bounded", move |_| {
        response("400 Bad Request", Some("application/octet-stream"), &body)
    });
    let client = support::client(&ctx, &server.socket, Duration::from_secs(5), limit);

    let error =
        support::block_on(client.describe_instance()).expect_err("the server rejects the call");
    let SandboxError::SubstrateApi { status, message } = error.current_context() else {
        panic!("wrong error: {error:?}");
    };
    assert_eq!(*status, 400);
    assert!(
        message.len() <= limit,
        "the message is {} bytes, over the {} byte limit",
        message.len(),
        limit
    );
}

#[test]
fn call_times_out() {
    let (ctx, clock) = support::simulated_ctx(13);
    let rpc_timeout = Duration::from_millis(500);
    let (server, accepted) = support::HoldingApi::start("timeout");
    let client = support::client(&ctx, &server.socket, rpc_timeout, 1024);

    // The advancer waits until the server has the request, then moves the
    // injected clock past the deadline. No real sleep bounds the call.
    let advancer = {
        let clock = Arc::clone(&clock);
        std::thread::spawn(move || {
            accepted
                .recv()
                .expect("the mock server accepted the request");
            clock.advance(rpc_timeout + Duration::from_millis(1));
        })
    };

    let error = support::block_on(client.describe_instance()).expect_err("the call times out");
    advancer.join().expect("the clock thread finished");
    let _ = server.release.send(());

    let SandboxError::ApiTimeout { timeout_ms } = error.current_context() else {
        panic!("wrong error: {error:?}");
    };
    assert_eq!(*timeout_ms, 500);
}

#[test]
fn socket_path_too_long() {
    let (ctx, _clock) = support::simulated_ctx(14);
    let long_name = "a".repeat(200);
    let socket = std::env::temp_dir().join(long_name);
    let limits = foundation_sandbox::native::firecracker::client::FirecrackerLimits::default();
    let error = foundation_sandbox::native::firecracker::client::FirecrackerClient::with_limits(
        &ctx, &socket, &limits,
    )
    .expect_err("the path is too long");
    assert!(matches!(
        error.current_context(),
        SandboxError::SocketPathTooLong { .. }
    ));
}

#[test]
fn missing_environment_is_typed() {
    let clock = Arc::new(foundation_core::valtron::SimulatedClock::new(
        Duration::ZERO,
    ));
    let ctx = foundation_context::Ctx::with_seed(
        Arc::clone(&clock) as Arc<dyn foundation_context::Clock>,
        15,
    );
    // No `Env`: the client cannot get a timer.
    let socket = std::path::PathBuf::from("/tmp/ewe-sandbox-no-env.sock");
    let limits = foundation_sandbox::native::firecracker::client::FirecrackerLimits::default();
    let error = foundation_sandbox::native::firecracker::client::FirecrackerClient::with_limits(
        &ctx, &socket, &limits,
    )
    .expect_err("no environment");
    assert!(matches!(
        error.current_context(),
        SandboxError::HostNotConfigured
    ));
}
