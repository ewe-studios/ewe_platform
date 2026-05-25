//! SSE message parser following W3C specification.
//!
//! WHY: SSE protocol requires parsing incoming data according to specific rules.
//! WHAT: Streaming SSE parser that reads lines and yields complete events.
//!
//! NOTE: This module now re-exports from `wire::simple_http::sse` for backward compatibility.
//! The actual implementation has been moved to the HTTP layer where SSE belongs.
//!
//! Reference: W3C Server-Sent Events specification (<https://html.spec.whatwg.org/multipage/server-sent-events.html>)

// Re-export from simple_http::sse for backward compatibility
pub use crate::event_source::shared::sse::SseParser;
