//! F02 readiness tests: drive the `ReadinessTask` for each strategy to its
//! single result via the valtron executor.

use std::io::Write;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::{collect_one, execute, valtron_test};
use foundation_nativeapis::daemon::{
    DaemonId, OutputLine, ReadinessConfig, ReadinessTask, StreamKind,
};

fn drive(task: ReadinessTask) -> Option<Result<(), foundation_nativeapis::daemon::ReadinessTimeout>>
{
    let stream = execute(task, None).expect("schedule readiness task");
    collect_one(stream)
}

fn queue() -> Arc<ConcurrentQueue<OutputLine>> {
    Arc::new(ConcurrentQueue::unbounded())
}

#[valtron_test]
fn immediate_is_ready_at_once() {
    let task = ReadinessTask::new(
        DaemonId::in_default("x"),
        ReadinessConfig::Immediate,
        queue(),
        Duration::from_secs(5),
    );
    assert_eq!(drive(task), Some(Ok(())));
}

#[valtron_test]
fn output_matches_regex_line() {
    let q = queue();
    q.push(OutputLine {
        text: "server listening on 8080".into(),
        stream: StreamKind::Stdout,
    })
    .unwrap();

    let task = ReadinessTask::new(
        DaemonId::in_default("x"),
        ReadinessConfig::output(r"listening on \d+"),
        q,
        Duration::from_secs(5),
    );
    assert_eq!(drive(task), Some(Ok(())));
}

#[valtron_test]
fn port_ready_when_listener_bound() {
    // Keep the listener alive for the duration so the port stays open.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();

    let task = ReadinessTask::new(
        DaemonId::in_default("x"),
        ReadinessConfig::Port(port),
        queue(),
        Duration::from_secs(5),
    );
    assert_eq!(drive(task), Some(Ok(())));
    drop(listener);
}

#[valtron_test]
fn port_times_out_when_nothing_listens() {
    // Bind then release to obtain a very-likely-closed port number.
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        l.local_addr().unwrap().port()
    };

    let task = ReadinessTask::new(
        DaemonId::in_default("x"),
        ReadinessConfig::Port(port),
        queue(),
        Duration::from_millis(400),
    );
    let result = drive(task);
    assert!(
        matches!(result, Some(Err(_))),
        "expected timeout, got {result:?}"
    );
}

#[valtron_test]
fn http_ready_on_2xx() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();

    // One-shot server that answers the probe with 200.
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        }
    });

    let task = ReadinessTask::new(
        DaemonId::in_default("x"),
        ReadinessConfig::Http(format!("http://127.0.0.1:{port}/health")),
        queue(),
        Duration::from_secs(5),
    );
    assert_eq!(drive(task), Some(Ok(())));
}
