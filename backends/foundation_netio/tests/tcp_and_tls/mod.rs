// tcp_and_tls tests — currently disabled (require real TCP/TLS infrastructure)
// The chunked tests live in simple_http/ and are imported via the simple_http
// entry in the parent mod.rs, so we do not re-declare them here.

// Feature 04: non-blocking TCP connect and TLS handshake regression tests.
// These use a mock DNS resolver and loopback listeners, not real network I/O.
#[cfg(unix)]
mod nonblocking_connect_tls;

// Feature 06: rustls readiness follows the TLS state, not the direction the
// application asked for.
#[cfg(all(unix, feature = "ssl-rustls"))]
mod rustls_opposite_direction;

// Moved inline tests (CLAUDE.md rule 7).
#[cfg(unix)]
mod tcp_stream_wrapper;
#[cfg(all(unix, feature = "ssl-rustls"))]
mod rustls_connector;
