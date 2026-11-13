//! Per-process token counter for reactor registrations.

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::poll::Token;

/// Per-process token counter for reactor registrations.
///
/// The completion selector reserves the top two bits of a `Token` (`POLL_TAG`),
/// so tokens must stay below `2^62`; the mask in [`next_token`] enforces that.
/// Starts at 1 — `Token(0)` is left free as a conventional sentinel.
static NEXT_TOKEN: AtomicUsize = AtomicUsize::new(1);

/// WHY: every reactor registration needs a distinct `Token`, and the completion
/// selector reserves the two high bits, so a naive counter could eventually
/// collide with the tag space.
///
/// WHAT: the next registration token, wrapped into the low 62 bits.
///
/// HOW: a relaxed fetch-add masked to `2^62 - 1`. Collision requires `2^62`
/// concurrently-live registrations, which is not reachable.
///
/// # Panics
/// Never panics.
pub fn next_token() -> Token {
    let n = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed) & ((1usize << 62) - 1);
    Token(n)
}
