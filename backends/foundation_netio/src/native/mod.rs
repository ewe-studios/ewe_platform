//! Native-only modules (TCP, TLS, io_uring — not compiled on wasm).

pub mod connection;
pub mod raw_stream;
pub mod ssl;

// Dead code: tls_verification was never wired in.
// pub mod tls_verification;
