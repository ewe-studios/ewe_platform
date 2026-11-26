//! The libkrun runner configuration (feature 09).
//!
//! The driver writes this value as `<run_dir>/krun.json`. The runner reads the
//! same type, because both sides are one crate. The types are plain data, so a
//! wasm target compiles them too.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The log level of libkrun that turns logging off.
pub const KRUN_LOG_LEVEL_OFF: u32 = 0;
/// The log level of libkrun that reports errors only.
pub const KRUN_LOG_LEVEL_ERROR: u32 = 1;
/// The log level of libkrun that reports errors and warnings. This is the
/// driver default: enough to explain a failed start, with a small log.
pub const KRUN_LOG_LEVEL_WARN: u32 = 2;
/// The log level of libkrun that reports information messages.
pub const KRUN_LOG_LEVEL_INFO: u32 = 3;
/// The log level of libkrun that reports debug messages.
pub const KRUN_LOG_LEVEL_DEBUG: u32 = 4;
/// The log level of libkrun that reports every message.
pub const KRUN_LOG_LEVEL_TRACE: u32 = 5;

/// One host directory that libkrun shares with the guest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KrunMount {
    /// The virtio-fs tag that the guest mounts.
    pub tag: String,
    /// The path on the host.
    pub host_path: PathBuf,
    /// Whether the guest may write to the directory.
    pub read_only: bool,
}

/// The full description of one libkrun virtual machine.
///
/// The driver writes it once. The runner only reads it, and it never changes
/// after the write (feature 09, section 3.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KrunRunnerConfig {
    /// The library path, or `None` for the loader search.
    pub libkrun_path: Option<PathBuf>,
    /// The number of virtual CPUs.
    pub vcpus: u8,
    /// The memory in mebibytes.
    pub memory_mib: u32,
    /// The private boot root: `<run_dir>/root`.
    pub boot_root: PathBuf,
    /// The caller's root directory, as the tag `ewe-root`. It is always
    /// read-only.
    pub user_root: Option<KrunMount>,
    /// The host path of the slave program.
    pub slave_path: PathBuf,
    /// The host Unix domain socket that libkrun makes for the slave port.
    pub slave_socket: PathBuf,
    /// The vsock port of the slave. The default is 1024.
    pub slave_port: u32,
    /// Whether the guest uses the host network through Transparent Socket
    /// Impersonation (TSI). It is on only with `NetworkMode::Host`.
    pub tsi_inet: bool,
    /// The extra host directories, as `{ tag, host_path, read_only }`.
    pub mounts: Vec<KrunMount>,
    /// The libkrun log level, one of the `KRUN_LOG_LEVEL_*` values.
    pub log_level: u32,
    /// The macOS lifeline descriptor. The driver sets 3 on macOS and `None` on
    /// Linux.
    pub lifeline_fd: Option<i32>,
}

/// The one line that `sandbox-krun --probe` prints.
///
/// The runner builds it; the driver reads it during the host check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KrunProbe {
    /// The library that the runner loaded.
    pub library: String,
    /// The most virtual CPUs that the library gives one machine.
    pub max_vcpus: u8,
}
