//! Dropping the create owner cancels the create task and its substrate.

use std::sync::Arc;

use foundation_sandbox::shared::control::testing::{spec, ScriptedSlave, SimDriver, SubstrateOp};
use foundation_sandbox::shared::control::SandboxProvider;

use super::support;

/// Waits until the launched substrate is killed and cleaned up and the record
/// is gone.
fn wait_for_cancel(
    substrate: &foundation_sandbox::shared::control::testing::SimSubstrate,
    manager: &foundation_sandbox::shared::control::SandboxManager,
) {
    let mut attempts = 0;
    while (substrate.op_count(SubstrateOp::Kill) == 0
        || substrate.op_count(SubstrateOp::Cleanup) == 0
        || !support::block_on(manager.list()).expect("list").is_empty())
        && attempts < 10_000
    {
        std::thread::yield_now();
        attempts += 1;
    }
    assert_eq!(substrate.op_count(SubstrateOp::Kill), 1);
    assert!(substrate.op_count(SubstrateOp::Cleanup) >= 1);
    assert!(support::block_on(manager.list()).expect("list").is_empty());
}

#[test]
fn drop_create_receiver_cancels() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(95);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_info_pending();
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let substrate = driver.substrate();
    let (manager, _store) = support::simple_manager(ctx, driver, Arc::clone(&slave));
    let mut stream = manager.create_with_progress(spec());
    // Read until the create reaches the slave wait, so the substrate is launched.
    let mut attempts = 0;
    loop {
        match stream.next() {
            Some(_) => {
                if slave.call_count("info") > 0 {
                    break;
                }
            }
            None => panic!("the create stream ended before the slave wait"),
        }
        attempts += 1;
        assert!(attempts < 100, "the create did not reach the slave wait");
    }
    assert_eq!(manager.inner().records().len(), 1);
    drop(stream);
    wait_for_cancel(&substrate, &manager);
}

#[test]
fn drop_create_future_cancels() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(96);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_info_pending();
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let substrate = driver.substrate();
    let (manager, _store) = support::simple_manager(ctx, driver, Arc::clone(&slave));
    let waiter = futures::future::poll_fn(|context| {
        if slave.call_count("info") > 0 {
            std::task::Poll::Ready(())
        } else {
            context.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    });
    let create = manager.create(spec());
    let _ = support::block_on(futures::future::select(create, waiter));
    wait_for_cancel(&substrate, &manager);
}
