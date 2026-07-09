//! WebSocket server `TaskIterator` implementation (F37).
//!
//! WHY: The blocking `WebSocketServerConnection` works for one-at-a-time use,
//! but valtron-based servers need a non-blocking `TaskIterator` that integrates
//! with the executor. This mirrors the existing `WebSocketTask` (client) for the
//! server side.
//!
//! WHAT: [`WebSocketServerTask`] — a `TaskIterator` driving a server connection
//! through a configurable lifecycle. Inbound frames are de-enveloped via the
//! resumable `WebSocketFrameDecoder` (F36), assembled into messages via
//! `MessageAssembler`, and delivered through a queue. Control frames (Ping) are
//! answered per config; Close frames trigger the graceful handshake.
//!
//! HOW: State machine: Reading → Assembling → Delivering. On each poll the task
//! tries to decode frames from the stream. Data frames go to the assembler;
//! control frames are handled inline. Completed messages are pushed to the
//! delivery queue and surfaced as `Ready`.

use std::sync::Arc;
use std::time::Duration;

use concurrent_queue::ConcurrentQueue;
use foundation_core::io::buffer_pool::BytesPool;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};

use crate::netcap::RawStream;
use crate::websocket::shared::assembler::MessageAssembler;
use crate::websocket::shared::batch_writer::BatchFrameWriter;
use crate::websocket::shared::decoder::WebSocketFrameDecoder;
use crate::websocket::shared::error::WebSocketError;
use crate::websocket::shared::frame::{Opcode, WebSocketFrame};
use crate::websocket::shared::message::WebSocketMessage;

use super::server::{ServerConnectionState, WebSocketServerConnection};

/// Configuration for a [`WebSocketServerTask`].
#[derive(Clone)]
pub struct WsServerConfig {
    /// Automatically respond to Ping frames with Pong.
    pub auto_pong: bool,
    /// Maximum inbound message size in bytes (default 64 MiB).
    pub max_message_size: usize,
    /// Whether to perform a graceful close handshake on error or client Close.
    pub graceful_close: bool,
    /// Capacity of the outbound frame queue.
    pub outbound_depth: usize,
    /// Capacity of the inbound message delivery queue.
    pub inbound_depth: usize,
}

impl Default for WsServerConfig {
    fn default() -> Self {
        Self {
            auto_pong: true,
            max_message_size: 64 * 1024 * 1024,
            graceful_close: true,
            outbound_depth: 64,
            inbound_depth: 64,
        }
    }
}

/// Phases of the server task.
enum ServerTaskPhase {
    /// Decoding frames from the stream.
    Reading,
    /// A close handshake is in progress.
    Closing,
    /// Terminal.
    Done,
}

/// Progress-driven WebSocket server task.
///
/// Wraps a [`WebSocketServerConnection`] with a resumable frame decoder,
/// message assembler, config-driven control-frame handling, and a queue-based
/// delivery seam. Implements [`TaskIterator`] so the valtron executor drives it.
pub struct WebSocketServerTask {
    conn: WebSocketServerConnection,
    decoder: WebSocketFrameDecoder,
    assembler: MessageAssembler,
    config: WsServerConfig,
    phase: ServerTaskPhase,
    /// Completed messages awaiting delivery.
    inbox: Arc<ConcurrentQueue<WebSocketMessage>>,
    /// Writer for outbound frames.
    writer: BatchFrameWriter<SharedByteBufferStream<RawStream>>,
    /// Buffer for frame payload reads.
    frame_buf: bytes::BytesMut,
    /// Buffer pool for zero-copy.
    buffer_pool: Arc<BytesPool>,
}

impl WebSocketServerTask {
    /// Create a new server task from an already-upgraded stream.
    #[must_use]
    pub fn new(stream: SharedByteBufferStream<RawStream>, config: WsServerConfig) -> Self {
        let writer = BatchFrameWriter::with_defaults(stream.clone());
        Self {
            conn: WebSocketServerConnection::new(stream),
            decoder: WebSocketFrameDecoder::new(),
            assembler: MessageAssembler::new(config.max_message_size),
            config,
            phase: ServerTaskPhase::Reading,
            inbox: Arc::new(ConcurrentQueue::bounded(64)),
            writer,
            frame_buf: bytes::BytesMut::with_capacity(8192),
            buffer_pool: Arc::new(BytesPool::new(4096, 32)),
        }
    }

    /// The delivery queue. Callers drain it on their side.
    #[must_use]
    pub fn inbox(&self) -> Arc<ConcurrentQueue<WebSocketMessage>> {
        Arc::clone(&self.inbox)
    }

    /// Send a frame on the outbound side. Frames are batched by the writer
    /// and flushed on the next poll cycle.
    fn send_control(&mut self, frame: WebSocketFrame) {
        let _ = self.writer.write_immediate(frame);
    }

    /// Handle an inbound control frame per config.
    fn handle_control(&mut self, frame: WebSocketFrame) -> Result<(), WebSocketError> {
        match frame.opcode {
            Opcode::Ping if self.config.auto_pong => {
                let pong = WebSocketFrame {
                    fin: true,
                    opcode: Opcode::Pong,
                    mask: None,
                    payload: frame.payload,
                };
                self.send_control(pong);
                Ok(())
            }
            Opcode::Pong => {
                // Pong responses are informational — nothing to do.
                Ok(())
            }
            Opcode::Close => {
                if self.config.graceful_close {
                    // Echo the close frame back.
                    self.send_control(WebSocketFrame {
                        fin: true,
                        opcode: Opcode::Close,
                        mask: None,
                        payload: frame.payload.clone(),
                    });
                }
                self.phase = ServerTaskPhase::Done;
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl TaskIterator for WebSocketServerTask {
    type Ready = WebSocketMessage;
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // Flush pending outbound writes.
        let _ = self.writer.flush();

        match self.phase {
            ServerTaskPhase::Done => return None,
            ServerTaskPhase::Closing => {
                self.phase = ServerTaskPhase::Done;
                return None;
            }
            ServerTaskPhase::Reading => {}
        }

        // Try to decode a frame from the stream.
        match self.decoder.step(&mut self.conn.stream()) {
            Ok(foundation_core::io::DecodeStep::Frame(frame)) => {
                if frame.opcode.is_control() {
                    if let Err(_e) = self.handle_control(frame) {
                        return None;
                    }
                    // Control frames handled — retry next poll.
                    return Some(TaskStatus::Pending(()));
                }

                // Data frame: feed to the assembler.
                match self.assembler.feed(frame) {
                    Ok(Some(message)) => {
                        // Complete message assembled — push to inbox.
                        let _ = self.inbox.push(message.clone());
                        // Also surface through Ready so a simple driver sees it.
                        return Some(TaskStatus::Ready(message));
                    }
                    Ok(None) => {
                        // Fragment accepted, more needed.
                        return Some(TaskStatus::Pending(()));
                    }
                    Err(_e) => {
                        self.phase = ServerTaskPhase::Done;
                        return None;
                    }
                }
            }
            Ok(foundation_core::io::DecodeStep::Pending) => {
                // No frame available yet — park briefly.
                return Some(TaskStatus::Delayed(Duration::from_millis(1)));
            }
            Err(_e) => {
                self.phase = ServerTaskPhase::Done;
                return None;
            }
        }
    }
}
