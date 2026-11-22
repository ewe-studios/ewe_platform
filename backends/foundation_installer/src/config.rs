//! The installer configuration.
//!
//! Every path, size limit and timeout is a field, never a constant (root rule
//! 8). Each default names the established server it follows.

use std::path::PathBuf;
use std::time::Duration;

/// Limits and paths of the installer.
///
/// `Default` gives the values that a host install uses. A test or a release
/// tool overrides the paths and the timeouts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallerConfig {
    /// The install record and the lock file. Default `/var/lib/ewe/installer`.
    pub state_dir: PathBuf,
    /// Downloaded files. Default `/var/lib/ewe/artifacts`.
    pub artifact_dir: PathBuf,
    /// The root of the version directories that the installer owns. Default
    /// `/opt/ewe`.
    pub install_root: PathBuf,
    /// The directory of the systemd unit files. Default
    /// `/etc/systemd/system`.
    pub systemd_unit_dir: PathBuf,
    /// Largest manifest file that the installer reads. Default 1 MiB: the
    /// default `client_max_body_size` of nginx.
    pub max_manifest_bytes: u64,
    /// Time to open a download connection. Default 30 s: the dial timeout of
    /// Go's `http.DefaultTransport`.
    pub connect_timeout: Duration,
    /// Longest wait for the next bytes of a download. Default 60 s: the
    /// default `proxy_read_timeout` of nginx.
    pub read_timeout: Duration,
    /// Longest wait for a unit or container to start. Default 90 s: the
    /// default `TimeoutStartSec` of systemd.
    pub start_timeout: Duration,
    /// How often to ask whether a unit or container started. Default 500 ms:
    /// short enough to notice a fast start and gentle on systemd.
    pub start_poll_interval: Duration,
    /// Longest wait for the installer lock. Default 5 s: a second run fails
    /// quickly instead of waiting for a long install.
    pub lock_timeout: Duration,
    /// How often to try the installer lock again. Default 100 ms.
    pub lock_poll_interval: Duration,
    /// Time that Docker gets to stop a container before it is killed. Default
    /// 10 s: the default `--time` of `docker stop`.
    pub stop_container_timeout_secs: u32,
    /// Directories searched for a program that a package manager installed.
    /// Default: the binary directories of a Debian and Ubuntu `PATH`.
    pub program_search_dirs: Vec<PathBuf>,
}

impl Default for InstallerConfig {
    fn default() -> Self {
        Self {
            state_dir: PathBuf::from("/var/lib/ewe/installer"),
            artifact_dir: PathBuf::from("/var/lib/ewe/artifacts"),
            install_root: PathBuf::from("/opt/ewe"),
            systemd_unit_dir: PathBuf::from("/etc/systemd/system"),
            max_manifest_bytes: 1024 * 1024,
            connect_timeout: Duration::from_secs(30),
            read_timeout: Duration::from_secs(60),
            start_timeout: Duration::from_secs(90),
            start_poll_interval: Duration::from_millis(500),
            lock_timeout: Duration::from_secs(5),
            lock_poll_interval: Duration::from_millis(100),
            stop_container_timeout_secs: 10,
            program_search_dirs: vec![
                PathBuf::from("/usr/local/sbin"),
                PathBuf::from("/usr/local/bin"),
                PathBuf::from("/usr/sbin"),
                PathBuf::from("/usr/bin"),
                PathBuf::from("/sbin"),
                PathBuf::from("/bin"),
            ],
        }
    }
}

impl InstallerConfig {
    /// The path of the install record.
    #[must_use]
    pub fn record_path(&self) -> PathBuf {
        self.state_dir.join("installed.json")
    }

    /// The path of the installer lock file.
    #[must_use]
    pub fn lock_path(&self) -> PathBuf {
        self.state_dir.join("installer.lock")
    }
}
