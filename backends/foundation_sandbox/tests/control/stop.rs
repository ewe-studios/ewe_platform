//! The stop and kill stories.

use std::sync::Arc;

use std::time::Duration;

use foundation_core::valtron::{execute, from_future, StreamIteratorExt};
use foundation_sandbox::shared::control::testing::{spec, ScriptedSlave, SimDriver, SubstrateOp};
use foundation_sandbox::shared::control::{SandboxProvider, SandboxState};
use foundation_sandbox::shared::error::SandboxError;

use super::support;

#[test]
fn stop_graceful() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(40);
    let slave = Arc::new(ScriptedSlave::new());
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let substrate = driver.substrate();
    slave.with_shutdown_exit(Arc::clone(&substrate));
    let (manager, _store) = support::simple_manager(ctx, driver, Arc::clone(&slave));
    let handle = support::block_on(manager.create(spec())).expect("create");
    let report = support::block_on(handle.stop()).expect("stop");
    assert!(!report.killed);
    assert_eq!(report.substrate_exit_code, Some(0));
    assert_eq!(slave.call_count("shutdown"), 1);
    assert_eq!(substrate.op_count(SubstrateOp::Terminate), 0);
    assert!(substrate.op_count(SubstrateOp::Cleanup) >= 1);
    assert_eq!(handle.info().state, SandboxState::Stopped);
    assert!(support::block_on(manager.list()).expect("list").is_empty());
}

#[test]
fn stop_forced() {
    let _serial = support::serial();
    let (ctx, clock) = support::ctx(41);
    let slave = Arc::new(ScriptedSlave::new());
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let substrate = driver.substrate();
    let (manager, _store) = support::simple_manager(ctx, driver, Arc::clone(&slave));
    let handle = support::block_on(manager.create(spec())).expect("create");
    let limits = handle.info();
    assert_eq!(limits.state, SandboxState::Ready);
    let stopper = handle.clone();
    let task = execute(from_future(async move { stopper.stop().await }), None).expect("schedule");
    let mut attempts = 0;
    while slave.call_count("shutdown") == 0 && attempts < 10_000 {
        std::thread::yield_now();
        attempts += 1;
    }
    clock.advance(Duration::from_secs(6));
    let (result, _rest) = support::block_on(task.into_ready_future()).expect("ready");
    let report = result.expect("stop");
    assert!(report.killed);
    assert_eq!(substrate.op_count(SubstrateOp::Terminate), 1);
    assert!(substrate.op_count(SubstrateOp::Cleanup) >= 1);
}

#[test]
fn stop_twice() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(42);
    let slave = Arc::new(ScriptedSlave::new());
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let substrate = driver.substrate();
    slave.with_shutdown_exit(substrate);
    let (manager, _store) = support::simple_manager(ctx, driver, Arc::clone(&slave));
    let handle = support::block_on(manager.create(spec())).expect("create");
    let first = support::block_on(handle.stop()).expect("first");
    let second = support::block_on(handle.stop()).expect("second");
    assert_eq!(first, second);
    assert_eq!(slave.call_count("shutdown"), 1);
}

#[test]
fn stop_when_slave_gone() {
    let _serial = support::serial();
    let (ctx, clock) = support::ctx(43);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_shutdown_error(SandboxError::Rpc {
        code: "unavailable".to_string(),
        message: "the slave is gone".to_string(),
    });
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let substrate = driver.substrate();
    let (manager, _store) = support::simple_manager(ctx, driver, Arc::clone(&slave));
    let handle = support::block_on(manager.create(spec())).expect("create");
    let stopper = handle.clone();
    let task = execute(from_future(async move { stopper.stop().await }), None).expect("schedule");
    let mut attempts = 0;
    while slave.call_count("shutdown") == 0 && attempts < 10_000 {
        std::thread::yield_now();
        attempts += 1;
    }
    clock.advance(Duration::from_secs(6));
    let (result, _rest) = support::block_on(task.into_ready_future()).expect("ready");
    let report = result.expect("the failed Shutdown is not an error");
    assert!(report.killed);
    assert!(substrate.op_count(SubstrateOp::Terminate) >= 1);
}

#[test]
fn kill() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(44);
    let slave = Arc::new(ScriptedSlave::new());
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let substrate = driver.substrate();
    let (manager, _store) = support::simple_manager(ctx, driver, Arc::clone(&slave));
    let handle = support::block_on(manager.create(spec())).expect("create");
    let report = support::block_on(handle.kill()).expect("kill");
    assert!(report.killed);
    assert_eq!(substrate.op_count(SubstrateOp::Kill), 1);
    assert!(substrate.op_count(SubstrateOp::Cleanup) >= 1);
    assert_eq!(handle.info().state, SandboxState::Stopped);
}

#[test]
fn stop_on_manager_after_handle_stop() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(45);
    let slave = Arc::new(ScriptedSlave::new());
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let substrate = driver.substrate();
    slave.with_shutdown_exit(substrate);
    let (manager, _store) = support::simple_manager(ctx, driver, slave);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let report = support::block_on(handle.stop()).expect("stop");
    let again = support::block_on(manager.stop(handle.id().clone())).expect("manager stop");
    assert_eq!(report, again);
}

#[test]
fn stop_of_a_creating_sandbox_refuses() {
    let _serial = support::serial();
    let (ctx, clock) = support::ctx(46);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_info_pending();
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _store) = support::simple_manager(ctx, driver, slave);
    let mut stream = manager.create_with_progress(spec());
    let id = {
        // The first step adds the record in `Creating`.
        let mut attempts = 0;
        loop {
            match stream.next() {
                Some(foundation_core::valtron::Stream::Pending(_)) => break,
                Some(_) => {
                    attempts += 1;
                    assert!(attempts < 100);
                }
                None => panic!("the create ended"),
            }
        }
        let listed = support::block_on(manager.list()).expect("list");
        listed.first().expect("one record").id.clone()
    };
    let id = id.parse().expect("id");
    let error = support::expect_error(support::block_on(manager.stop(id)), "creating");
    assert!(matches!(error, SandboxError::SandboxStopped));
    // Let the create task reach its deadline, so it leaves no worker behind.
    clock.advance(Duration::from_secs(11));
    drop(stream);
    std::thread::yield_now();
}
