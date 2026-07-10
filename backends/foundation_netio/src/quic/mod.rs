//! QUIC transport layer — trait abstraction + quinn-proto driver (F33).
//!
//! WHY: HTTP/3 needs a QUIC backend. `quinn-proto` is a sans-IO state
//! machine — no tokio, no async runtime. We drive it from a valtron task
//! that feeds UDP bytes in and polls events out.
//!
//! WHAT: [`QuicDriver`] wraps a `quinn_proto::Connection` + `UdpSocket` and
//! implements [`TaskIterator`]. Each poll does ONE I/O operation — no loops.

#[cfg(feature = "quic")]
mod quinn_driver;

#[cfg(feature = "quic")]
pub use quinn_driver::*;
