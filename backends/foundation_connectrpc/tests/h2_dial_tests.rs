//! Feature 127 — the H2 client dials without blocking a valtron worker.
//!
//! WHY: `H2Transport::open` used to call `TcpStream::connect_timeout`, which
//! held the calling worker for the whole kernel connect. A dial to an address
//! that never answers held it for the connect budget (10 s by default), and
//! every task that worker hosted waited too. The h2 handshake had the same
//! defect and feature 121 moved it into the pump; this feature moves the dial.
//!
//! WHAT: four tests drive a real `H2Transport`:
//! - `a_closed_port_fails_on_the_head_pipe`: a refused dial reaches the caller
//!   as `Err(TransportError::Connect)` on the response head, and `open` returns
//!   at once. The old code returned the error from `open` instead.
//! - `a_blackholed_dial_does_not_block_the_worker`: a non-routable address with
//!   a 300 ms connect timeout. `open` returns in under 50 ms and an unrelated
//!   valtron task finishes while the dial is still in flight. The old code
//!   blocks `open` for the whole 300 ms.
//! - `a_unix_socket_dial_still_connects`: the Unix path connects.
//! - `a_unix_dial_to_a_full_listener_does_not_block`: a saturated Unix listener
//!   does not hold the worker (issue #270).
//!
//! HOW: `#[valtron_test]` provides the pool the pump runs on. `initialize_pool`
//! serializes pool users, so the tests in this binary do not run at once. A
//! blackhole address is environment dependent: the test reports a skip when the
//! environment refuses it at once.

// The non-blocking dial parks on the shared reactor, which exists on unix only.
// Off unix the pump keeps the fixed `POLL_DELAY` fallback, so these tests do
// not apply.
#![cfg(unix)]

use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use futures::StreamExt;

use foundation_connectrpc::{H2Transport, Transport, TransportError};
use foundation_core::url::Uri;
use foundation_core::valtron::{
    execute, from_future, sleep_async, valtron_test, StreamIteratorExt,
};
use foundation_netio::shared::http::{
    Proto, RequestDescriptor, SimpleHeaders, SimpleMethod, SimpleUrl,
};

/// The longest `open` may take when it does not wait for the peer.
const MAX_OPEN_TIME: Duration = Duration::from_millis(50);

/// The connect budget of the blackhole test.
const CONNECT_TIMEOUT: Duration = Duration::from_millis(300);

/// How long the unrelated task waits before it records its completion.
const UNRELATED_TASK_DELAY: Duration = Duration::from_millis(50);

/// A non-routable address that does not answer a connect.
const BLACKHOLE_URL: &str = "http://10.255.255.1:9/t.Svc/Unary";

fn request_descriptor(url: &str) -> RequestDescriptor {
    RequestDescriptor {
        proto: Proto::HTTP20,
        request_url: SimpleUrl::url_only(url.to_string()),
        request_uri: Uri::parse(url).expect("valid uri"),
        headers: SimpleHeaders::new(),
        method: SimpleMethod::POST,
    }
}

/// Whether the environment refused the blackhole address at once.
///
/// A sandbox that has no route to the address fails the first `connect` with
/// one of these errors. The test then reports a skip instead of a false pass.
fn refused_at_once(err: &TransportError) -> bool {
    let TransportError::Connect(inner) = err else {
        return false;
    };
    let Some(io) = inner.downcast_ref::<std::io::Error>() else {
        return false;
    };
    matches!(
        io.raw_os_error(),
        Some(libc::EACCES | libc::ENETUNREACH | libc::EHOSTUNREACH)
    )
}

/// A dial to a closed port must reach the caller as a connect error on the
/// response head. `open` itself must not wait.
#[valtron_test(timeout = 5_000)]
async fn a_closed_port_fails_on_the_head_pipe() {
    // Bind a listener, note its port, then close it so nothing accepts.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    drop(listener);

    let transport = H2Transport::new();
    let url = format!("http://{addr}/t.Svc/Unary");

    let started = Instant::now();
    let mut call = transport
        .open(request_descriptor(&url))
        .expect("open starts the dial; a refused dial rides the head");
    let open_time = started.elapsed();
    assert!(
        open_time < MAX_OPEN_TIME,
        "open waited {open_time:?} for the closed port (max {MAX_OPEN_TIME:?})"
    );

    let head_started = Instant::now();
    let head = call.head.next().await.expect("the call publishes a head");
    assert!(
        head_started.elapsed() < Duration::from_secs(2),
        "the refused dial took {:?} to reach the head",
        head_started.elapsed()
    );
    match head {
        Err(TransportError::Connect(_)) => {}
        other => panic!("a refused dial must reach the caller as Connect, got {other:?}"),
    }
}

/// A dial to an address that never answers must not hold the worker.
#[valtron_test(timeout = 5_000)]
async fn a_blackholed_dial_does_not_block_the_worker() {
    let transport = H2Transport::new().with_connect_timeout(CONNECT_TIMEOUT);

    // The unrelated task records when it runs. It is scheduled before the dial,
    // so if `open` held this worker the record would arrive only after `open`.
    let ran_at: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    let task_ran_at = Arc::clone(&ran_at);
    let unrelated = execute(
        from_future(async move {
            sleep_async(UNRELATED_TASK_DELAY).await;
            *task_ran_at.lock().expect("task lock") = Some(Instant::now());
        }),
        None,
    )
    .expect("schedule the unrelated task");

    let started = Instant::now();
    let opened = transport.open(request_descriptor(BLACKHOLE_URL));
    let open_time = started.elapsed();

    let mut call = match opened {
        Ok(call) => call,
        Err(err) if refused_at_once(&err) => {
            eprintln!("skip: the environment refused the blackhole address at once: {err}");
            let _ = unrelated.into_collect_future().await;
            return;
        }
        Err(err) => panic!("a blackhole dial must not fail inside open after {open_time:?}: {err}"),
    };
    assert!(
        open_time < MAX_OPEN_TIME,
        "open waited {open_time:?} on a dial that cannot answer (max {MAX_OPEN_TIME:?})"
    );

    // Park on the head. The dial fails at the connect timeout; the unrelated
    // task must finish well before that.
    let head = call.head.next().await.expect("the call publishes a head");
    let head_resolved = Instant::now();

    // An environment that rejects the address asynchronously cannot show the
    // overlap. Report a skip instead of a false result.
    if let Err(ref err) = head {
        if head_resolved.duration_since(started) < UNRELATED_TASK_DELAY && refused_at_once(err) {
            eprintln!(
                "skip: the environment rejected the blackhole address before the unrelated task ran: {err}"
            );
            let _ = unrelated.into_collect_future().await;
            return;
        }
    }

    let ran = *ran_at.lock().expect("task lock");
    let ran = ran.expect("the unrelated task must run while the dial is in flight");
    assert!(
        ran < head_resolved,
        "the unrelated task finished {:?} after the dial resolved",
        ran.duration_since(head_resolved)
    );
    assert!(
        ran.duration_since(started) < CONNECT_TIMEOUT,
        "the unrelated task waited {:?}, the dial budget is {CONNECT_TIMEOUT:?}",
        ran.duration_since(started)
    );

    match head {
        Err(TransportError::Connect(_)) => {}
        other => panic!("a dial that never answers must fail as Connect, got {other:?}"),
    }

    let _ = unrelated.into_collect_future().await;
}

/// The Unix domain socket path still connects.
#[valtron_test(timeout = 5_000)]
async fn a_unix_socket_dial_still_connects() {
    use std::os::unix::net::UnixListener;
    use std::sync::mpsc;

    let pid = std::process::id();
    let tid = std::thread::current().id();
    let path = std::env::temp_dir().join(format!("ewe-h2-dial-{pid}-{tid:?}.sock"));
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).expect("bind the unix socket");

    let (accepted_tx, accepted_rx) = mpsc::channel();
    let peer = thread::spawn(move || {
        let (socket, _) = listener.accept().expect("accept the unix dial");
        accepted_tx.send(()).expect("report the accept");
        // Hold the socket open until the client drops the call.
        thread::sleep(Duration::from_millis(300));
        drop(socket);
    });

    let transport = H2Transport::unix(&path).with_handshake_timeout(Duration::from_millis(200));
    let call = transport
        .open(request_descriptor("http://localhost/t.Svc/Unary"))
        .expect("a unix dial connects");

    let mut accepted = false;
    for _ in 0..100 {
        if accepted_rx.try_recv().is_ok() {
            accepted = true;
            break;
        }
        sleep_async(Duration::from_millis(5)).await;
    }
    assert!(accepted, "the peer accepted the unix dial");

    drop(call);
    peer.join().expect("peer thread");
    let _ = std::fs::remove_file(&path);
}

/// A Unix dial to a listener whose queue is full must not block the caller
/// (issue #270).
///
/// The listener is saturated with blocking connects first. One of them is still
/// waiting when the client dials, so the kernel cannot accept the new
/// connection at once. A blocking `UnixStream::connect` would then hold the
/// worker; the non-blocking dial returns from `open` at once and the pump
/// retries the connect.
#[valtron_test(timeout = 10_000)]
async fn a_unix_dial_to_a_full_listener_does_not_block() {
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc;

    let pid = std::process::id();
    let tid = std::thread::current().id();
    let path = std::env::temp_dir().join(format!("ewe-h2-dial-full-{pid}-{tid:?}.sock"));
    let _ = std::fs::remove_file(&path);

    // A backlog of zero keeps almost nothing queued, so a burst of connects
    // saturates the listener. The sockets stay open, so the queue stays full.
    let listener = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)
        .expect("create the listener socket");
    let addr = socket2::SockAddr::unix(&path).expect("socket address");
    listener.bind(&addr).expect("bind");
    listener.listen(0).expect("listen");

    let mut fillers = Vec::new();
    let mut results = Vec::new();
    for _ in 0..4 {
        let (tx, rx) = mpsc::channel::<Option<UnixStream>>();
        let connect_path = path.clone();
        fillers.push(thread::spawn(move || {
            let _ = tx.send(UnixStream::connect(&connect_path).ok());
        }));
        results.push(rx);
    }
    thread::sleep(Duration::from_millis(50));

    // Keep every completed connect's socket open, so its queue slot stays
    // taken. A channel with no value yet is a connect the kernel made wait.
    let mut held = Vec::new();
    let mut blocked = false;
    for rx in &results {
        match rx.try_recv() {
            Ok(Some(stream)) => held.push(stream),
            Ok(None) => {}
            Err(_) => blocked = true,
        }
    }
    if !blocked {
        drop(held);
        drop(listener);
        for filler in fillers {
            let _ = filler.join();
        }
        let _ = std::fs::remove_file(&path);
        eprintln!("skip: the listener accepted every connect at once");
        return;
    }

    let transport = H2Transport::unix(&path).with_connect_timeout(Duration::from_millis(200));
    let started = Instant::now();
    let opened = transport.open(request_descriptor("http://localhost/t.Svc/Unary"));
    let open_time = started.elapsed();
    assert!(
        open_time < MAX_OPEN_TIME,
        "open waited {open_time:?} on a full listener (max {MAX_OPEN_TIME:?})"
    );
    match opened {
        Ok(call) => drop(call),
        // A refusal the kernel reports at once is also non-blocking.
        Err(TransportError::Connect(_)) => {}
        Err(other) => panic!("unexpected open error: {other:?}"),
    }

    // Release the queue, then let the blocked connects finish.
    drop(held);
    drop(listener);
    for filler in fillers {
        let _ = filler.join();
    }
    let _ = std::fs::remove_file(&path);
}
