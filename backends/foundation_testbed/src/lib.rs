//! foundation_testbed — QEMU/KVM VM orchestration for cross-platform build & test.
//!
//! Provides a sync API to launch, manage, and interact with QEMU virtual
//! machines for building and testing binaries on Windows and Linux guests
//! from a Linux host.
//!
//! # Quick Start
//!
//! ```no_run
//! use foundation_testbed::config::{get_profile, DisplayMode};
//! use foundation_testbed::qemu::QemuConfig;
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
//! - **qemu** — Process management, disk operations, networking, snapshots, downloads
//! - **ssh** — SSH connections and command execution on guests
//! - **winrm** — WinRM SOAP client for Windows guest bootstrapping
//! - **import** — Image download, cache management, Vagrant Cloud integration
//! - **bootstrap** — VM bootstrapping with mise + nushell
//! - **build** — Build pipeline, code sync, artifact retrieval

pub mod bootstrap;
pub mod build;
pub mod config;
pub mod import;
pub mod qemu;
pub mod ssh;
pub mod winrm;
