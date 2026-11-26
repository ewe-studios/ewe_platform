//! The tests of the container port transport.

use std::sync::Arc;
use std::time::Duration;

use foundation_sandbox::shared::cloudflare::testing::transport_round_trip;
use foundation_sandbox::shared::cloudflare::transport::{ContainerPortTransport, PortSource};
use foundation_sandbox::shared::cloudflare::{PortError, PortRequest, PortResponse};
use foundation_sandbox::shared::control::SlaveClient;
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::BoxFuture;

use super::support::{self, block_on, Harness};

/// A port that never answers.
struct Never;

impl PortSource for Never {
    fn fetch(
        &self,
        _ctx: &foundation_context::Ctx,
        _request: PortRequest,
    ) -> BoxFuture<'_, Result<PortResponse, PortError>> {
        Box::pin(std::future::pending())
    }
}

#[test]
fn transport_refuses_request_streaming() {
    let h = Harness::new(50, true, false);
    let container: Arc<dyn foundation_sandbox::shared::cloudflare::ContainerApi> =
        h.container.clone();
    let transport = ContainerPortTransport::over_container(container, 1024);
    assert!(!transport.accepts_request_stream());
}

#[test]
fn transport_streams_response() {
    let h = Harness::new(51, true, false);
    let chunks = 10;
    let body: foundation_sandbox::shared::BoxByteStream = Box::pin(futures::stream::iter(
        (0..chunks).map(|_| Ok(vec![1_u8; 1024 * 1024])),
    ));
    h.container.set_raw_response(PortResponse {
        status: 200,
        headers: vec![(
            "content-type".to_string(),
            "application/octet-stream".to_string(),
        )],
        body,
    });
    let container: Arc<dyn foundation_sandbox::shared::cloudflare::ContainerApi> =
        h.container.clone();
    let transport = ContainerPortTransport::over_container(container, 8080);
    let (status, lengths) =
        transport_round_trip(&transport, "/large", Vec::new()).expect("the exchange works");
    assert_eq!(status, 200);
    assert!(lengths.len() >= chunks, "{lengths:?}");
    assert_eq!(lengths.iter().sum::<usize>(), chunks * 1024 * 1024);
}

#[test]
fn port_call_times_out() {
    // The typed client enforces `rpc_timeout` with the host clock; the port
    // fetch never answers. The deadline must drop the exchange and fail the
    // call instead of waiting forever.
    let ctx = foundation_context::Ctx::host();
    let limits = support::limits().with_rpc_timeout(Duration::from_millis(50));
    let transport = ContainerPortTransport::new(Arc::new(Never));
    let client = SlaveClient::new(ctx, Arc::new(transport), limits.clone()).expect("client");
    let error = block_on(client.info(limits.rpc_timeout)).expect_err("the call times out");
    assert!(
        matches!(
            error.current_context(),
            SandboxError::Rpc { code, .. } if code == "deadline_exceeded"
        ) || matches!(error.current_context(), SandboxError::ApiTimeout { .. }),
        "unexpected error: {error:?}"
    );
}
