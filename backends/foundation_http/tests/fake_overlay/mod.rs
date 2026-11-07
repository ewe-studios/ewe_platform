//! A deterministic in-memory overlay transport with a small send buffer.
//!
//! This is the transport used by `response_writer_timer_tests.rs`, extracted so
//! the SSE writer regression test drives the same readiness path. The writer
//! blocks once the send buffer is full; the client fires the writer's one-shot
//! `ConnWaker` after every read. It behaves the same on macOS and Linux: no
//! kernel socket buffer sizes and no kernel pacing rules.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::native::connection::{ConnWaker, OverlayReadWrite};
use foundation_netio::netcap::{Connection, RawStream};

/// The shared send buffer between the writer and the test client.
pub struct FakeState {
    /// Bytes the writer queued and the client has not read.
    pub send: VecDeque<u8>,
    /// Bytes the client sent and the reader has not read.
    pub recv: VecDeque<u8>,
    /// The most the writer may queue before it blocks.
    pub capacity: usize,
    /// The writer's one-shot write-readiness waker.
    pub write_waker: Option<ConnWaker>,
    /// Set when the client goes away, so the writer sees a broken connection
    /// instead of a full one.
    pub closed: bool,
}

/// The writer's end of the fake transport. It blocks once the send buffer is
/// full, and fires the registered waker when the client frees space.
#[derive(Clone)]
pub struct FakeSocket {
    state: Arc<Mutex<FakeState>>,
}

impl std::fmt::Debug for FakeSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeSocket").finish_non_exhaustive()
    }
}

impl Read for FakeSocket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut state = self.state.lock().expect("fake socket lock");
        let take = state.recv.len().min(buf.len());
        if take == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        for (slot, byte) in buf.iter_mut().zip(state.recv.drain(..take)) {
            *slot = byte;
        }
        Ok(take)
    }
}

impl Write for FakeSocket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut state = self.state.lock().expect("fake socket lock");
        if state.closed {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
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
        let state = self.state.lock().expect("fake socket lock");
        !state.recv.is_empty()
    }

    fn writable(&self) -> bool {
        let state = self.state.lock().expect("fake socket lock");
        !state.closed && state.send.len() < state.capacity
    }
}

/// The client's end of the fake transport.
pub struct FakeClient {
    state: Arc<Mutex<FakeState>>,
}

impl FakeClient {
    /// Queue bytes for the reader (the server) to read.
    pub fn queue_request(&self, bytes: &[u8]) {
        let mut state = self.state.lock().expect("fake socket lock");
        state.recv.extend(bytes);
    }

    /// Fill the send buffer, as a previous response that the client has not read.
    pub fn fill(&self) {
        self.fill_with_free(0);
    }

    /// Fill the send buffer but leave `free` bytes of room.
    pub fn fill_with_free(&self, free: usize) {
        let mut state = self.state.lock().expect("fake socket lock");
        let target = state.capacity.saturating_sub(free);
        while state.send.len() < target {
            state.send.push_back(0);
        }
    }

    /// Read up to `max` bytes, then fire the writer's readiness waker.
    pub fn read(&self, max: usize) -> Vec<u8> {
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

    /// Read every byte currently buffered, without waiting for more.
    pub fn read_available(&self) -> Vec<u8> {
        self.read(usize::MAX)
    }

    /// Close the connection: later writes fail with `BrokenPipe`.
    pub fn close(&self) {
        let mut state = self.state.lock().expect("fake socket lock");
        state.closed = true;
        if let Some(waker) = state.write_waker.take() {
            waker();
        }
    }

    /// Read `want` bytes in `chunk`-sized pieces, pausing `pace` between pieces.
    pub fn drain(&self, want: usize, chunk: usize, pace: std::time::Duration) -> Vec<u8> {
        let mut got = Vec::with_capacity(want);
        while got.len() < want {
            let piece = self.read(chunk);
            if piece.is_empty() {
                std::thread::sleep(std::time::Duration::from_millis(1));
            } else {
                got.extend_from_slice(&piece);
            }
            std::thread::sleep(pace);
        }
        got
    }
}

/// A writer over a fake overlay stream with a `capacity`-byte send buffer.
pub fn overlay_pair(capacity: usize) -> (SharedByteBufferStream<RawStream>, FakeClient) {
    let state = Arc::new(Mutex::new(FakeState {
        send: VecDeque::with_capacity(capacity),
        recv: VecDeque::new(),
        capacity,
        write_waker: None,
        closed: false,
    }));
    let socket = FakeSocket {
        state: Arc::clone(&state),
    };
    let raw = RawStream::from_connection(Connection::Overlay(Box::new(socket)))
        .expect("build the fake raw stream");
    (SharedByteBufferStream::rwrite(raw), FakeClient { state })
}
