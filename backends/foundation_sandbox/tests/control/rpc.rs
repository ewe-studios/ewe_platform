//! The same calls through `SandboxService`: the remote client and the service.

use std::sync::Arc;

use foundation_sandbox::shared::command::{Command, ExitStatus};
use foundation_sandbox::shared::control::service::ControlService;
use foundation_sandbox::shared::control::testing::{
    in_memory_control_transport, spec, ScriptedSlave, SimDriver,
};
use foundation_sandbox::shared::control::ManagerConfig;
use foundation_sandbox::shared::control::{MemoryRecordStore, SandboxProvider, SandboxRpcClient};
use foundation_sandbox::shared::driver::SandboxDriver;
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::ids::SandboxId;
use foundation_sandbox::shared::limits::SandboxLimits;
use foundation_sandbox::shared::rpc::messages::ExecEventKind;

use super::support;

fn rpc_client(ctx: foundation_context::Ctx, slave: Arc<ScriptedSlave>) -> SandboxRpcClient {
    let limits = SandboxLimits::default();
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let manager = Arc::new(support::manager(
        ctx.clone(),
        vec![driver as Arc<dyn SandboxDriver>],
        Arc::clone(&slave),
        ManagerConfig::default(),
        Arc::new(MemoryRecordStore::new()),
    ));
    let service = Arc::new(ControlService::new(manager, limits.clone()));
    let transport = in_memory_control_transport(ctx.clone(), service);
    SandboxRpcClient::new(ctx, transport, limits)
}

#[test]
fn rpc_parity_sim() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(70);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_exec_events(vec![
        ExecEventKind::Started { pid: 7 },
        ExecEventKind::Stdout {
            data: b"hi\n".to_vec(),
        },
        ExecEventKind::Exit {
            code: Some(0),
            signal: None,
        },
    ]);
    let client = rpc_client(ctx, Arc::clone(&slave));
    let handle = support::block_on(client.create(spec())).expect("rpc create");
    assert_eq!(slave.call_count("info"), 1);
    let output = support::block_on(handle.run(Command::new(["echo", "hi"]))).expect("run");
    assert_eq!(output.stdout, b"hi\n");
    assert_eq!(output.status, ExitStatus::code(0));
    let listed = support::block_on(client.list()).expect("list");
    assert_eq!(listed.len(), 1);
}

#[test]
fn rpc_error_variants() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(71);
    let slave = Arc::new(ScriptedSlave::new());
    let client = rpc_client(ctx, slave);
    let missing: SandboxId = "aaaaaaaaaaaa".parse().expect("id");
    let error = support::expect_error(support::block_on(client.get(missing.clone())), "missing");
    assert!(matches!(error, SandboxError::NotFound { .. }));
    let error = support::expect_error(support::block_on(client.stop(missing)), "missing");
    assert!(matches!(error, SandboxError::NotFound { .. }));
}

#[test]
fn rpc_stop_reaches_the_local_control() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(72);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_shutdown_exit(Arc::new(
        foundation_sandbox::shared::control::testing::SimSubstrate::new(),
    ));
    let client = rpc_client(ctx, Arc::clone(&slave));
    let handle = support::block_on(client.create(spec())).expect("create");
    // The simulated substrate is not the one the script exits; the stop forces
    // it down after the grace time, so this test only proves the RPC path runs
    // the local stop and returns a report.
    let report = support::block_on(handle.kill()).expect("kill");
    assert!(report.killed);
}
