//! Foundation Deployment Platform — VM/container orchestration.
//!
//! **WHY:** The workspace needs a single crate that owns platform abstraction:
//! the `Provider` trait for VM/container backends (QEMU, UTM, Docker),
//! guest communication (SSH, WinRM), bootstrap, image management, and
//! the Docker interaction layer (testcontainers-style container lifecycle).
//!
//! **WHAT:** The `docker` module provides `ContainerHandle` (RAII guard),
//! `ContainerConfig` (builder), `WaitFor` strategies, `ContainerGroup`,
//! `NetworkHandle`, `DockerClient`, and `DockerError`. The `providers`
//! module holds the `Provider` trait and backend implementations.
//!
//! **HOW:** The core Docker API is `async fn` (bollard requires tokio).
//! Sync callers use `futures_lite::block_on` (re-exported here) at
//! the boundary. The `#[docker_container]` proc macro (Phase 2) will
//! live in `foundation_macros` and be re-exported from this crate.

pub mod docker;

// Re-export futures_lite::block_on for sync callers
pub use futures_lite::future::block_on;
