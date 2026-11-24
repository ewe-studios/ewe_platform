//! Resource use of one running app.
//!
//! WHY: the proxy shows CPU, memory and restarts for each backend. A value
//! that the platform cannot provide is left out, never guessed (spec 82
//! feature 01, sections 4.5 and 6.12).
//!
//! WHAT: [`MetricsSource`], [`AppMetrics`], [`CgroupMetrics`] (a control group
//! version 2 directory) and [`DockerMetrics`] (one `container_stats` sample).
//!
//! HOW: the cgroup files are read as text; Docker uses the generated
//! `ContainerStatsResponse`. A missing file or a `max` limit leaves the field
//! unset.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::discovery::docker::ContainerApi;
use crate::discovery::RunHandle;
use crate::error::RegistrarError;

/// Resource use of one app.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppMetrics {
    /// CPU time used since the process started, in microseconds.
    pub cpu_usage_micros: Option<u64>,
    /// Resident memory now, in bytes.
    pub memory_bytes: Option<u64>,
    /// The memory limit, in bytes.
    pub memory_limit_bytes: Option<u64>,
}

/// Reads the resource use of one running app.
#[async_trait]
pub trait MetricsSource: Send + Sync {
    /// Read the metrics of `handle`.
    ///
    /// An unreadable metric is left out of the result, not an error.
    ///
    /// # Errors
    /// [`RegistrarError::Metrics`] when the source itself cannot be asked.
    async fn read(&self, handle: &RunHandle) -> Result<AppMetrics, RegistrarError>;
}

/// Reads systemd units from control group version 2 files.
///
/// The path is `<root>/<unit>/cpu.stat`, `memory.current` and `memory.max`;
/// `root` defaults to `/sys/fs/cgroup/system.slice` in
/// [`CgroupMetrics::host`]. A test points `root` at a fixture directory.
#[derive(Debug, Clone)]
pub struct CgroupMetrics {
    root: PathBuf,
}

impl CgroupMetrics {
    /// A source over `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The host source for a systemd system (`/sys/fs/cgroup/system.slice`).
    #[must_use]
    pub fn host() -> Self {
        Self::new("/sys/fs/cgroup/system.slice")
    }

    /// The directory of one unit.
    #[must_use]
    pub fn unit_dir(&self, unit: &str) -> PathBuf {
        self.root.join(unit)
    }
}

#[async_trait]
impl MetricsSource for CgroupMetrics {
    async fn read(&self, handle: &RunHandle) -> Result<AppMetrics, RegistrarError> {
        let RunHandle::Process { unit, .. } = handle else {
            return Ok(AppMetrics::default());
        };
        let directory = self.unit_dir(unit);
        let cpu_usage_micros = read_cgroup_cpu(&directory.join("cpu.stat"));
        let memory_bytes = read_cgroup_value(&directory.join("memory.current"));
        let memory_limit_bytes = read_cgroup_value(&directory.join("memory.max"));
        Ok(AppMetrics {
            cpu_usage_micros,
            memory_bytes,
            memory_limit_bytes,
        })
    }
}

/// Read `usage_usec` from a cgroup `cpu.stat` file.
fn read_cgroup_cpu(path: &std::path::Path) -> Option<u64> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines().find_map(|line| {
        let (key, value) = line.split_once(' ')?;
        if key == "usage_usec" {
            value.trim().parse().ok()
        } else {
            None
        }
    })
}

/// Read one cgroup counter. `max` means "no limit", and a missing file means
/// "not readable"; both leave the value unset.
fn read_cgroup_value(path: &std::path::Path) -> Option<u64> {
    let text = std::fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    if trimmed == "max" {
        return None;
    }
    trimmed.parse().ok()
}

/// Reads Docker container stats through the generated client.
#[derive(Clone)]
pub struct DockerMetrics {
    api: Arc<dyn ContainerApi>,
}

impl std::fmt::Debug for DockerMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockerMetrics").finish_non_exhaustive()
    }
}

impl DockerMetrics {
    /// A source over `api`.
    #[must_use]
    pub fn new(api: Arc<dyn ContainerApi>) -> Self {
        Self { api }
    }
}

#[async_trait]
impl MetricsSource for DockerMetrics {
    async fn read(&self, handle: &RunHandle) -> Result<AppMetrics, RegistrarError> {
        let RunHandle::Container { id } = handle else {
            return Ok(AppMetrics::default());
        };
        let stats = match self.api.container_stats(id).await {
            Ok(stats) => stats,
            // A metric that cannot be read is left out (section 6.12).
            Err(error) => {
                tracing::debug!(%error, container = %id, "container stats unavailable");
                return Ok(AppMetrics::default());
            }
        };
        let cpu_usage_micros = stats
            .cpu_stats
            .as_ref()
            .and_then(|cpu| cpu.cpu_usage.as_ref())
            .and_then(|usage| usage.total_usage)
            .and_then(|nanos| u64::try_from(nanos).ok())
            .map(|nanos| nanos / 1_000);
        let (memory_bytes, memory_limit_bytes) =
            stats.memory_stats.as_ref().map_or((None, None), |memory| {
                (
                    memory.usage.and_then(|value| u64::try_from(value).ok()),
                    memory.limit.and_then(|value| u64::try_from(value).ok()),
                )
            });
        Ok(AppMetrics {
            cpu_usage_micros,
            memory_bytes,
            memory_limit_bytes,
        })
    }
}

/// The metrics source for the deploy kind of one running app.
///
/// A `systemd` unit reads its cgroup files; a Docker container reads one stats
/// sample. An app with neither (a Kubernetes pod without a Docker socket)
/// leaves metrics out.
#[derive(Debug, Clone, Default)]
pub struct CompositeMetrics {
    cgroup: Option<Arc<CgroupMetrics>>,
    docker: Option<Arc<DockerMetrics>>,
}

impl CompositeMetrics {
    /// A source over the cgroup and Docker sources that the host has.
    #[must_use]
    pub fn new(cgroup: Option<Arc<CgroupMetrics>>, docker: Option<Arc<DockerMetrics>>) -> Self {
        Self { cgroup, docker }
    }
}

#[async_trait]
impl MetricsSource for CompositeMetrics {
    async fn read(&self, handle: &RunHandle) -> Result<AppMetrics, RegistrarError> {
        match handle {
            RunHandle::Process { .. } => match &self.cgroup {
                Some(cgroup) => cgroup.read(handle).await,
                None => Ok(AppMetrics::default()),
            },
            RunHandle::Container { .. } => match &self.docker {
                Some(docker) => docker.read(handle).await,
                None => Ok(AppMetrics::default()),
            },
        }
    }
}
