//! HTTP/3 (RFC 9114) unit suites — framing and QPACK (F34).
#![cfg(feature = "quic")]

mod frame_tests;
mod qpack_tests;
mod types_tests;
mod varint_tests;
