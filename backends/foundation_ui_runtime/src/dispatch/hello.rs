//! Negotiate connection capabilities and announce callable server actions.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use foundation_compact::rng::Rng;
use foundation_context::Ctx;
use foundation_ui_wire::vocab::{vocab_version, VOCABULARY_VERSION};
use foundation_ui_wire::writer::{
    write_caps, ClientCaps, FrameHeader, StreamSequencer, WriteError,
};
use foundation_ui_wire::{
    read_frame, CapsStatus, FrameKindName, ServerActionT, ServerCapsT, WireError, SCHEMA_VERSION,
};

use super::Dispatcher;
use crate::action::Surface;

/// A failure to read a hello, allocate its stream, or encode its reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelloError {
    Wire(WireError),
    NotAHello,
    MissingRng,
    InvalidResume,
    Write(WriteError),
}

impl fmt::Display for HelloError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => fmt::Display::fmt(error, formatter),
            Self::Write(error) => fmt::Display::fmt(error, formatter),
            Self::NotAHello => formatter.write_str("Expected a request with a hello field only."),
            Self::MissingRng => {
                formatter.write_str("The host random number generator is unavailable.")
            }
            Self::InvalidResume => {
                formatter.write_str("The stream has no remaining sequence numbers.")
            }
        }
    }
}

impl std::error::Error for HelloError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Wire(error) => Some(error),
            Self::Write(error) => Some(error),
            _ => None,
        }
    }
}

impl From<WireError> for HelloError {
    fn from(error: WireError) -> Self {
        Self::Wire(error)
    }
}

/// The capabilities reply and, on acceptance, its connection.
#[derive(Debug)]
pub enum HelloOutcome {
    Accepted {
        connection: Connection,
        caps_frame: Vec<u8>,
    },
    UpdateRequired {
        caps_frame: Vec<u8>,
    },
}

/// Negotiated capabilities and sequence counters for one stream.
#[derive(Debug)]
pub struct Connection {
    caps: ClientCaps,
    stream_id: u64,
    sequencer: Mutex<StreamSequencer>,
    last_request: Option<AtomicU64>,
}

impl Connection {
    /// Creates a connection for an independent request. The surface selects the writer gate.
    #[must_use]
    pub fn stateless(mut caps: ClientCaps, surface: Surface, stream_id: u64) -> Self {
        caps.mount = surface;
        Self {
            caps,
            stream_id,
            sequencer: Mutex::new(StreamSequencer::new(stream_id)),
            last_request: None,
        }
    }

    #[must_use]
    pub fn caps(&self) -> &ClientCaps {
        &self.caps
    }

    #[must_use]
    pub fn surface(&self) -> Surface {
        self.caps.mount
    }

    #[must_use]
    pub fn stream_id(&self) -> u64 {
        self.stream_id
    }

    pub(super) fn claim_request(&self, sequence: u64) -> bool {
        self.last_request
            .as_ref()
            .is_none_or(|last| last.fetch_max(sequence, Ordering::Relaxed) < sequence)
    }

    pub(super) fn next_header(&self, ack: u64) -> FrameHeader {
        self.sequencer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .header(ack)
    }
}

impl Dispatcher {
    /// Lists the server actions that can run for this surface.
    #[must_use]
    pub fn server_actions(&self, surface: Surface) -> Vec<ServerActionT> {
        self.registry.server_actions(surface)
    }

    /// Reads a hello and builds its capabilities reply with sequence zero.
    ///
    /// # Errors
    /// Returns a typed error for invalid input, unavailable randomness, or a failed write.
    pub fn hello(&self, ctx: &Ctx, bytes: &[u8]) -> Result<HelloOutcome, HelloError> {
        let frame = read_frame(bytes, self.config.wire_limits())?;
        let request = frame.kind_as_frame_request().ok_or_else(|| {
            if frame.kind_as_frame_message().is_some() {
                WireError::WrongKind {
                    expected: FrameKindName::Request,
                    found: FrameKindName::Message,
                }
            } else {
                WireError::NoKind
            }
        })?;
        let hello = request
            .hello()
            .filter(|_| request.action().is_none())
            .ok_or(HelloError::NotAHello)?;
        let accepted = frame.schema_version() == SCHEMA_VERSION
            && vocab_version(hello.vocabulary_version()).is_some();
        let mut caps = ServerCapsT::default();
        caps.status = if accepted {
            CapsStatus::Ok
        } else {
            CapsStatus::UpdateRequired
        };
        caps.schema_version = SCHEMA_VERSION;
        caps.vocabulary_version = VOCABULARY_VERSION;
        caps.actions = Some(self.server_actions(hello.mount()));
        let connection = if accepted {
            caps.vocabulary_version = hello.vocabulary_version().min(VOCABULARY_VERSION);
            if frame.stream_id() == 0 {
                let rng = ctx.try_rng().ok_or(HelloError::MissingRng)?;
                caps.stream_id = rng
                    .lock()
                    .map_err(|_| HelloError::MissingRng)?
                    .next_u64()
                    .max(1);
            } else {
                if hello.resume_from() == u64::MAX {
                    return Err(HelloError::InvalidResume);
                }
                caps.stream_id = frame.stream_id();
                caps.resume_from = hello.resume_from();
            }
            Some(Connection {
                caps: ClientCaps {
                    platform: hello.platform(),
                    mount: hello.mount(),
                    vocabulary_version: caps.vocabulary_version,
                },
                stream_id: caps.stream_id,
                sequencer: Mutex::new(StreamSequencer::resume(caps.stream_id, caps.resume_from)),
                last_request: Some(AtomicU64::new(0)),
            })
        } else {
            None
        };
        let header = FrameHeader {
            stream_id: caps.stream_id,
            sequence_id: 0,
            ack: 0,
        };
        let caps_frame = write_caps(&caps, header, &self.config).map_err(HelloError::Write)?;
        Ok(match connection {
            Some(connection) => HelloOutcome::Accepted {
                connection,
                caps_frame,
            },
            None => HelloOutcome::UpdateRequired { caps_frame },
        })
    }
}
