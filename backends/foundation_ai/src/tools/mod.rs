//! Development tools — subprocess harnesses for local AI backends.
//!
//! Gated behind `features = ["tools"]`. These are NOT production deps;
//! they start/stop external binaries for integration testing.

pub mod llama_server;

pub use llama_server::{LlamaServer, LlamaServerConfig};
