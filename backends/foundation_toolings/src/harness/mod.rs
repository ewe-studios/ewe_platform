//! Subprocess harnesses for dev/test infrastructure.
//!
//! Each harness starts an external process, polls for readiness, and kills
//! it on drop.

pub mod llama_server;
pub mod miniflare;
pub mod workerd;

fn pick_unused_port() -> Option<u16> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .ok()
        .and_then(|l| l.local_addr().ok())
        .map(|a| a.port())
}

pub use llama_server::{LlamaServer, LlamaServerConfig, LlamaServerError};
pub use miniflare::{Miniflare, MiniflareConfig, MiniflareError, MinflareTool};
pub use workerd::{Workerd, WorkerdConfig, WorkerdError};
