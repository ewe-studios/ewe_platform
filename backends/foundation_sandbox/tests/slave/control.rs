//! The slave's control role (feature 11, section 7).
//!
//! A slave started with `--control` serves both `SlaveService` and
//! `SandboxService` on the same listener. Without the argument the control
//! paths answer `NotFound` (invariant I4).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use foundation_connectrpc::shared::transport::Transport;
use foundation_connectrpc::{ClientOptions, ConnectError, H1Transport, Request, RpcCtx};
use foundation_errstacks::ErrorTrace;
use foundation_netio::HttpClientBuilder;

use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::rpc::messages::{Empty, ListResponse, ProbeResponse};
use foundation_sandbox::shared::rpc::SandboxServiceClient;

use super::support::{block_on, sandbox_error, SlaveProcess};

/// A `SandboxService` client on the listener of one slave.
fn control_client(socket: &Path) -> SandboxServiceClient {
    let http = HttpClientBuilder::new()
        .unix_socket(socket)
        .read_timeout(Duration::from_secs(60))
        .build();
    let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(http));
    SandboxServiceClient::new(
        transport,
        "http://slave.local",
        ClientOptions::new().with_codec("json"),
    )
    .expect("the control client builds")
}

/// Calls `Probe` on the control service of one slave.
fn probe(client: SandboxServiceClient) -> Result<ProbeResponse, ErrorTrace<ConnectError>> {
    let rpc_ctx = RpcCtx::background().with_deadline(Duration::from_secs(60));
    block_on(async move { client.probe(rpc_ctx, Request::new(Empty {})).await })
        .map(|response| response.msg)
}

/// Calls `List` on the control service of one slave.
fn list(client: SandboxServiceClient) -> Result<ListResponse, ErrorTrace<ConnectError>> {
    let rpc_ctx = RpcCtx::background().with_deadline(Duration::from_secs(60));
    block_on(async move { client.list(rpc_ctx, Request::new(Empty {})).await })
        .map(|response| response.msg)
}

/// I4 and "The control paths are off by default".
#[test]
fn slave_control_off() {
    let slave = SlaveProcess::start("control-off", None, 4);
    let error = list(control_client(slave.socket_path())).expect_err("the call must fail");
    match sandbox_error(&error) {
        SandboxError::NotFound { what } => assert_eq!(what, "method"),
        other => panic!("expected NotFound, got {other:?}"),
    }
    // `Info` is still there and says that the slave cannot control.
    let info = slave.caller.info().expect("Info answers");
    assert!(!info.can_control);
}

/// I4 and `slave_control_on`: both services answer on the same listener.
#[test]
fn slave_control_on() {
    let slave = SlaveProcess::start_with_control("control-on");
    // The slave service answers.
    let info = slave.caller.info().expect("Info answers");
    // The control service answers. This host has no `/dev/kvm`; the probe
    // report is still a report, so the path is served.
    let report = probe(control_client(slave.socket_path())).expect("Probe answers");
    let firecracker = report
        .reports
        .iter()
        .find(|report| report.driver == foundation_sandbox::shared::ids::DriverId::Firecracker)
        .expect("the Firecracker report");
    // `can_control` is the probe result of the Firecracker driver.
    assert_eq!(info.can_control, firecracker.usable);
}
