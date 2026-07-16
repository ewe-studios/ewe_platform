//! Foundation Deployment Platform — VM/container orchestration.
//!
//! # Quick Start (VM)
//!
//! The VM surface (`config`, `qemu`, …) is gated behind the `vms` feature, so
//! this example is `ignore`d in the default (Docker-only) build — build with
//! `--features vms` to use it.
//!
//! ```ignore
//! use foundation_deployment_platform::config::{get_profile, DisplayMode};
//! use foundation_deployment_platform::qemu::QemuConfig;
//!
//! let profile = get_profile("windows-build").unwrap();
//! let vm = QemuConfig::new(profile.clone(), DisplayMode::Headless)
//!     .launch()
//!     .unwrap();
//! ```
//!
//! # Architecture
//!
//! - **config** — VM profiles, guest OS types, display modes, error types
//! - **docker** — Container lifecycle (ContainerHandle, ContainerConfig, WaitFor)
//! - **qemu** — Process management, disk operations, networking, snapshots, downloads
//! - **providers** — Provider trait + Docker/QEMU/UTM backends
//! - **ssh** — SSH connections and command execution on guests
//! - **winrm** — WinRM SOAP client for Windows guest bootstrapping
//! - **import** — Image download, cache management, Vagrant Cloud integration
//! - **bootstrap** — VM bootstrapping with mise + nushell
//! - **build** — Build pipeline, code sync, artifact retrieval
//! - **runner** — Binary launcher, screenshots, logs, file transfer, UI automation
//! - **state** — Persistent VM state management
//! - **doctor** — Host and VM health checks
//! - **init** — Project scaffolding (testbed init, scripts, .gitignore)
//! - **host_bootstrap** — Host prerequisites (mise, nushell, pitchfork)

#![allow(clippy::too_many_arguments)]

// ── Core types (always available — used by Docker provider) ──────────────

pub mod providers;

// ── VM types (available with vms) ────────────────────────────────────────

#[cfg(feature = "vms")]
pub mod config;
#[cfg(feature = "vms")]
pub mod doctor;

// ── Docker runtime (always enabled) ───────────────────────────────────────

pub mod docker;

// ── VM infrastructure (moved from foundation_testbed, gated on vms) ───────

#[cfg(feature = "vms")]
pub mod artifacts;
#[cfg(feature = "vms")]
pub mod bootstrap;
#[cfg(feature = "vms")]
pub mod build;
#[cfg(feature = "vms")]
pub mod export;
#[cfg(feature = "vms")]
pub mod host_bootstrap;
#[cfg(feature = "vms")]
pub mod import;
#[cfg(feature = "vms")]
pub mod init;
#[cfg(feature = "vms")]
pub mod qemu;
#[cfg(feature = "vms")]
pub mod runner;
#[cfg(feature = "vms")]
pub mod ssh;
#[cfg(feature = "vms")]
pub mod state;
#[cfg(feature = "vms")]
pub mod winrm;

#[cfg(all(feature = "vms", feature = "cli"))]
pub mod cli;

// Re-export futures_lite::block_on for sync callers
pub use futures_lite::future::block_on;

// Re-export the proc macro from foundation_macros
pub use foundation_macros::docker_container;
