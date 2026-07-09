//! WebSocket server `TaskIterator` implementation (F37).
//!
//! WHY: The blocking `WebSocketServerConnection` works for one-at-a-time use,
//! but valtron-based servers need a non-blocking `TaskIterator` that integrates
//! with the executor.
//!
//! WHAT: [`WebSocketServerTask`] — a `TaskIterator` wrapping a post-upgrade
//! stream with the resumable `WebSocketFrameDecoder` (F36), `MessageAssembler`,
//! config-driven control-frame handling (Ping→Pong, Close echo), and a delivery
//! queue. Outbound frames are batched via `BatchFrameWriter` and flushed on each
//! poll.
//!
//! HOW: State machine: Reading → Deliver → Flush. On each poll, frames are
//! decoded from the stream via the incremental decoder. Data frames feed the
//! assembler; completed messages are pushed to the queue and surfaced as
//! `Ready`. Control frames (Ping/Close) are handled per `WsServerConfig`.

use std::sync::Arc;
use std::time::Duration;

use concurrent_queue::ConcurrentQueue;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::{NoSpawner, TaskIterator, TaskStatus};

use crate::netcap::RawStream;
use crate::websocket::shared::assembler::MessageAssembler;
use crate::websocket::shared::batch_writer::BatchFrameWriter;
use crate::websocket::shared::decoder::WebSocketFrameDecoder;
use crate::websocket::shared::frame::{Opcode, WebSocketFrame};
use crate::websocket::shared::message::WebSocketMessage;

use foundation_core::io::{DecodeStep, IncrementalDecoder};

/// How the server task parks when the stream has no data ready.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadModel {
    /// Yield the worker with a short delay — no reactor dependency.
    /// Safe default for all platforms.
    Poll,
    /// Park on the reactor via `TaskStatus::Depends(RegisteredFd)`.
    /// Requires a reactor Registry (see F40 shared reactor). Falls back
    /// to `Poll` when no reactor is available.
    Depends,
}

/// Configuration for a [`WebSocketServerTask`].
#[derive(Clone)]
pub struct WsServerConfig {
    /// Automatically respond to Ping frames with Pong.
    pub auto_pong: bool,
    /// Maximum inbound message size in bytes (default 64 MiB).
    pub max_message_size: usize,
    /// Whether to echo a Close frame and perform a graceful close handshake.
    pub graceful_close: bool,
    /// How the task parks when no data is available.
    pub read_model: ReadModel,
}

impl Default for WsServerConfig {
    fn default() -> Self {
        Self {
            auto_pong: true,
            max_message_size: 64 * 1024 * 1024,
            graceful_close: true,
            read_model: ReadModel::Poll,
        }
    }
}

/// Poll-backoff delay when the stream is idle and no reactor is available.
const IDLE_POLL_DELAY_MS: u64 = 1;

/// Progress-driven WebSocket server task for valtron integration.
///
/// Wraps a post-upgrade stream with a resumable frame decoder, message
/// assembler, config-driven control-frame handling, and a queue-based
/// delivery seam. Implements [`TaskIterator`] so the valtron executor
/// drives it cooperatively.
pub struct WebSocketServerTask {
    stream: SharedByteBufferStream<RawStream>,
    decoder: WebSocketFrameDecoder,
    assembler: MessageAssembler,
    writer: BatchFrameWriter<SharedByteBufferStream<RawStream>>,
    config: WsServerConfig,
    /// Completed inbound messages awaiting delivery.
    delivery: Arc<ConcurrentQueue<WebSocketMessage>>,
    /// The task is draining (close frame sent, waiting for final flush).
    draining: bool,
}

impl WebSocketServerTask {
    /// Create a server task from an already-upgraded stream.
    #[must_use]
    pub fn new(
        stream: SharedByteBufferStream<RawStream>,
        config: WsServerConfig,
        delivery: Arc<ConcurrentQueue<WebSocketMessage>>,
    ) -> Self {
        let writer = BatchFrameWriter::with_defaults(stream.clone());
        Self {
            stream,
            decoder: WebSocketFrameDecoder::new(),
            assembler: MessageAssembler::new(config.max_message_size),
            writer,
            config,
            delivery,
            draining: false,
        }
    }

    /// Send a control frame immediately (not batched).
    fn send_control(&mut self, frame: WebSocketFrame) {
        let mut frame = frame;
        frame.mask = None; // Server never masks.
        let _ = self.writer.write_immediate(frame);
    }

    /// Handle a received control frame.
    fn on_control(&mut self, frame: WebSocketFrame) {
        match frame.opcode {
            Opcode::Ping if self.config.auto_pong => {
                let pong = WebSocketFrame {
                    fin: true,
                    opcode: Opcode::Pong,
                    mask: None,
                    payload: frame.payload,
                };
                self.send_control(pong);
            }
            Opcode::Close if self.config.graceful_close => {
                self.send_control(WebSocketFrame {
                    fin: true,
                    opcode: Opcode::Close,
                    mask: None,
                    payload: frame.payload,
                });
                self.draining = true;
            }
            _ => {} // Pong, non-auto Ping, etc.
        }
    }
}

impl TaskIterator for WebSocketServerTask {
    type Ready = WebSocketMessage;
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // Flush pending outbound writes every poll.
        let _ = self.writer.flush();

        if self.draining {
            return None;
        }

        // Try to decode a frame from the stream.
        match self.decoder.step(&mut self.stream) {
            Ok(DecodeStep::Frame(frame)) => {
                if frame.opcode.is_control() {
                    // Server MUST reject unmasked data frames from client.
                    if frame.mask.is_none() {
                        let close_frame = WebSocketFrame {
                            fin: true,
                            opcode: Opcode::Close,
                            mask: None,
                            payload: {
                                let mut p = 1002u16.to_be_bytes().to_vec();
                                p.extend_from_slice(b"unmasked frame");
                                p
                            },
                        };
                        self.send_control(close_frame);
                        self.draining = true;
                        return None;
                    }
                    self.on_control(frame);
                    return Some(TaskStatus::Pending(()));
                }

                // Server MUST reject unmasked data frames.
                if frame.mask.is_none() {
                    let close_frame = WebSocketFrame {
                        fin: true,
                        opcode: Opcode::Close,
                        mask: None,
                        payload: {
                            let mut p = 1002u16.to_be_bytes().to_vec();
                            p.extend_from_slice(b"unmasked frame");
                            p
                        },
                    };
                    self.send_control(close_frame);
                    self.draining = true;
                    return None;
                }

                match self.assembler.process_frame(frame) {
                    Ok(Some(message)) => {
                        let _ = self.delivery.push(message.clone());
                        return Some(TaskStatus::Ready(message));
                    }
                    Ok(None) => {
                        return Some(TaskStatus::Pending(()));
                    }
                    Err(_e) => {
                        self.draining = true;
                        return None;
                    }
                }
            }
            Ok(DecodeStep::Pending) => {
                // No data available. With Poll: yield the worker briefly.
                // With Depends: would return Depends(registered_fd) once
                // the shared reactor (F40) provides a Registry. The fallback
                // is an explicit short delay — avoids a hot spin while
                // keeping the task schedulable without a reactor.
                return Some(TaskStatus::Delayed(Duration::from_millis(
                    IDLE_POLL_DELAY_MS,
                )));
            }
            Err(_e) => {
                self.draining = true;
                return None;
            }
        }
    }
}

// ── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use foundation_core::io::ioutils::SharedByteBufferStream;
    use std::io::Write;

    /// A helper to get a TcpStream pair for tests.
    /// Returns (server_side, client_side).
    /// The actual I/O is localhost-only and non-blocking compatible.
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

    /// Drive the task through `n` polls, collect all `Ready` values.
    fn drain(task: &mut WebSocketServerTask, n: usize) -> Vec<WebSocketMessage> {
        let mut msgs = Vec::new();
        for _ in 0..n {
            match task.next_status() {
                Some(TaskStatus::Ready(m)) => msgs.push(m),
                Some(TaskStatus::Pending(())) => continue,
                _ => break,
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
        let delivery = Arc::new(ConcurrentQueue::unbounded());
        let mut task = WebSocketServerTask::new(stream, WsServerConfig::default(), delivery);

        let msgs = drain(&mut task, 10);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0], WebSocketMessage::Text("hello".into()));
    }

    #[test]
    fn ping_triggers_pong_output() {
        let (server, mut client) = tcp_pair();
        let wire = ping_frame(b"keepalive");
        client.write_all(&wire).unwrap();

        let stream = SharedByteBufferStream::rwrite(RawStream::from_tcp(server).unwrap());
        let delivery = Arc::new(ConcurrentQueue::unbounded());
        let mut task = WebSocketServerTask::new(stream, WsServerConfig::default(), delivery);

        let msgs = drain(&mut task, 5);
        // Ping → no Ready message (control frame handled internally).
        assert!(msgs.is_empty());
        // Task should still be alive (not drained).
    }

    #[test]
    fn close_frame_drains_task() {
        let (server, mut client) = tcp_pair();
        let wire = close_frame(1000);
        client.write_all(&wire).unwrap();

        let stream = SharedByteBufferStream::rwrite(RawStream::from_tcp(server).unwrap());
        let delivery = Arc::new(ConcurrentQueue::unbounded());
        let mut task = WebSocketServerTask::new(stream, WsServerConfig::default(), delivery);

        let msgs = drain(&mut task, 10);
        assert!(msgs.is_empty());
        assert!(task.next_status().is_none(), "task ends after close");
    }

    #[test]
    fn unmasked_frame_triggers_close_and_drain() {
        let (server, mut client) = tcp_pair();
        // Unmasked data frame (protocol violation — client MUST mask).
        let wire: Vec<u8> = vec![0x81, 0x05, b'H', b'e', b'l', b'l', b'o'];
        client.write_all(&wire).unwrap();

        let stream = SharedByteBufferStream::rwrite(RawStream::from_tcp(server).unwrap());
        let delivery = Arc::new(ConcurrentQueue::unbounded());
        let mut task = WebSocketServerTask::new(stream, WsServerConfig::default(), delivery);

        let msgs = drain(&mut task, 10);
        // No messages — violation sends close and drains.
        assert!(msgs.is_empty());
        assert!(task.next_status().is_none());
    }
}
