//! The end of the sandbox, the reaper and the rules about blocking calls.

use std::time::Duration;

use foundation_sandbox::shared::error::SlaveErrorCode;

use super::support::{
    drain, exec_request, exit_of, hex, plain_prepare_work, process_exists, started_pid, wait_until,
    zombies_of, SlaveProcess,
};

/// The Unix domain socket file has the mode that `--socket-mode` gives.
///
/// The Docker driver sets mode `666`: the slave runs as root in the container
/// and the control plane on the host is a normal user (feature 10, section
/// 6.3). The run directory's own mode keeps other users out.
#[test]
fn slave_socket_mode() {
    use std::os::unix::fs::PermissionsExt as _;

    let slave = SlaveProcess::start_with_socket_mode("socket-mode", None, 4, 0o666);
    let mode = std::fs::metadata(slave.socket_path())
        .expect("the socket file")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o666);
}

/// `Shutdown` gives programs time to stop.
#[test]
fn shutdown_grace() {
    let slave = SlaveProcess::start("shutdown-grace", None, 4);
    slave.prepare(&plain_prepare_work());
    let stream = slave
        .caller
        .exec(exec_request(
            &hex(500),
            &["sh", "-c", "trap 'sleep 0.1; exit 0' TERM; sleep 60 & wait"],
        ))
        .expect("the program starts");
    let (events, stream) = super::support::take_events(stream, 1);
    let pid = started_pid(&events);
    slave.caller.shutdown(2_000).expect("Shutdown answers");
    let events = drain(stream);
    assert!(
        !process_exists(pid),
        "the program traps SIGTERM and exits by itself"
    );
    assert_eq!(exit_of(&events), (Some(0), None));
    let mut slave = slave;
    assert_eq!(slave.wait(), 0, "the slave exits with code 0");
}

/// `Shutdown` kills a program that ignores `SIGTERM` after the grace time.
#[test]
fn shutdown_kill() {
    let slave = SlaveProcess::start("shutdown-kill", None, 4);
    slave.prepare(&plain_prepare_work());
    let stream = slave
        .caller
        .exec(exec_request(
            &hex(501),
            &["sh", "-c", "trap '' TERM; while :; do sleep 0.05; done"],
        ))
        .expect("the program starts");
    let (events, _stream) = super::support::take_events(stream, 1);
    let pid = started_pid(&events);
    let clock = foundation_context::Ctx::host();
    let start = clock.clock().now();
    slave.caller.shutdown(200).expect("Shutdown answers");
    wait_until(|| !process_exists(pid), Duration::from_secs(3));
    assert!(
        clock.clock().now().saturating_duration_since(start) < Duration::from_secs(3),
        "the program is gone after the grace time"
    );
    let mut slave = slave;
    assert_eq!(slave.wait(), 0);
}

/// A second `Shutdown` is accepted and does nothing.
#[test]
fn shutdown_twice() {
    let slave = SlaveProcess::start("shutdown-twice", None, 4);
    slave.prepare(&plain_prepare_work());
    let _stream = slave
        .caller
        .exec(exec_request(
            &hex(502),
            &["sh", "-c", "trap '' TERM; while :; do sleep 0.05; done"],
        ))
        .expect("the program starts");
    slave.caller.shutdown(2_000).expect("the first Shutdown");
    slave.caller.shutdown(2_000).expect("the second Shutdown");
    let mut slave = slave;
    assert_eq!(slave.wait(), 0);
}

/// An `Exec` after `Shutdown` is refused.
#[test]
fn exec_after_shutdown() {
    let slave = SlaveProcess::start("exec-after-shutdown", None, 4);
    slave.prepare(&plain_prepare_work());
    let _stream = slave
        .caller
        .exec(exec_request(
            &hex(503),
            &["sh", "-c", "trap '' TERM; while :; do sleep 0.05; done"],
        ))
        .expect("the program starts");
    slave.caller.shutdown(3_000).expect("Shutdown answers");
    let error = slave.caller.exec_error(exec_request(&hex(504), &["true"]));
    assert!(matches!(
        error,
        foundation_sandbox::shared::error::SandboxError::SlaveError {
            code: SlaveErrorCode::BadRequest,
            ..
        }
    ));
}

/// Orphans are reaped.
#[test]
fn orphans_reaped() {
    let slave = SlaveProcess::start("orphans", None, 8);
    slave.prepare(&plain_prepare_work());
    for index in 0..20 {
        let stream = slave
            .caller
            .exec(exec_request(
                &hex(600 + index),
                &["sh", "-c", "(sleep 0.1 &); exit 0"],
            ))
            .expect("the program starts");
        let events = drain(stream);
        assert_eq!(exit_of(&events), (Some(0), None));
    }
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(
        zombies_of(slave.pid()),
        Vec::<u32>::new(),
        "no zombie child"
    );
}

/// The subreaper adopts the orphan of a program, and the reaper collects it.
#[test]
fn subreaper_adopts() {
    let slave = SlaveProcess::start("subreaper", None, 8);
    slave.prepare(&plain_prepare_work());
    let stream = slave
        .caller
        .exec(exec_request(
            &hex(700),
            &["sh", "-c", "(sleep 0.15 &); exit 0"],
        ))
        .expect("the program starts");
    let events = drain(stream);
    assert_eq!(exit_of(&events), (Some(0), None));
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        zombies_of(slave.pid()),
        Vec::<u32>::new(),
        "the grandchild was reaped"
    );
}

/// The signal mask is set before the pool threads start, so every exit arrives.
#[test]
fn sigchld_blocked_before_threads() {
    let slave = SlaveProcess::start("sigchld", None, 8);
    slave.prepare(&plain_prepare_work());
    for index in 0..200 {
        let stream = slave
            .caller
            .exec(exec_request(&hex(10_000 + index), &["true"]))
            .expect("the program starts");
        let events = drain(stream);
        assert_eq!(exit_of(&events), (Some(0), None), "the exit of run {index}");
    }
}

/// The slave code has no call that blocks a worker on a descriptor.
#[test]
fn no_blocking_calls() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/slave");
    let mut checked = 0_usize;
    for entry in std::fs::read_dir(&directory).expect("the slave source") {
        let path = entry.expect("a source file").path();
        if path.file_name().and_then(|name| name.to_str()) == Some("sys.rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("the source text");
        checked += 1;
        assert!(
            !source.contains("std::thread::spawn"),
            "{} starts a thread",
            path.display()
        );
        assert!(
            !source.contains(".read("),
            "{} reads a blocking descriptor",
            path.display()
        );
        assert!(
            !source.contains(".write("),
            "{} writes a blocking descriptor",
            path.display()
        );
        assert!(
            !source.contains("SystemTime::now"),
            "{} reads the global clock",
            path.display()
        );
        assert!(
            !source.contains("thread_rng"),
            "{} reads the global entropy",
            path.display()
        );
    }
    assert!(checked >= 8, "the scan ran on every slave source file");
}
