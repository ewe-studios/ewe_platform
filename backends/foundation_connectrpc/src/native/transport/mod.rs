//! Native client transports — HTTP/2 (h2c) and HTTP/3 (QUIC).
//!
//! These consume the cross-platform transport seam in
//! [`crate::shared::transport`] (the `Transport` trait, `ByteSink`/`ByteSource`,
//! stream helpers) but are native-only because they drive
//! `foundation_netio::http2` / `foundation_netio::quic` directly.

pub mod h2;

/// Test hook: the number of `H2Pump` poll turns since process start. See the
/// no-busy-poll regression test (`tests/h2_pump_parking_tests.rs`).
#[doc(hidden)]
pub use h2::h2_pump_poll_count;

/// HTTP/3 over QUIC (F35). Behind the `h3` feature: it pulls `quinn-proto`.
#[cfg(feature = "h3")]
pub mod h3;
