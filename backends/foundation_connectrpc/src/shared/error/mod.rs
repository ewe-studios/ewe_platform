//! The protocol-agnostic ConnectRPC error model (Decision 03).
//!
//! WHY: One error vocabulary serves all three wire protocols. The canonical RPC
//! error is [`ConnectResult`] = `Result<T, ErrorTrace<ConnectError>>`: an
//! errstacks trace carrying [`ConnectError`] as its context. Domain layers keep
//! their own typed errors and map in at the boundary via `From`/`change_context`
//! (`CodecError`, `CompressionError`, `EnvelopeError`, `TransportError` — defined
//! in their own features). Bare `Result<_, ConnectError>` appears nowhere public.
//!
//! WHAT: [`Code`], [`ConnectError`], [`ErrorDetail`], the JSON wire shapes
//! ([`WireError`], [`WireErrorDetail`], [`EndStreamResponse`]), the
//! [`ConnectResult`] alias, and the [`code_of`] / `wrap_if_*` classification
//! helpers.
//!
//! HOW: See the sub-modules. The `wrap_if_*` helpers classify arbitrary
//! lower-level errors into a [`Code`] by inspecting the error chain; transports
//! compose them at the boundary to produce a [`ConnectError`].

mod code;
mod connect_error;
mod detail;
mod wire;

pub use code::Code;
pub use connect_error::ConnectError;
pub use detail::{ErrorDetail, ERRSTACKS_TYPE_URL};
pub use wire::{EndStreamResponse, WireError, WireErrorDetail};

use std::error::Error as StdError;

use foundation_errstacks::ErrorTrace;

/// The error type of every public RPC-surface signature (Decision 03).
pub type ConnectResult<T> = Result<T, ErrorTrace<ConnectError>>;

/// Extract the [`Code`] from an arbitrary error by walking its `source()` chain
/// for a [`ConnectError`] (Decision 03 H12/H13). Returns [`Code::Unknown`] if no
/// coded error is found.
#[must_use]
pub fn code_of(err: &(dyn StdError + 'static)) -> Code {
    let mut current: Option<&(dyn StdError + 'static)> = Some(err);
    while let Some(e) = current {
        if let Some(ce) = e.downcast_ref::<ConnectError>() {
            return ce.code();
        }
        current = e.source();
    }
    Code::Unknown
}

/// Lowercased concatenation of an error and its `source()` chain's `Display`,
/// used by the `wrap_if_*` signature classifiers.
fn chain_text(err: &(dyn StdError + 'static)) -> String {
    let mut text = String::new();
    let mut current: Option<&(dyn StdError + 'static)> = Some(err);
    while let Some(e) = current {
        if !text.is_empty() {
            text.push_str(": ");
        }
        text.push_str(&e.to_string());
        current = e.source();
    }
    text.to_lowercase()
}

/// Classify a cancellation/deadline error (Decision 03 H11). Returns
/// [`Code::DeadlineExceeded`] for deadline/timeout signatures,
/// [`Code::Canceled`] for cancellation, else `None`.
#[must_use]
pub fn wrap_if_context(err: &(dyn StdError + 'static)) -> Option<Code> {
    let text = chain_text(err);
    if text.contains("deadline") || text.contains("timed out") || text.contains("timeout") {
        Some(Code::DeadlineExceeded)
    } else if text.contains("cancel") {
        Some(Code::Canceled)
    } else {
        None
    }
}

/// Classify an HTTP/2 stream-reset (`RST_STREAM`) error (Decision 03 H11).
/// Stream resets surface as [`Code::Canceled`] (the peer abandoned the stream),
/// else `None`.
#[must_use]
pub fn wrap_if_rst(err: &(dyn StdError + 'static)) -> Option<Code> {
    let text = chain_text(err);
    if text.contains("rst_stream") || text.contains("stream reset") || text.contains("stream closed")
    {
        Some(Code::Canceled)
    } else {
        None
    }
}

/// Classify a likely h2c-misconfiguration error (Decision 03 H11) — an HTTP/2
/// cleartext connection attempted against a server that only speaks HTTP/1.1.
/// Returns [`Code::Unavailable`], else `None`.
#[must_use]
pub fn wrap_if_h2c(err: &(dyn StdError + 'static)) -> Option<Code> {
    let text = chain_text(err);
    let looks_h2 = text.contains("http2") || text.contains("h2c");
    if looks_h2
        && (text.contains("unexpected")
            || text.contains("malformed")
            || text.contains("goaway")
            || text.contains("not configured"))
    {
        Some(Code::Unavailable)
    } else {
        None
    }
}

/// Wrap an arbitrary error as a [`ConnectError`], assigning the code found in its
/// chain (Decision 03 H11) — [`Code::Unknown`] if it carries none. The original
/// error is retained as the source.
#[must_use]
pub fn wrap_if_uncoded<E>(err: E) -> ConnectError
where
    E: StdError + Send + Sync + 'static,
{
    let code = code_of(&err);
    ConnectError::with_source(code, err)
}
