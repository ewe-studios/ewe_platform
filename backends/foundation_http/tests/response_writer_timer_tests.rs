//! Spec 70 feature 63 — the response writer's idle timer measures a stall
//! inside one write or flush, not how long the writer has existed.
//!
//! Both tests drive a real [`ResponseWriter`] over an in-memory overlay stream
//! with a small send buffer. The stream reports write readiness through the same
//! `ConnWaker` path as the `WireGuard` overlay, so the tests are deterministic on
//! every platform: no kernel socket buffer sizes and no kernel pacing rules.
//! Each test fails on the feature-61 writer:
//!
//! - `idle_stream_then_blocked_write_does_not_time_out`: the timer started when
//!   the writer was created, so the first blocked write after a long idle timed
//!   out at once.
//! - `buffered_flush_restarts_idle_timer`: `flush` did not restart the timer when
//!   the stream's buffer shrank, so a flush longer than `send_timeout` was cut
//!   off while it was making progress.
#![cfg(all(feature = "multi", unix))]

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::native::writer::{ResponseWriter, WriterConfig};
use foundation_netio::native::connection::{ConnWaker, OverlayReadWrite};
use foundation_netio::netcap::{Connection, RawStream};

// ---------------------------------------------------------------------------
// An in-memory overlay stream with a small send buffer

/// The shared send buffer between the writer and the test client.
struct FakeState {
    /// Bytes the writer queued and the client has not read.
    send: VecDeque<u8>,
    /// The most the writer may queue before it blocks.
    capacity: usize,
    /// The writer's one-shot write-readiness waker.
    write_waker: Option<ConnWaker>,
}

/// The writer's end of the fake transport. It blocks once the send buffer is
/// full, and fires the registered waker when the client frees space.
#[derive(Clone)]
struct FakeSocket {
    state: Arc<Mutex<FakeState>>,
}

impl std::fmt::Debug for FakeSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeSocket").finish_non_exhaustive()
    }
}

impl Read for FakeSocket {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Err(io::ErrorKind::WouldBlock.into())
    }
}

impl Write for FakeSocket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut state = self.state.lock().expect("fake socket lock");
        let free = state.capacity.saturating_sub(state.send.len());
        if free == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let take = free.min(buf.len());
        state.send.extend(&buf[..take]);
        Ok(take)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl OverlayReadWrite for FakeSocket {
    fn clone_box(&self) -> Box<dyn OverlayReadWrite> {
        Box::new(self.clone())
    }

    fn local_addr(&self) -> std::net::SocketAddr {
        "127.0.0.1:1".parse().expect("literal address")
    }

    fn peer_addr(&self) -> std::net::SocketAddr {
        "127.0.0.1:2".parse().expect("literal address")
    }

    fn set_read_waker(&self, _waker: ConnWaker) {}

    fn set_write_waker(&self, waker: ConnWaker) {
        self.state.lock().expect("fake socket lock").write_waker = Some(waker);
    }

    fn readable(&self) -> bool {
        false
    }

    fn writable(&self) -> bool {
        let state = self.state.lock().expect("fake socket lock");
        state.send.len() < state.capacity
    }
}

/// The client's end of the fake transport.
struct FakeClient {
    state: Arc<Mutex<FakeState>>,
}

impl FakeClient {
    /// Fill the send buffer, as a previous response that the client has not read.
    fn fill(&self) {
        let mut state = self.state.lock().expect("fake socket lock");
        while state.send.len() < state.capacity {
            state.send.push_back(0);
        }
    }

    /// Read up to `max` bytes, then fire the writer's readiness waker.
    fn read(&self, max: usize) -> Vec<u8> {
        let (bytes, waker) = {
            let mut state = self.state.lock().expect("fake socket lock");
            let take = max.min(state.send.len());
            let bytes: Vec<u8> = state.send.drain(..take).collect();
            (bytes, state.write_waker.take())
        };
        if let Some(waker) = waker {
            waker();
        }
        bytes
    }
}

/// A writer over a fake overlay stream with a `capacity`-byte send buffer.
fn overlay_pair(capacity: usize) -> (SharedByteBufferStream<RawStream>, FakeClient) {
    let state = Arc::new(Mutex::new(FakeState {
        send: VecDeque::with_capacity(capacity),
        capacity,
        write_waker: None,
    }));
    let socket = FakeSocket {
        state: Arc::clone(&state),
    };
    let raw = RawStream::from_connection(Connection::Overlay(Box::new(socket)))
        .expect("build the fake raw stream");
    (SharedByteBufferStream::rwrite(raw), FakeClient { state })
}

// ---------------------------------------------------------------------------
// Harness

/// Drive a writer future on the valtron pool, as a server connection task does,
/// and wait for its result on this (blocking) test thread.
fn run_writer<F>(future: F) -> io::Result<()>
where
    F: std::future::Future<Output = io::Result<()>> + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    let task = foundation_core::valtron::FutureTask::new(async move {
        let _ = tx.send(future.await);
    });
    foundation_core::valtron::send(task).expect("submit the writer task");
    rx.recv_timeout(Duration::from_secs(30))
        .expect("the writer task finished")
}

/// Read `want` bytes from the fake client in `chunk`-sized pieces, pausing
/// `pace` between pieces.
#[allow(
    clippy::needless_pass_by_value,
    reason = "the client is moved into the reader thread and dropped there"
)]
fn drain(client: FakeClient, want: usize, chunk: usize, pace: Duration) -> Vec<u8> {
    let mut got = Vec::with_capacity(want);
    while got.len() < want {
        let piece = client.read(chunk);
        if piece.is_empty() {
            std::thread::sleep(Duration::from_millis(1));
        } else {
            got.extend_from_slice(&piece);
        }
        std::thread::sleep(pace);
    }
    got
}

// ---------------------------------------------------------------------------
// Tests

/// A writer that has been idle for longer than `send_timeout` still completes
/// its next write when the client reads: the timer starts when the write does.
#[foundation_macros::valtron_test(seed = 631, threads = 3, timeout = 60_000)]
fn idle_stream_then_blocked_write_does_not_time_out() {
    let (conn, client) = overlay_pair(4096);
    // A previous response left the send buffer full; the client has not read it.
    client.fill();

    let mut writer = ResponseWriter::new(
        conn,
        WriterConfig::new().with_send_timeout(Duration::from_millis(300)),
        foundation_core::valtron::Ctx::host(),
    );

    // Idle for longer than the timeout, as an open gRPC stream does.
    std::thread::sleep(Duration::from_millis(700));

    let body = vec![0x5Au8; 32 * 1024];
    let want = 4096 + body.len();
    // The client starts reading after 50 ms: the old writer had already
    // exceeded its deadline by then and failed before any byte moved.
    let reader = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        drain(client, want, 1024, Duration::from_millis(10))
    });

    let result = run_writer(async move {
        writer.write_all(&body).await?;
        writer.flush().await
    });
    assert!(
        result.is_ok(),
        "a long-idle writer must not time out a write the client is reading: {result:?}"
    );

    let got = reader.join().expect("reader thread");
    assert_eq!(got.len(), want, "the whole body must arrive");
    assert!(
        got[4096..].iter().all(|byte| *byte == 0x5A),
        "body bytes changed"
    );
}

/// A buffered flush larger than one socket write keeps going while it drains:
/// each time the stream's buffer shrinks, the idle timer restarts.
#[foundation_macros::valtron_test(seed = 632, threads = 3, timeout = 60_000)]
fn buffered_flush_restarts_idle_timer() {
    let (conn, client) = overlay_pair(4096);
    // The send buffer is full, so the flush below blocks after its first attempt.
    client.fill();

    let mut writer = ResponseWriter::new(
        conn,
        WriterConfig::new().with_send_timeout(Duration::from_millis(400)),
        foundation_core::valtron::Ctx::host(),
    );
    // Smaller than the stream's 8 KiB write buffer, so it is buffered, not sent.
    let buffered = vec![0x33u8; 7000];
    let want = 4096 + buffered.len();

    // The client drains 512 bytes every 50 ms: the flush takes longer than the
    // 400 ms send timeout but keeps making progress.
    let reader = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        drain(client, want, 512, Duration::from_millis(50))
    });

    let result = run_writer(async move {
        writer.write_all(&buffered).await?;
        writer.flush().await
    });
    assert!(
        result.is_ok(),
        "a flush that keeps draining must not time out: {result:?}"
    );

    let got = reader.join().expect("reader thread");
    assert_eq!(
        got.len(),
        want,
        "the filler and the buffered flush must both arrive"
    );
    assert!(
        got[4096..].iter().all(|byte| *byte == 0x33),
        "the flushed bytes changed"
    );
}
