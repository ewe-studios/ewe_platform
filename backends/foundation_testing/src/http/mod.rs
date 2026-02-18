//! HTTP test server utilities.
//!
//! WHY: Provides real HTTP test server built on stdlib TCP.
//! Avoids external dependencies by using hand-crafted HTTP/1.1 responses.
//!
//! WHAT: `TestHttpServer` for integration testing HTTP clients.
//!
//! HOW: Uses stdlib's `TcpListener` with manually crafted HTTP responses.
//! Simple implementation suitable for basic HTTP client testing.

mod server;

pub use server::{HttpRequest, HttpResponse, TestHttpServer};
