//! The memory VFS runs the shared conformance kit.
//!
//! This is the anchor run for the kit. The file VFS runs the same scenarios in
//! its own crate.

use core::time::Duration;
use foundation_context::Ctx;
use foundation_core::durable::vfs_testkit::{run_all, SCENARIOS};
use foundation_core::durable::{DurableStream, MemoryVfs};
use foundation_core::valtron::{block_on_future_scoped, SimulatedClock};
use std::sync::Arc;

#[test]
fn every_vfs_runs_the_same_scenarios() {
    let make = || {
        let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
        let ctx = Ctx::fixed(clock.clone());
        (
            DurableStream::with_ctx(Arc::new(MemoryVfs::new()), &ctx),
            clock,
        )
    };

    let report = block_on_future_scoped(run_all(&make));
    let names: Vec<&str> = report.iter().map(|scenario| scenario.name).collect();

    assert_eq!(names, SCENARIOS, "the kit runs every scenario in order");
    assert!(
        report.iter().all(|scenario| !scenario.values.is_empty()),
        "every scenario observes at least one value"
    );
}
