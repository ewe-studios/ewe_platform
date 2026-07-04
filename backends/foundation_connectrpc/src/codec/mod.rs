//! Codec system (Decision 02): the single codec authority.
//!
//! WHY: ConnectRPC negotiates message serialization by content-type. The codec
//! is chosen by the client per request (the wire token in `Content-Type`), so a
//! procedure owns a *set* of codecs keyed by that token — there is **no
//! request-time registry** and **no message type-erasure**. The message type
//! lives on the trait ([`CodecFor<M>`]); each codec blanket-implements it over
//! its family's native message trait, so `dyn CodecFor<M>` is object-safe and no
//! `dyn Any` message ever exists.
//!
//! WHAT: the [`Codec`] metadata supertrait, the typed [`CodecFor<M>`] trait,
//! the three first-class codecs ([`ProtoCodec`], [`JsonCodec`], and — under the
//! `arrow` feature — [`ArrowCodec`]), the frozen per-procedure
//! [`ProcedureCodecs`] table, the [`CodecSet`] tuple constructor, and
//! [`CodecError`].
//!
//! HOW: See the sub-modules. Domain [`CodecError`]s map into the RPC error at the
//! boundary via `From`/`change_context` (Decision 03).

mod proto;
mod json;
mod procedure;

#[cfg(feature = "arrow")]
mod arrow;

pub use json::JsonCodec;
pub use proto::ProtoCodec;
pub use procedure::{CodecSet, ProcedureCodecs};

#[cfg(feature = "arrow")]
pub use arrow::ArrowCodec;

use std::error::Error as StdError;

use bytes::Bytes;
use foundation_errstacks::ErrorTrace;

use crate::error::{Code, ConnectError};

/// Object-safe metadata supertrait. [`name`](Codec::name) is the **wire token**
/// carried in `Content-Type` (`application/{name}`, `application/connect+{name}`,
/// GET `?encoding={name}`), not an internal registry key.
pub trait Codec: Send + Sync + 'static {
    /// Wire name used in `Content-Type` headers (`"proto"` | `"json"` | `"arrow"`).
    fn name(&self) -> &str;
    /// Binary (base64 in GET query) vs text encoding.
    fn is_binary(&self) -> bool;
}

/// Typed encode/decode for one concrete message type `M`. The generic is on the
/// trait so each codec blanket-implements it over its family's message bound
/// (bounds live on the impl block), keeping `dyn CodecFor<M>` object-safe.
pub trait CodecFor<M>: Codec {
    /// Encode a concrete message to wire bytes.
    fn marshal(&self, message: &M) -> Result<Bytes, CodecError>;
    /// Decode wire bytes into a concrete owned message.
    fn unmarshal(&self, data: Bytes) -> Result<M, CodecError>;
    /// Deterministic serialization for HTTP GET caching (stable ordering).
    fn marshal_stable(&self, message: &M) -> Result<Bytes, CodecError>;
    /// Append-encode into a (pooled) buffer for hot paths (Decision 02 RS8).
    fn marshal_append(&self, buf: &mut Vec<u8>, message: &M) -> Result<(), CodecError>;
}

/// A codec failure below the RPC surface. Maps into [`ConnectError`] at the
/// boundary (Decision 03): encode failures → [`Code::Internal`], decode /
/// zero-length failures → [`Code::InvalidArgument`].
#[derive(Debug)]
pub enum CodecError {
    /// Encoding a message failed.
    Encode {
        /// The codec's wire name.
        codec: &'static str,
        /// Human-readable cause.
        message: String,
    },
    /// Decoding wire bytes failed.
    Decode {
        /// The codec's wire name.
        codec: &'static str,
        /// Human-readable cause.
        message: String,
    },
    /// A zero-length payload where the codec requires content (Decision 02 P16).
    ZeroLength {
        /// The codec's wire name.
        codec: &'static str,
    },
}

impl CodecError {
    fn encode(codec: &'static str, message: impl Into<String>) -> Self {
        CodecError::Encode {
            codec,
            message: message.into(),
        }
    }

    fn decode(codec: &'static str, message: impl Into<String>) -> Self {
        CodecError::Decode {
            codec,
            message: message.into(),
        }
    }

    /// The RPC code this failure maps to.
    fn code(&self) -> Code {
        match self {
            CodecError::Encode { .. } => Code::Internal,
            CodecError::Decode { .. } | CodecError::ZeroLength { .. } => Code::InvalidArgument,
        }
    }

    /// Build the boundary [`ConnectError`] this failure maps to.
    fn to_connect_error(&self) -> ConnectError {
        ConnectError::new(self.code(), self.to_string())
    }
}

impl core::fmt::Display for CodecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CodecError::Encode { codec, message } => {
                write!(f, "{codec} codec: encode failed: {message}")
            }
            CodecError::Decode { codec, message } => {
                write!(f, "{codec} codec: decode failed: {message}")
            }
            CodecError::ZeroLength { codec } => {
                write!(f, "{codec} codec: zero-length payload is not valid")
            }
        }
    }
}

impl StdError for CodecError {}

impl From<CodecError> for ConnectError {
    fn from(err: CodecError) -> Self {
        err.to_connect_error()
    }
}

impl From<CodecError> for ErrorTrace<ConnectError> {
    fn from(err: CodecError) -> Self {
        let connect = err.to_connect_error();
        // Preserve the CodecError as the originating frame under the RPC context.
        ErrorTrace::new(err).change_context(connect)
    }
}
