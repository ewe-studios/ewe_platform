//! The execution scenarios: output, exit, signals and the end of the caller.

use std::time::Duration;

use proptest::prelude::*;
use proptest::test_runner::RngSeed;

use foundation_sandbox::shared::error::SlaveErrorCode;
use foundation_sandbox::shared::rpc::messages::ExecSignalRequest;

use super::support::{
    drain, exec_request, exit_of, hex, limits, plain_prepare_work, process_exists, slave_code,
    started_pid, stderr_of, stdout_of, wait_until, SlaveProcess,
};

/// Run a program and get its output and exit code.
#[test]
fn exec_echo() {
    let slave = SlaveProcess::start("exec-echo", None, 4);
    slave.prepare(&plain_prepare_work());
    let stream = slave
        .caller
        .exec(exec_request(&hex(10), &["/bin/echo", "hi"]))
        .expect("the program starts");
    let events = drain(stream);
    assert!(started_pid(&events) > 0);
    assert_eq!(stdout_of(&events), b"hi\n");
    assert_eq!(exit_of(&events), (Some(0), None));
    let seqs: Vec<u64> = events.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2]);
}

/// A missing program does not start.
#[test]
fn exec_not_found() {
    let slave = SlaveProcess::start("exec-not-found", None, 4);
    slave.prepare(&plain_prepare_work());
    let error = slave
        .caller
        .exec_error(exec_request(&hex(11), &["/no/such/program"]));
    match error {
        foundation_sandbox::shared::error::SandboxError::SlaveError { code, errno, .. } => {
            assert_eq!(code, SlaveErrorCode::NotFound);
            assert_eq!(errno, Some(2));
        }
        other => panic!("not a slave error: {other:?}"),
    }
}

/// A file without an execute bit does not start.
#[test]
fn exec_not_executable() {
    let slave = SlaveProcess::start("exec-not-executable", None, 4);
    slave.prepare(&plain_prepare_work());
    let error = slave
        .caller
        .exec_error(exec_request(&hex(12), &["/etc/passwd"]));
    match error {
        foundation_sandbox::shared::error::SandboxError::SlaveError { code, errno, .. } => {
            assert_eq!(code, SlaveErrorCode::PermissionDenied);
            assert_eq!(errno, Some(13));
        }
        other => panic!("not a slave error: {other:?}"),
    }
}

/// Standard output and standard error stay separate.
#[test]
fn exec_stderr_is_separate() {
    let slave = SlaveProcess::start("exec-stderr", None, 4);
    slave.prepare(&plain_prepare_work());
    let stream = slave
        .caller
        .exec(exec_request(&hex(13), &["sh", "-c", "echo a; echo b >&2"]))
        .expect("the program starts");
    let events = drain(stream);
    assert_eq!(stdout_of(&events), b"a\n");
    assert_eq!(stderr_of(&events), b"b\n");
    assert_eq!(exit_of(&events), (Some(0), None));
}

/// The bytes of each stream are exactly what the program wrote.
#[test]
fn exec_output_is_exact() {
    let slave = SlaveProcess::start("exec-output-exact", None, 4);
    slave.prepare(&plain_prepare_work());
    let cases = [0_usize, 1, 7, 4095, 65536, 131_073];
    for (index, size) in cases.into_iter().enumerate() {
        let script = format!("yes a | head -c {size}; yes b | head -c {size} >&2");
        let stream = slave
            .caller
            .exec(exec_request(
                &hex(100 + index as u64),
                &["sh", "-c", &script],
            ))
            .expect("the program starts");
        let events = drain(stream);
        assert_eq!(
            stdout_of(&events),
            pattern(b"a\n", size),
            "stdout of {size}"
        );
        assert_eq!(
            stderr_of(&events),
            pattern(b"b\n", size),
            "stderr of {size}"
        );
        assert_eq!(exit_of(&events), (Some(0), None));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        rng_seed: RngSeed::Fixed(0x5eed_0001),
        cases: 12,
        .. ProptestConfig::default()
    })]

    /// The `Stdout` bytes are exact for a generated size.
    #[test]
    fn exec_output_property(size in 0_usize..200_000) {
        let slave = SlaveProcess::start("exec-output-property", None, 4);
        slave.prepare(&plain_prepare_work());
        let script = format!("yes a | head -c {size}");
        let stream = slave
            .caller
            .exec(exec_request(&hex(200), &["sh", "-c", &script]))
            .expect("the program starts");
        let events = drain(stream);
        prop_assert_eq!(stdout_of(&events), pattern(b"a\n", size));
        prop_assert_eq!(exit_of(&events), (Some(0), None));
    }
}

/// A signal reaches the whole process group.
#[test]
fn signal_group() {
    let slave = SlaveProcess::start("signal-group", None, 4);
    slave.prepare(&plain_prepare_work());
    let stream = slave
        .caller
        .exec(exec_request(&hex(20), &["sh", "-c", "sleep 60 & wait"]))
        .expect("the program starts");
    let (events, stream) = super::support::take_events(stream, 1);
    let pid = started_pid(&events);
    slave
        .caller
        .exec_signal(ExecSignalRequest {
            exec_id: hex(20),
            signal: 9,
        })
        .expect("the signal is accepted");
    let events = drain(stream);
    assert_eq!(exit_of(&events), (None, Some(9)));
    wait_until(|| !process_exists(pid), Duration::from_secs(2));
}

/// A signal number outside 1 to 64 is refused.
#[test]
fn signal_bad_number() {
    let slave = SlaveProcess::start("signal-bad", None, 4);
    slave.prepare(&plain_prepare_work());
    let stream = slave
        .caller
        .exec(exec_request(&hex(21), &["sleep", "60"]))
        .expect("the program starts");
    for signal in [0_i32, 65] {
        let error = slave
            .caller
            .exec_signal(ExecSignalRequest {
                exec_id: hex(21),
                signal,
            })
            .expect_err("the signal number is refused");
        assert_eq!(
            slave_code(&error).map(|(code, _)| code),
            Some(SlaveErrorCode::BadRequest.to_string())
        );
    }
    // The program sleeps for a minute; dropping the stream kills its group.
    drop(stream);
}

/// A signal after the program ended is accepted and does nothing.
#[test]
fn signal_after_exit() {
    let slave = SlaveProcess::start("signal-after", None, 4);
    slave.prepare(&plain_prepare_work());
    let stream = slave
        .caller
        .exec(exec_request(&hex(22), &["true"]))
        .expect("the program starts");
    let events = drain(stream);
    assert_eq!(exit_of(&events), (Some(0), None));
    slave
        .caller
        .exec_signal(ExecSignalRequest {
            exec_id: hex(22),
            signal: 15,
        })
        .expect("the signal is accepted");
}

/// A lost caller kills the program.
#[test]
fn stream_drop_kills() {
    let slave = SlaveProcess::start("stream-drop", None, 4);
    slave.prepare(&plain_prepare_work());
    let stream = slave
        .caller
        .exec(exec_request(&hex(30), &["sleep", "60"]))
        .expect("the program starts");
    let pid = {
        let (events, stream) = super::support::take_events(stream, 1);
        let pid = started_pid(&events);
        drop(stream);
        pid
    };
    wait_until(|| !process_exists(pid), Duration::from_secs(1));
    // The identifier is free again.
    let stream = slave
        .caller
        .exec(exec_request(&hex(30), &["true"]))
        .expect("the identifier is free");
    let _ = drain(stream);
}

/// The identifier is free after the program ended.
#[test]
fn exec_id_reuse_after_end() {
    let slave = SlaveProcess::start("exec-id-reuse", None, 4);
    slave.prepare(&plain_prepare_work());
    let events = drain(
        slave
            .caller
            .exec(exec_request(&hex(31), &["true"]))
            .expect("the first run"),
    );
    assert_eq!(exit_of(&events), (Some(0), None));
    let events = drain(
        slave
            .caller
            .exec(exec_request(&hex(31), &["true"]))
            .expect("the second run"),
    );
    assert_eq!(exit_of(&events), (Some(0), None));
}

/// Two sessions with one identifier never run at the same time.
#[test]
fn exec_id_conflict() {
    let slave = SlaveProcess::start("exec-id-conflict", None, 4);
    slave.prepare(&plain_prepare_work());
    let stream = slave
        .caller
        .exec(exec_request(&hex(32), &["sleep", "60"]))
        .expect("the first run");
    let error = slave.caller.exec_error(exec_request(&hex(32), &["true"]));
    assert!(matches!(
        error,
        foundation_sandbox::shared::error::SandboxError::SlaveError {
            code: SlaveErrorCode::AlreadyExists,
            ..
        }
    ));
    drop(stream);
}

/// A background child does not hold the exit.
#[test]
fn drain_timeout() {
    let slave = SlaveProcess::start("drain-timeout", None, 4);
    let mut prepare = plain_prepare_work();
    prepare.limits.output_drain_timeout_ms = 500;
    slave.prepare(&prepare);
    let clock = foundation_context::Ctx::host();
    let start = clock.clock().now();
    let stream = slave
        .caller
        .exec(exec_request(&hex(40), &["sh", "-c", "sleep 30 & echo x"]))
        .expect("the program starts");
    let events = drain(stream);
    let elapsed = clock.clock().now().saturating_duration_since(start);
    assert!(
        elapsed < Duration::from_secs(2),
        "the Exit event took {elapsed:?}"
    );
    assert_eq!(stdout_of(&events), b"x\n");
    assert_eq!(exit_of(&events), (Some(0), None));
}

/// A slow reader loses nothing.
#[test]
fn slow_reader_exact() {
    let slave = SlaveProcess::start("slow-reader", None, 4);
    slave.prepare(&plain_prepare_work());
    let mut stream = slave
        .caller
        .exec(exec_request(
            &hex(41),
            &["head", "-c", "1048576", "/dev/zero"],
        ))
        .expect("the program starts");
    let mut total = 0_usize;
    let mut events = Vec::new();
    loop {
        std::thread::sleep(Duration::from_millis(50));
        let (next, rest) = super::support::take_events(stream, 1);
        stream = rest;
        if next.is_empty() {
            break;
        }
        total += stdout_of(&next).len();
        events.extend(next);
        if total >= 1_048_576 {
            let rest = drain(stream);
            total += stdout_of(&rest).len();
            events.extend(rest);
            break;
        }
    }
    assert_eq!(total, 1_048_576);
    assert_eq!(exit_of(&events), (Some(0), None));
}

/// Too many sessions are refused.
#[test]
fn too_many_sessions() {
    let slave = SlaveProcess::start("too-many", None, 4);
    let mut prepare = plain_prepare_work();
    prepare.limits = limits();
    prepare.limits.max_exec_sessions = 2;
    slave.prepare(&prepare);
    let first = slave
        .caller
        .exec(exec_request(&hex(50), &["sleep", "60"]))
        .expect("the first session");
    let second = slave
        .caller
        .exec(exec_request(&hex(51), &["sleep", "60"]))
        .expect("the second session");
    let error = slave.caller.exec_error(exec_request(&hex(52), &["true"]));
    assert_eq!(
        error,
        foundation_sandbox::shared::error::SandboxError::TooManySessions
    );
    // The two sessions continue.
    let (events, _) = super::support::take_events(first, 1);
    assert!(started_pid(&events) > 0);
    let (events, _) = super::support::take_events(second, 1);
    assert!(started_pid(&events) > 0);
}

/// The bytes of `pattern` repeated to `size` bytes.
fn pattern(pattern: &[u8], size: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(size);
    while bytes.len() < size {
        let take = (size - bytes.len()).min(pattern.len());
        bytes.extend_from_slice(&pattern[..take]);
    }
    bytes
}
