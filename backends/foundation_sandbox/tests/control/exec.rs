//! The handle behaviour: events, input, capture, files and time limits.

use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{execute, from_future, StreamIteratorExt};
use foundation_errstacks::ErrorTrace;
use foundation_sandbox::shared::command::{Command, ExecEvent, ExitStatus, StdinMode};
use foundation_sandbox::shared::control::testing::{spec, ScriptedSlave, SimDriver, SubstrateOp};
use foundation_sandbox::shared::control::{ManagerConfig, SandboxProvider};
use foundation_sandbox::shared::driver::{SandboxDriver, SubstrateExit};
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::rpc::messages::{ExecEventKind, ExecEventMsg, FileChunk};
use futures::StreamExt;

use super::support;

fn started() -> ExecEventKind {
    ExecEventKind::Started { pid: 42 }
}

fn stdout(bytes: &[u8]) -> ExecEventKind {
    ExecEventKind::Stdout {
        data: bytes.to_vec(),
    }
}

fn exit(code: i32) -> ExecEventKind {
    ExecEventKind::Exit {
        code: Some(code),
        signal: None,
    }
}

fn create(
    ctx: foundation_context::Ctx,
    slave: Arc<ScriptedSlave>,
    driver: Arc<SimDriver>,
) -> (
    foundation_sandbox::shared::control::SandboxManager,
    Arc<ScriptedSlave>,
    Arc<SimDriver>,
) {
    let (manager, _store) = support::simple_manager(ctx, Arc::clone(&driver), Arc::clone(&slave));
    (manager, slave, driver)
}

#[test]
fn events_in_order() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(20);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_exec_events(vec![
        started(),
        stdout(b"a"),
        ExecEventKind::Stderr {
            data: b"b".to_vec(),
        },
        stdout(b"c"),
        exit(0),
    ]);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _slave, _driver) = create(ctx, Arc::clone(&slave), driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let mut exec = support::block_on(handle.exec(Command::new(["prog"]))).expect("exec");
    let pid = exec.pid();
    let events = support::block_on(async move {
        let mut stream = exec.events();
        let mut events = Vec::new();
        while let Some(item) = stream.next().await {
            events.push(item.expect("an event"));
        }
        events
    });
    assert_eq!(
        events,
        vec![
            ExecEvent::Stdout(b"a".to_vec()),
            ExecEvent::Stderr(b"b".to_vec()),
            ExecEvent::Stdout(b"c".to_vec()),
            ExecEvent::Exit(ExitStatus::code(0)),
        ]
    );
    assert_eq!(pid, 42);
}

#[test]
fn event_gap_ends_the_stream() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(21);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_exec_messages(vec![
        ExecEventMsg {
            seq: 0,
            event: started(),
        },
        ExecEventMsg {
            seq: 1,
            event: stdout(b"a"),
        },
        ExecEventMsg {
            seq: 3,
            event: stdout(b"c"),
        },
    ]);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _slave, _driver) = create(ctx, slave, driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let mut exec = support::block_on(handle.exec(Command::new(["prog"]))).expect("exec");
    let mut items = support::block_on(async move {
        let mut stream = exec.events();
        let mut items = Vec::new();
        while let Some(item) = stream.next().await {
            items.push(item);
        }
        items
    });
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0].as_ref().expect("first"),
        &ExecEvent::Stdout(b"a".to_vec())
    );
    let error = support::expect_error(items.remove(1), "gap");
    assert!(matches!(error, SandboxError::StreamLost));
}

#[test]
fn stdin_retry_on_full() {
    let _serial = support::serial();
    let (ctx, _clock, _timer) = support::recording_ctx(22);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_exec_events(vec![started(), exit(0)]);
    slave.with_input_queue_full(1, 2);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _slave, _driver) = create(ctx, Arc::clone(&slave), driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let mut command = Command::new(["cat"]);
    command.stdin = StdinMode::Piped;
    let mut exec = support::block_on(handle.exec(command)).expect("exec");
    let mut stdin = exec.stdin().expect("stdin");
    support::block_on(async {
        stdin.write(b"a").await.expect("first");
        stdin.write(b"b").await.expect("second");
        stdin.write(b"c").await.expect("third");
        stdin.close().await.expect("close");
    });
    let applied = slave.applied_input();
    assert_eq!(applied.len(), 4);
    for (index, record) in applied.iter().enumerate() {
        assert_eq!(record.seq, u64::try_from(index).unwrap_or(u64::MAX));
    }
    assert_eq!(applied[0].data, b"a");
    assert_eq!(applied[1].data, b"b");
    assert_eq!(applied[2].data, b"c");
    assert!(applied[3].eof);
    assert_eq!(slave.call_count("exec_input"), 6);
}

#[test]
fn stdin_retry_on_transport_error() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(23);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_exec_events(vec![started(), exit(0)]);
    slave.with_input_transport_error(0, 1);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _slave, _driver) = create(ctx, Arc::clone(&slave), driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let mut command = Command::new(["cat"]);
    command.stdin = StdinMode::Piped;
    let mut exec = support::block_on(handle.exec(command)).expect("exec");
    let mut stdin = exec.stdin().expect("stdin");
    support::block_on(async {
        stdin.write(b"a").await.expect("write");
        stdin.close().await.expect("close");
    });
    let applied = slave.applied_input();
    assert_eq!(applied.len(), 2);
    assert_eq!(applied[0].seq, 0);
    assert_eq!(applied[0].data, b"a");
    assert!(applied[1].eof);
}

#[test]
fn stdin_bytes_mode() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(24);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_exec_events(vec![started(), exit(0)]);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _slave, _driver) = create(ctx, Arc::clone(&slave), driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let mut command = Command::new(["cat"]);
    command.stdin = StdinMode::Bytes(b"hello".to_vec());
    let _exec = support::block_on(handle.exec(command)).expect("exec");
    let mut attempts = 0;
    while slave.applied_input().len() < 2 && attempts < 10_000 {
        std::thread::yield_now();
        attempts += 1;
    }
    let applied = slave.applied_input();
    assert_eq!(applied.len(), 2);
    assert_eq!(applied[0].data, b"hello");
    assert!(applied[1].eof);
}

#[test]
fn stdin_gives_up() {
    let _serial = support::serial();
    let (ctx, clock) = support::ctx(25);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_exec_events(vec![started(), exit(0)]);
    slave.with_input_queue_full(0, usize::MAX);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _slave, _driver) = create(ctx, Arc::clone(&slave), driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let mut command = Command::new(["cat"]);
    command.stdin = StdinMode::Piped;
    let mut exec = support::block_on(handle.exec(command)).expect("exec");
    let mut stdin = exec.stdin().expect("stdin");
    let task = execute(
        from_future(async move {
            let _ = stdin.write(b"x").await;
        }),
        None,
    )
    .expect("schedule");
    let mut attempts = 0;
    while slave.call_count("exec_input") == 0 && attempts < 10_000 {
        std::thread::yield_now();
        attempts += 1;
    }
    clock.advance(Duration::from_secs(31));
    support::block_on(task.into_ready_future());
    assert!(slave.applied_input().is_empty());
}

#[test]
fn command_timeout() {
    let _serial = support::serial();
    let (ctx, clock) = support::ctx(26);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_gated_exec(vec![started()], exit(137));
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _slave, _driver) = create(ctx, Arc::clone(&slave), driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let mut command = Command::new(["sleep"]);
    command.timeout = Some(Duration::from_secs(5));
    let exec = support::block_on(handle.exec(command)).expect("exec");
    clock.advance(Duration::from_secs(5));
    let mut attempts = 0;
    while slave.signals().is_empty() && attempts < 10_000 {
        std::thread::yield_now();
        attempts += 1;
    }
    assert_eq!(slave.signals(), vec![9]);
    let error = support::expect_error(support::block_on(exec.wait()), "timeout");
    assert!(matches!(error, SandboxError::ExecTimeout), "{error:?}");
}

#[test]
fn run_truncates() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(27);
    let slave = Arc::new(ScriptedSlave::new());
    let big = vec![b'x'; 1024 * 1024];
    slave.with_exec_events(vec![started(), stdout(&big), exit(0)]);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _slave, _driver) = create(ctx, slave, driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let output = support::block_on(handle.run(Command::new(["noise"]))).expect("run");
    assert_eq!(output.stdout.len(), 1024 * 1024);
    assert!(!output.stdout_truncated);
    assert_eq!(output.status, ExitStatus::code(0));
}

#[test]
fn run_with_limit_truncates() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(28);
    let slave = Arc::new(ScriptedSlave::new());
    let big = vec![b'x'; 4096];
    slave.with_exec_events(vec![started(), stdout(&big), exit(0)]);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let store = Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::new());
    let limits =
        foundation_sandbox::shared::limits::SandboxLimits::default().with_max_captured_output(1024);
    let config = ManagerConfig::default().with_limits(limits);
    let manager = support::manager(
        ctx,
        vec![Arc::clone(&driver) as Arc<dyn SandboxDriver>],
        slave,
        config,
        store,
    );
    let handle = support::block_on(manager.create(spec())).expect("create");
    let output = support::block_on(handle.run(Command::new(["noise"]))).expect("run");
    assert_eq!(output.stdout.len(), 1024);
    assert!(output.stdout_truncated);
    assert_eq!(output.status, ExitStatus::code(0));
}

#[test]
fn run_with_timeout() {
    let _serial = support::serial();
    let (ctx, _clock, timer) = support::recording_ctx(29);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_gated_exec(vec![started()], exit(137));
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _slave, _driver) = create(ctx, Arc::clone(&slave), driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    // The timer answers the command deadline at once, so the timer task sends
    // the signal and the gate releases without a clock move.
    timer.resolve_long_waits();
    let mut command = Command::new(["sleep"]);
    command.timeout = Some(Duration::from_secs(5));
    let error = support::expect_error(support::block_on(handle.run(command)), "timeout");
    assert!(matches!(error, SandboxError::ExecTimeout), "{error:?}");
}

fn chunks(
    count: usize,
    size: usize,
) -> impl futures::Stream<Item = Result<Vec<u8>, ErrorTrace<SandboxError>>> + Send + 'static {
    futures::stream::iter(
        (0..count)
            .map(|_| Ok(vec![b'a'; size]))
            .collect::<Vec<Result<Vec<u8>, ErrorTrace<SandboxError>>>>(),
    )
}

#[test]
fn write_file_abort_on_error() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(30);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_chunk_failure(
        2,
        SandboxError::Io {
            op: "chunk".to_string(),
            message: "the disk is full".to_string(),
        },
    );
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _slave, _driver) = create(ctx, Arc::clone(&slave), driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let error = support::expect_error(
        support::block_on(handle.write_file("/target", 0o644, chunks(3, 64 * 1024))),
        "chunk fails",
    );
    assert!(matches!(error, SandboxError::Io { .. }));
    assert_eq!(slave.call_count("file_write_abort"), 1);
}

#[test]
fn write_file_abort_on_drop() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(31);
    let slave = Arc::new(ScriptedSlave::new());
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _slave, _driver) = create(ctx, Arc::clone(&slave), driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let data = futures::stream::unfold(0_u32, |step| async move {
        if step == 0 {
            Some((Ok(vec![b'a'; 64 * 1024]), 1_u32))
        } else {
            std::future::pending::<()>().await;
            None
        }
    });
    let write = Box::pin(handle.write_file("/target", 0o644, data));
    let waiter = futures::future::poll_fn(|context| {
        if slave.call_count("file_write_chunk") > 0 {
            std::task::Poll::Ready(())
        } else {
            context.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    });
    let _ = support::block_on(futures::future::select(write, waiter));
    let mut attempts = 0;
    while slave.call_count("file_write_abort") == 0 && attempts < 10_000 {
        std::thread::yield_now();
        attempts += 1;
    }
    assert_eq!(slave.call_count("file_write_abort"), 1);
}

#[test]
fn read_file_gap() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(32);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_read_chunks(vec![
        FileChunk {
            offset: 0,
            data: vec![b'a'; 100],
            eof: false,
        },
        FileChunk {
            offset: 200,
            data: vec![b'b'; 10],
            eof: true,
        },
    ]);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _slave, _driver) = create(ctx, slave, driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let stream = support::block_on(handle.read_file("/file")).expect("read");
    let mut items = support::block_on(async move {
        let mut stream = stream;
        let mut items = Vec::new();
        while let Some(item) = stream.next().await {
            items.push(item);
        }
        items
    });
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].as_ref().expect("first"), &vec![b'a'; 100]);
    let error = support::expect_error(items.remove(1), "gap");
    assert!(matches!(error, SandboxError::StreamLost));
}

#[test]
fn drop_exec_handle_closes_the_session() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(33);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_exec_events(vec![started(), exit(0)]);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _slave, _driver) = create(ctx, Arc::clone(&slave), driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let exec = support::block_on(handle.exec(Command::new(["tail"]))).expect("exec");
    assert_eq!(slave.call_count("exec"), 1);
    drop(exec);
    support::block_on(handle.kill()).expect("kill");
}

#[test]
fn substrate_exit_ends_the_command_stream() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(34);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_exec_events(vec![started()]);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let substrate = driver.substrate();
    let (manager, _slave, _driver) = create(ctx, slave, driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let record = manager.inner().record(handle.id()).expect("record");
    let mut exec = support::block_on(handle.exec(Command::new(["tail"]))).expect("exec");
    substrate.exit_now(SubstrateExit {
        code: Some(137),
        signal: None,
        forced: false,
    });
    let items = support::block_on(async move {
        let mut stream = exec.events();
        let mut items = Vec::new();
        while let Some(item) = stream.next().await {
            items.push(item);
        }
        items
    });
    assert!(items.iter().any(Result::is_err));
    support::block_on(record.stop.wait());
    assert_eq!(
        record.info().state,
        foundation_sandbox::shared::control::SandboxState::Stopped
    );
    assert!(support::block_on(manager.list()).expect("list").is_empty());
    assert!(substrate.op_count(SubstrateOp::Cleanup) >= 1);
}

#[test]
fn stdin_chunk_sizes_are_bounded() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(35);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_exec_events(vec![started(), exit(0)]);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _slave, _driver) = create(ctx, Arc::clone(&slave), driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let mut command = Command::new(["cat"]);
    command.stdin = StdinMode::Piped;
    let mut exec = support::block_on(handle.exec(command)).expect("exec");
    let mut stdin = exec.stdin().expect("stdin");
    let data = vec![b'z'; 200 * 1024];
    support::block_on(async {
        stdin.write(&data).await.expect("write");
        stdin.close().await.expect("close");
    });
    let applied = slave.applied_input();
    let chunk = 64 * 1024;
    assert_eq!(applied.len(), 5);
    assert_eq!(applied[0].data.len(), chunk);
    assert!(applied[4].eof);
}

/// The first stream error is reported as it is, not as `StreamLost`.
///
/// A bare `StreamLost` hides the transport or slave cause, so the runner could
/// not name the failing stage (issue #1125).
#[test]
fn the_first_exec_stream_error_keeps_its_cause() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(45);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_exec_stream_error(
        Vec::new(),
        SandboxError::Io {
            op: "exec stream".to_string(),
            message: "the pipe broke".to_string(),
        },
    );
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _slave, _driver) = create(ctx, slave, driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let error = support::expect_error(
        support::block_on(handle.run(Command::new(["prog"]))),
        "the run fails",
    );
    assert!(
        matches!(error, SandboxError::Io { ref op, .. } if op == "exec stream"),
        "got {error:?}"
    );
}

/// A stream error after the start is reported as it is, not as `StreamLost`.
#[test]
fn a_later_exec_stream_error_keeps_its_cause() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(46);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_exec_stream_error(
        vec![
            ExecEventMsg {
                seq: 0,
                event: started(),
            },
            ExecEventMsg {
                seq: 1,
                event: stdout(b"a"),
            },
        ],
        SandboxError::Io {
            op: "exec stream".to_string(),
            message: "the pipe broke".to_string(),
        },
    );
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let (manager, _slave, _driver) = create(ctx, slave, driver);
    let handle = support::block_on(manager.create(spec())).expect("create");
    let error = support::expect_error(
        support::block_on(handle.run(Command::new(["prog"]))),
        "the run fails",
    );
    assert!(
        matches!(error, SandboxError::Io { ref op, .. } if op == "exec stream"),
        "got {error:?}"
    );
}
