//! systemd discovery through `systemctl show`.
//!
//! WHY: in host mode the installer starts each entry as the unit
//! `ewe-<name>.service` and marks it with `Environment=EWE_ENTRY=<name>` and
//! `Environment=EWE_VERSION=<version>` (spec 79 feature 00, section 4.4).
//!
//! WHAT: [`SystemdDiscovery`], which reads `LoadState`, `ActiveState`,
//! `MainPID`, `Environment`, `NRestarts` and
//! `ExecMainStartTimestampMonotonic`.
//!
//! HOW: the command runs through the injected [`CommandRunner`]. The
//! monotonic start time plus the boot wall time gives the start in Unix
//! milliseconds; the boot wall time comes from `/proc/stat` (`btime`) at the
//! host entry, or from the constructor in a test.

use async_trait::async_trait;

use crate::discovery::{Discovered, Discovery, RunHandle};
use crate::error::RegistrarError;
use crate::manifest::EntryView;
use crate::RegistrarContext;

/// The systemd properties that discovery reads.
const PROPERTIES: &str =
    "LoadState,ActiveState,MainPID,Environment,NRestarts,ExecMainStartTimestampMonotonic";

/// Reads one entry's systemd unit.
pub struct SystemdDiscovery {
    boot_time_unix_ms: u64,
}

impl std::fmt::Debug for SystemdDiscovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemdDiscovery")
            .field("boot_time_unix_ms", &self.boot_time_unix_ms)
            .finish_non_exhaustive()
    }
}

impl SystemdDiscovery {
    /// An adapter with a known boot wall time, for a test that fakes
    /// `systemctl`.
    #[must_use]
    pub fn with_boot_time(boot_time_unix_ms: u64) -> Self {
        Self { boot_time_unix_ms }
    }

    /// The host adapter. It reads the boot wall time from `/proc/stat`.
    ///
    /// # Errors
    /// [`RegistrarError::Discovery`] when `/proc/stat` has no `btime` line.
    pub fn host() -> Result<Self, RegistrarError> {
        let boot_time_unix_ms =
            read_boot_time_unix_ms().ok_or_else(|| RegistrarError::Discovery {
                entry: "<host>".to_string(),
                reason: "cannot read the boot time from /proc/stat".to_string(),
            })?;
        Ok(Self { boot_time_unix_ms })
    }

    /// The unit name that the installer writes.
    #[must_use]
    pub fn unit_name(entry: &str) -> String {
        format!("ewe-{entry}.service")
    }
}

#[async_trait]
impl Discovery for SystemdDiscovery {
    fn kind(&self) -> &'static str {
        "systemd"
    }

    async fn discover(
        &self,
        entry: &EntryView,
        ctx: &RegistrarContext,
    ) -> Result<Discovered, RegistrarError> {
        let unit = Self::unit_name(&entry.name);
        let args = vec![
            "show".to_string(),
            unit.clone(),
            "-p".to_string(),
            PROPERTIES.to_string(),
        ];
        let output = ctx
            .commands
            .run("systemctl", &args)
            .await
            .map_err(|error| RegistrarError::Discovery {
                entry: entry.name.clone(),
                reason: error.to_string(),
            })?;

        let properties = parse_properties(&output.stdout);
        let load_state = properties.get("LoadState").map_or("", String::as_str);
        if !output.success() || matches!(load_state, "not-found" | "masked" | "error" | "") {
            return Ok(Discovered::NotInstalled);
        }
        let Some((marked_entry, version)) = parse_marks(properties.get("Environment")) else {
            return Ok(Discovered::NotInstalled);
        };
        if marked_entry != entry.name {
            return Ok(Discovered::NotInstalled);
        }

        let active = properties.get("ActiveState").map(String::as_str) == Some("active");
        let pid = properties
            .get("MainPID")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        if !active || pid == 0 {
            return Ok(Discovered::Stopped { version });
        }

        let started_at_unix_ms = properties
            .get("ExecMainStartTimestampMonotonic")
            .and_then(|value| value.parse::<u64>().ok())
            .map_or(0, |micros| {
                self.boot_time_unix_ms.saturating_add(micros / 1_000)
            });
        let restarts = properties
            .get("NRestarts")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        Ok(Discovered::Running {
            version,
            handle: RunHandle::Process {
                pid,
                unit: unit.clone(),
            },
            started_at_unix_ms,
            restarts,
            container_ip: None,
        })
    }
}

/// Parse `Key=Value` lines from `systemctl show`.
fn parse_properties(stdout: &str) -> std::collections::BTreeMap<String, String> {
    stdout
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// Parse `EWE_ENTRY=<name> EWE_VERSION=<version>` out of the `Environment`
/// property. systemd quotes a value only when it holds a space.
fn parse_marks(environment: Option<&String>) -> Option<(String, String)> {
    let environment = environment?;
    let mut entry = None;
    let mut version = None;
    for token in environment.split_whitespace() {
        // `systemctl show` keeps a unit's quoting when a value needs it, so a
        // token may arrive as `"EWE_ENTRY=agent-rpc"`.
        let token = token.trim_matches('"');
        if let Some(value) = token.strip_prefix("EWE_ENTRY=") {
            entry = Some(value.to_string());
        } else if let Some(value) = token.strip_prefix("EWE_VERSION=") {
            version = Some(value.to_string());
        }
    }
    Some((entry?, version?))
}

/// The wall-clock time of the boot, from the `btime` line of `/proc/stat`.
fn read_boot_time_unix_ms() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/stat").ok()?;
    let seconds = text
        .lines()
        .find_map(|line| line.strip_prefix("btime "))?
        .trim()
        .parse::<u64>()
        .ok()?;
    Some(seconds.saturating_mul(1_000))
}
