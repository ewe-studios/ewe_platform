//! Foundation Deployment Platform — VM/container orchestration.

pub mod docker;

// Re-export futures_lite::block_on for sync callers
pub use futures_lite::future::block_on;

// Re-export the proc macro from foundation_macros
pub use foundation_macros::docker_container;
