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
    read_frame, CapsStatus, ClientHello, Frame, FrameKindName, ServerActionT, ServerCapsT,
    WireError, SCHEMA_VERSION,
};

use super::{Dispatcher, SubscriptionReplay};
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
    subscription_resume: bool,
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
            subscription_resume: false,
            sequencer: Mutex::new(StreamSequencer::new(stream_id)),
            last_request: None,
        }
    }

    #[must_use]
    pub fn caps(&self) -> &ClientCaps {
        &self.caps
    }

    /// Returns the resume capability that the peers negotiated for this connection.
    #[must_use]
    pub fn subscription_resume(&self) -> bool {
        self.subscription_resume
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

fn read_hello(frame: Frame<'_>) -> Result<ClientHello<'_>, HelloError> {
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
    request
        .hello()
        .filter(|_| request.action().is_none() && request.resume().is_none())
        .ok_or(HelloError::NotAHello)
}

fn accepts_hello(frame: Frame<'_>, hello: ClientHello<'_>) -> bool {
    frame.schema_version() == SCHEMA_VERSION && vocab_version(hello.vocabulary_version()).is_some()
}

impl Connection {
    /// Restores capabilities for an independent request on the supplied stream.
    /// An unsupported hello needs an update response from the dispatcher.
    ///
    /// # Errors
    /// Returns an error when the frame has an invalid hello.
    pub fn from_verified_hello(
        frame: Frame<'_>,
        stream_id: u64,
    ) -> Result<Option<Self>, HelloError> {
        let hello = read_hello(frame)?;
        if !accepts_hello(frame, hello) {
            return Ok(None);
        }
        if hello.resume_from() == u64::MAX {
            return Err(HelloError::InvalidResume);
        }
        let caps = ClientCaps {
            platform: hello.platform(),
            mount: hello.mount(),
            vocabulary_version: hello.vocabulary_version(),
        };
        Ok(Some(Self::stateless(caps, hello.mount(), stream_id)))
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
        self.hello_verified(ctx, frame)
    }

    /// Negotiates a hello that the caller verified with this dispatcher's wire limits.
    ///
    /// # Errors
    /// Returns an error for an invalid hello, missing randomness, or failed encoding.
    pub fn hello_verified(&self, ctx: &Ctx, frame: Frame<'_>) -> Result<HelloOutcome, HelloError> {
        let hello = read_hello(frame)?;
        let accepted = accepts_hello(frame, hello);
        let mut caps = ServerCapsT::default();
        caps.status = if accepted {
            CapsStatus::Ok
        } else {
            CapsStatus::UpdateRequired
        };
        caps.request_tag = hello.request_tag();
        caps.subscription_resume = accepted
            && hello.vocabulary_version() >= foundation_ui_wire::vocab::MANAGED_CONNECTION_VERSION
            && hello.subscription_resume()
            && ctx.get::<SubscriptionReplay>().is_some();
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
                subscription_resume: caps.subscription_resume,
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
