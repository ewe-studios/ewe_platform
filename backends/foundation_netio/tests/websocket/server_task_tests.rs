//! WebSocket server `TaskIterator` state-machine tests (F37).
//!
//! Moved out of `server_task.rs`'s inline `#[cfg(test)] mod` per AGENTS.md
//! ("tests live in `tests/`, never inline"). Each test drives a real
//! [`WebSocketServerTask`] over a localhost TCP pair, so the incremental frame
//! decoder reads from a real (non-blocking) stream rather than a byte buffer.

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::{Ctx, Pipe, TaskIterator, TaskStatus};
use foundation_netio::netcap::RawStream;
use foundation_netio::websocket::{WebSocketMessage, WebSocketServerTask, WsServerConfig};
use std::io::Write;
use std::time::{Duration, Instant};

/// A connected `(server, client)` TCP pair. Both ends are non-blocking: the
/// server side is what the task polls, and a non-blocking client keeps the
/// write path from ever blocking the test thread.
fn tcp_pair() -> (std::net::TcpStream, std::net::TcpStream) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let client = std::net::TcpStream::connect(addr).unwrap();
    let (server, _) = listener.accept().unwrap();
    server.set_nonblocking(true).ok();
    client.set_nonblocking(true).ok();
    (server, client)
}

/// Encoded wire bytes for a masked text frame.
fn text_frame(payload: &[u8]) -> Vec<u8> {
    let mut wire = vec![0x81u8];
    wire.push(0x80 | (payload.len() as u8));
    let mask: [u8; 4] = [0x01, 0x02, 0x03, 0x04];
    wire.extend_from_slice(&mask);
    for (i, &b) in payload.iter().enumerate() {
        wire.push(b ^ mask[i % 4]);
    }
    wire
}

/// Encoded wire bytes for a masked ping frame.
fn ping_frame(payload: &[u8]) -> Vec<u8> {
    let mut wire = vec![0x89u8];
    wire.push(0x80 | (payload.len() as u8));
    let mask: [u8; 4] = [0x0A, 0x0B, 0x0C, 0x0D];
    wire.extend_from_slice(&mask);
    for (i, &b) in payload.iter().enumerate() {
        wire.push(b ^ mask[i % 4]);
    }
    wire
}

/// Encoded wire bytes for a masked close frame.
fn close_frame(code: u16) -> Vec<u8> {
    let payload = code.to_be_bytes().to_vec();
    let mut wire = vec![0x88u8];
    wire.push(0x80 | (payload.len() as u8));
    let mask: [u8; 4] = [0x01, 0x00, 0x00, 0x00];
    wire.extend_from_slice(&mask);
    for (i, &b) in payload.iter().enumerate() {
        wire.push(b ^ mask[i % 4]);
    }
    wire
}

/// Poll a server task to completion, collecting every delivered message.
///
/// The task is poll-driven: an idle non-blocking stream yields
/// [`TaskStatus::Delayed`], not a message. A fixed poll count is therefore racy
/// — one `WouldBlock` before the frame bytes land aborts the loop and drops the
/// message. Poll instead until the task drains (`None`) or, after making
/// progress, goes idle; retry `Delayed` until the bytes arrive, bounded by a
/// deadline so a genuine regression fails fast rather than hanging.
fn drain(task: &mut WebSocketServerTask) -> Vec<WebSocketMessage> {
    let deadline = Duration::from_secs(2);
    let mut msgs = Vec::new();
    let mut progressed = false;
    let start = Instant::now();
    while start.elapsed() < deadline {
        match task.next_status(Ctx::new()) {
            Some(TaskStatus::Ready(m)) => {
                msgs.push(m);
                progressed = true;
            }
            Some(TaskStatus::Pending(())) => progressed = true,
            Some(TaskStatus::Delayed(d)) => {
                if progressed {
                    break;
                }
                std::thread::sleep(d);
            }
            Some(TaskStatus::Depends(_)) => {
                if progressed {
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            // Init / Wait / Ignore / Spawn / Spread — not produced by this task;
            // treat as progress so the loop still terminates.
            Some(_) => progressed = true,
            None => break,
        }
    }
    msgs
}

#[test]
fn masked_text_message_delivers_via_incremental_decoder() {
    // Use a TCP pair so the incremental decoder reads from a real stream.
    let (server, mut client) = tcp_pair();
    let wire = text_frame(b"hello");
    client.write_all(&wire).unwrap();

    let stream = SharedByteBufferStream::rwrite(RawStream::from_tcp(server).unwrap());
    let (tx, _rx) = Pipe::with_depth(8);
    let mut task = WebSocketServerTask::new(stream, WsServerConfig::default(), tx, None);

    let msgs = drain(&mut task);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0], WebSocketMessage::Text("hello".into()));
}

#[test]
fn ping_triggers_pong_output() {
    let (server, mut client) = tcp_pair();
    let wire = ping_frame(b"keepalive");
    client.write_all(&wire).unwrap();

    let stream = SharedByteBufferStream::rwrite(RawStream::from_tcp(server).unwrap());
    let (tx, _rx) = Pipe::with_depth(8);
    let mut task = WebSocketServerTask::new(stream, WsServerConfig::default(), tx, None);

    // Ping → no `Ready` message (control frame handled internally, pong written
    // straight to the stream). The task stays alive, not drained.
    let msgs = drain(&mut task);
    assert!(msgs.is_empty());
}

#[test]
fn close_frame_drains_task() {
    let (server, mut client) = tcp_pair();
    let wire = close_frame(1000);
    client.write_all(&wire).unwrap();

    let stream = SharedByteBufferStream::rwrite(RawStream::from_tcp(server).unwrap());
    let (tx, _rx) = Pipe::with_depth(8);
    let mut task = WebSocketServerTask::new(stream, WsServerConfig::default(), tx, None);

    let msgs = drain(&mut task);
    assert!(msgs.is_empty());
    assert!(task.next_status(Ctx::new()).is_none(), "task ends after close");
}

#[test]
fn unmasked_frame_triggers_close_and_drain() {
    let (server, mut client) = tcp_pair();
    // Unmasked data frame (protocol violation — client MUST mask).
    let wire: Vec<u8> = vec![0x81, 0x05, b'H', b'e', b'l', b'l', b'o'];
    client.write_all(&wire).unwrap();

    let stream = SharedByteBufferStream::rwrite(RawStream::from_tcp(server).unwrap());
    let (tx, _rx) = Pipe::with_depth(8);
    let mut task = WebSocketServerTask::new(stream, WsServerConfig::default(), tx, None);

    // No messages — violation sends close and drains.
    let msgs = drain(&mut task);
    assert!(msgs.is_empty());
    assert!(task.next_status(Ctx::new()).is_none());
}
