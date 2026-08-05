//! F02 lifecycle tests (unix): boot real short-lived children, confirm status
//! and pid, stop, restart, and Drop teardown.

use std::time::Duration;

use foundation_core::valtron::valtron_test;
use foundation_nativeapis::daemon::platform::process_alive;
use foundation_nativeapis::daemon::{DaemonDef, DaemonGroup, DaemonStatus};

fn sleeper(name: &str) -> DaemonDef {
    // `sleep` is a coreutils binary present on every unix CI image.
    DaemonDef::new(name, vec!["sleep".into(), "30".into()])
}

#[valtron_test]
fn boot_marks_ready_and_records_pid() {
    let group = DaemonGroup::boot(vec![sleeper("s")]).expect("boot");
    let handle = group.daemon("s").expect("handle");
    assert_eq!(handle.status(), DaemonStatus::Ready);
    assert!(handle.pid().is_some(), "pid recorded after spawn");
    // Explicit stop keeps the drop path a no-op.
    handle.stop().expect("stop");
    assert_eq!(handle.status(), DaemonStatus::Stopped);
}

#[valtron_test]
fn stop_actually_kills_the_process() {
    let group = DaemonGroup::boot(vec![sleeper("s").restart(false)]).expect("boot");
    let handle = group.daemon("s").expect("handle");
    let pid = handle.pid().expect("pid");
    assert!(process_alive(pid));
    handle.stop().expect("stop");
    assert!(!process_alive(pid), "process gone after stop");
}

#[valtron_test]
fn restart_replaces_pid() {
    let group = DaemonGroup::boot(vec![sleeper("s").restart(false)]).expect("boot");
    let handle = group.daemon("s").expect("handle");
    let first = handle.pid().expect("first pid");

    handle.restart().expect("restart");
    let second = handle.pid().expect("second pid");
    assert_ne!(first, second, "restart spawns a fresh process");
    assert_eq!(handle.status(), DaemonStatus::Ready);
    assert!(!process_alive(first), "old process reaped");
    handle.stop().expect("stop");
}

#[valtron_test]
fn drop_tears_down_all_children() {
    let pid;
    {
        let group = DaemonGroup::boot(vec![sleeper("s").restart(false)]).expect("boot");
        pid = group.daemon("s").expect("handle").pid().expect("pid");
        assert!(process_alive(pid));
        // group drops here
    }
    // Give the two-phase kill a moment to complete.
    std::thread::sleep(Duration::from_millis(200));
    assert!(!process_alive(pid), "child killed on group drop");
}

#[valtron_test]
fn dependency_order_boots_all() {
    let group = DaemonGroup::boot(vec![
        sleeper("a"),
        sleeper("b").depends(&["a"]),
        sleeper("c").depends(&["b"]),
    ])
    .expect("boot");

    for name in ["a", "b", "c"] {
        assert_eq!(
            group.daemon(name).expect(name).status(),
            DaemonStatus::Ready
        );
    }
    group.shutdown();
}
