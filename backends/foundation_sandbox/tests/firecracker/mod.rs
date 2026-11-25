//! The Firecracker API client tests (feature 04).
//!
//! Every test talks to a mock HTTP server on a Unix domain socket. One test
//! needs the real `firecracker` program; it returns with a message when the
//! program is absent.

mod body;
mod errors;
mod mock_socket;
mod real;
mod source;
mod support;
