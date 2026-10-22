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

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::{Ctx, PipeSender, TaskIterator, TaskStatus};

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
    delivery: PipeSender<WebSocketMessage>,
    /// Caller-injected fd readiness signal (F38). When `Some`, the task parks
    /// via `Depends(fd_readiness)` on idle; when `None`, falls back to `Delayed`.
    fd_readiness: Option<Arc<dyn foundation_core::valtron::EventReadiness + Send + Sync>>,
    /// The task is draining (close frame sent, waiting for final flush).
    draining: bool,
}

impl WebSocketServerTask {
    /// Create a server task from an already-upgraded stream.
    #[must_use]
    pub fn new(
        stream: SharedByteBufferStream<RawStream>,
        config: WsServerConfig,
        delivery: PipeSender<WebSocketMessage>,
        fd_readiness: Option<Arc<dyn foundation_core::valtron::EventReadiness + Send + Sync>>,
    ) -> Self {
        let writer = BatchFrameWriter::with_defaults(stream.clone());
        Self {
            stream,
            decoder: WebSocketFrameDecoder::new(),
            assembler: MessageAssembler::new(config.max_message_size),
            writer,
            config,
            delivery,
            fd_readiness,
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

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
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
                        let _ = self.delivery.try_send(message.clone());
                        Some(TaskStatus::Ready(message))
                    }
                    Ok(None) => Some(TaskStatus::Pending(())),
                    Err(_e) => {
                        self.draining = true;
                        None
                    }
                }
            }
            Ok(DecodeStep::Pending) => {
                // Park on the caller-injected readiness signal if one was
                // provided; otherwise fall back to a short poll delay.
                match &self.fd_readiness {
                    Some(fd) => Some(TaskStatus::Depends(Arc::clone(fd)
                        as Arc<dyn foundation_core::valtron::EventReadiness + Send + Sync>)),
                    None => Some(TaskStatus::Delayed(Duration::from_millis(
                        IDLE_POLL_DELAY_MS,
                    ))),
                }
            }
            Err(_e) => {
                self.draining = true;
                None
            }
        }
    }
}
