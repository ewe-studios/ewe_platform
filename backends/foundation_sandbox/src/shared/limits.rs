//! Every size and time limit of the platform.
//!
//! A limit is configuration, never a constant in a code path (root rule 8).
//! Each field has a default with the source of that default in its doc comment.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::shared::serde_helpers::duration_millis;

/// Every size and time limit of one sandbox.
///
/// Build a value with [`SandboxLimits::default`] and change the fields that
/// matter with the `with_` methods.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxLimits {
    /// The deadline for one create: the driver launch, the readiness wait and
    /// `Prepare`.
    ///
    /// Default 10 s. Firecracker reaches user space in about 125 ms, and a cold
    /// Cloudflare container takes seconds. 10 s covers both and a slow CI host.
    #[serde(with = "duration_millis")]
    pub boot_timeout: Duration,
    /// The time a graceful stop waits before it forces the substrate down.
    ///
    /// Default 5 s: half of the Docker stop default of 10 s.
    #[serde(with = "duration_millis")]
    pub shutdown_grace: Duration,
    /// The deadline for one unary remote procedure call.
    ///
    /// Default 5 s. The calls are local or one hop.
    #[serde(with = "duration_millis")]
    pub rpc_timeout: Duration,
    /// The largest message that a server reads.
    ///
    /// Default 4 MiB: the default maximum message size of the gRPC protocol.
    pub max_message_bytes: usize,
    /// The largest byte chunk of one event, one input write or one file write.
    ///
    /// Default 64 KiB: the Linux pipe buffer size, so one pipe read is one
    /// event.
    pub chunk_bytes: usize,
    /// The most `Exec` sessions that one slave serves at the same time.
    ///
    /// Default 128: the default `SETTINGS_MAX_CONCURRENT_STREAMS` of nginx for
    /// HTTP/2.
    pub max_exec_sessions: usize,
    /// The most captured bytes for one output stream of `run`.
    ///
    /// Default 1 MiB: the `client_max_body_size` default of nginx.
    pub max_captured_output: usize,
    /// The most bytes of error text that one error carries.
    ///
    /// Default 64 KiB: `BodyLimits::max_error_body` in this repository.
    pub max_error_message: usize,
    /// The time the slave waits for output after a program exits.
    ///
    /// Default 2 s.
    #[serde(with = "duration_millis")]
    pub output_drain_timeout: Duration,
    /// The time an upload may stay idle before the slave removes it.
    ///
    /// Default 60 s: the `client_body_timeout` default of nginx.
    #[serde(with = "duration_millis")]
    pub upload_idle_timeout: Duration,
    /// The time one input write retries for queue space.
    ///
    /// Default 30 s: half of the `send_timeout` default of nginx (60 s).
    #[serde(with = "duration_millis")]
    pub input_retry_timeout: Duration,
    /// The most entries that one image unpack reads.
    ///
    /// Default 1 000 000. Feature 12 uses this limit.
    pub max_unpack_entries: usize,
    /// The most bytes that one image unpack writes.
    ///
    /// Default 16 GiB. The type is `u64` because a 32-bit target cannot hold
    /// 16 GiB in `usize`. Feature 12 uses this limit.
    pub max_unpack_bytes: u64,
    /// The longest path that one image unpack accepts.
    ///
    /// Default 4096: `PATH_MAX` on Linux. A longer path fails the unpack.
    pub max_unpack_path_bytes: usize,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            boot_timeout: Duration::from_secs(10),
            shutdown_grace: Duration::from_secs(5),
            rpc_timeout: Duration::from_secs(5),
            max_message_bytes: 4 * 1024 * 1024,
            chunk_bytes: 64 * 1024,
            max_exec_sessions: 128,
            max_captured_output: 1024 * 1024,
            max_error_message: 64 * 1024,
            output_drain_timeout: Duration::from_secs(2),
            upload_idle_timeout: Duration::from_secs(60),
            input_retry_timeout: Duration::from_secs(30),
            max_unpack_entries: 1_000_000,
            max_unpack_bytes: 16 * 1024 * 1024 * 1024,
            max_unpack_path_bytes: 4096,
        }
    }
}

/// Every size and time limit of one `Sandboxfile` build (feature 14).
///
/// Build a value with [`BuildLimits::default`] and change the fields that
/// matter with the `with_` methods.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildLimits {
    /// The largest build file that the parser reads.
    ///
    /// Default 1 MiB: the `client_max_body_size` default of nginx. A build
    /// file is text, so this is generous.
    pub max_file_bytes: u64,
    /// The most instructions that one build file may hold.
    ///
    /// Default 1000. A build file that needs more steps is a script that
    /// belongs in a `RUN` step.
    pub max_steps: usize,
    /// The most progress values that the builder holds for a slow consumer.
    ///
    /// Default 8: the `proxy_buffers` count default of nginx. The build waits
    /// for room, so a consumer that pauses does not let output collect in
    /// host memory (invariant I12).
    pub progress_queue: usize,
    /// The largest file that one `FETCH` downloads.
    ///
    /// Default 1 GiB: the default of
    /// [`ArtifactLimits::max_download_bytes`].
    pub max_fetch_bytes: u64,
    /// The longest time that one `RUN` step may take.
    ///
    /// Default 30 minutes. A build step that compiles a large program is
    /// slower than a sandbox command, so the value is larger than the
    /// shutdown grace.
    #[serde(with = "duration_millis")]
    pub step_timeout: Duration,
    /// The most bytes that the step cache keeps.
    ///
    /// Default 20 GiB. When the cache passes this size, the builder removes
    /// the snapshots whose access time is oldest, and never one that the
    /// running build named.
    pub cache_max_bytes: u64,
    /// The most bytes that one `ADD` copies from the build context.
    ///
    /// Default 2 GiB: a smaller limit than the image size, because a build
    /// context is source code and data, not a whole root file system.
    pub max_context_bytes: u64,
    /// The most entries that one `ADD` copies.
    ///
    /// Default 100 000. A source tree with more entries is larger than the
    /// default image size in metadata alone.
    pub max_context_entries: usize,
}

impl Default for BuildLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 1024 * 1024,
            max_steps: 1000,
            progress_queue: 8,
            max_fetch_bytes: 1024 * 1024 * 1024,
            step_timeout: Duration::from_mins(30),
            cache_max_bytes: 20 * 1024 * 1024 * 1024,
            max_context_bytes: 2 * 1024 * 1024 * 1024,
            max_context_entries: 100_000,
        }
    }
}

impl BuildLimits {
    /// A set of limits with every default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces [`Self::max_file_bytes`].
    #[must_use]
    pub fn with_max_file_bytes(mut self, value: u64) -> Self {
        self.max_file_bytes = value;
        self
    }

    /// Replaces [`Self::max_steps`].
    #[must_use]
    pub fn with_max_steps(mut self, value: usize) -> Self {
        self.max_steps = value;
        self
    }

    /// Replaces [`Self::progress_queue`].
    #[must_use]
    pub fn with_progress_queue(mut self, value: usize) -> Self {
        self.progress_queue = value;
        self
    }

    /// Replaces [`Self::max_fetch_bytes`].
    #[must_use]
    pub fn with_max_fetch_bytes(mut self, value: u64) -> Self {
        self.max_fetch_bytes = value;
        self
    }

    /// Replaces [`Self::step_timeout`].
    #[must_use]
    pub fn with_step_timeout(mut self, value: Duration) -> Self {
        self.step_timeout = value;
        self
    }

    /// Replaces [`Self::cache_max_bytes`].
    #[must_use]
    pub fn with_cache_max_bytes(mut self, value: u64) -> Self {
        self.cache_max_bytes = value;
        self
    }

    /// Replaces [`Self::max_context_bytes`].
    #[must_use]
    pub fn with_max_context_bytes(mut self, value: u64) -> Self {
        self.max_context_bytes = value;
        self
    }

    /// Replaces [`Self::max_context_entries`].
    #[must_use]
    pub fn with_max_context_entries(mut self, value: usize) -> Self {
        self.max_context_entries = value;
        self
    }

    /// Whether every time and every size limit is more than zero.
    #[must_use]
    pub fn all_positive(&self) -> bool {
        self.max_file_bytes > 0
            && self.max_steps > 0
            && self.progress_queue > 0
            && self.max_fetch_bytes > 0
            && !self.step_timeout.is_zero()
            && self.cache_max_bytes > 0
            && self.max_context_bytes > 0
            && self.max_context_entries > 0
    }
}

/// Every size and time limit of the sandbox image store.
///
/// Build a value with [`ImageLimits::default`] and change the fields that
/// matter with the `with_` methods.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageLimits {
    /// The size of a blank work image in mebibytes.
    ///
    /// Default 4096. The file is sparse, so its disk use is its content; a
    /// guest that fills it gets `ENOSPC` and the error names this limit.
    pub default_size_mib: u32,
    /// The oldest age of a work image that the store removes at start.
    ///
    /// Default 24 h. A work image older than this belongs to a tool that died;
    /// the store removes it at start.
    #[serde(with = "duration_millis")]
    pub work_max_age: Duration,
    /// The longest time that one image import waits for a Docker image pull.
    ///
    /// Default 10 minutes: the default pull deadline of the containerd
    /// container runtime interface (CRI) is 5 minutes, and a full pull gets
    /// twice that (feature 10, section 6.2). The import of feature 13 uses it.
    #[serde(with = "duration_millis")]
    pub pull_timeout: Duration,
    /// The most bytes that one image export may stream into the guest.
    ///
    /// Default 16 GiB: the same value as
    /// [`SandboxLimits::max_unpack_bytes`], the limit that the slave applies.
    /// The importer counts the export chunks and stops one that passes this
    /// limit (feature 13, invariant I1).
    pub max_archive_bytes: u64,
    /// The most bytes that the registry blob cache keeps.
    ///
    /// Default 20 GiB: the same value as
    /// [`BuildLimits::cache_max_bytes`]. When the cache passes this size, the
    /// importer removes the blobs whose last use is oldest, and never one that
    /// the running import named (feature 15, section 6.4).
    pub blob_cache_max_bytes: u64,
    /// The youngest age of a blob that the importer keeps in the cache.
    ///
    /// Default 1 hour. A blob that another import used within this age is not
    /// removed, so a concurrent import keeps the blob that it just read. The
    /// limit bounds the overshoot of the cache to the pulls of one hour.
    #[serde(with = "duration_millis")]
    pub blob_cache_min_age: Duration,
}

impl Default for ImageLimits {
    fn default() -> Self {
        Self {
            default_size_mib: 4096,
            work_max_age: Duration::from_hours(24),
            pull_timeout: Duration::from_mins(10),
            max_archive_bytes: 16 * 1024 * 1024 * 1024,
            blob_cache_max_bytes: 20 * 1024 * 1024 * 1024,
            blob_cache_min_age: Duration::from_hours(1),
        }
    }
}

impl ImageLimits {
    /// A set of limits with every default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces [`Self::default_size_mib`].
    #[must_use]
    pub fn with_default_size_mib(mut self, value: u32) -> Self {
        self.default_size_mib = value;
        self
    }

    /// Replaces [`Self::work_max_age`].
    #[must_use]
    pub fn with_work_max_age(mut self, value: Duration) -> Self {
        self.work_max_age = value;
        self
    }

    /// Replaces [`Self::pull_timeout`].
    #[must_use]
    pub fn with_pull_timeout(mut self, value: Duration) -> Self {
        self.pull_timeout = value;
        self
    }

    /// Replaces [`Self::max_archive_bytes`].
    #[must_use]
    pub fn with_max_archive_bytes(mut self, value: u64) -> Self {
        self.max_archive_bytes = value;
        self
    }

    /// Replaces [`Self::blob_cache_max_bytes`].
    #[must_use]
    pub fn with_blob_cache_max_bytes(mut self, value: u64) -> Self {
        self.blob_cache_max_bytes = value;
        self
    }

    /// Replaces [`Self::blob_cache_min_age`].
    #[must_use]
    pub fn with_blob_cache_min_age(mut self, value: Duration) -> Self {
        self.blob_cache_min_age = value;
        self
    }
}

/// Every size and time limit of the guest-artifact store.
///
/// Build a value with [`ArtifactLimits::default`] and change the fields that
/// matter with the `with_` methods. The store reads it from
/// [`HostConfig`](crate::shared::host::HostConfig)`::artifact_limits`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactLimits {
    /// The largest body that one download writes.
    ///
    /// Default 1 GiB. The largest pinned artifact is the Firecracker kernel at
    /// about 40 MiB; a user image can be larger.
    pub max_download_bytes: u64,
    /// The most bytes that one archive unpack writes.
    ///
    /// Default 4 GiB. The type is `u64` because a 32-bit target cannot hold
    /// 4 GiB in `usize`.
    pub max_unpacked_bytes: u64,
    /// The most entries that one archive unpack reads.
    ///
    /// Default 1 000 000. The Alpine minirootfs has about 500 entries.
    pub max_entries: usize,
    /// The most bytes of an error body that the store reads.
    ///
    /// Default 64 KiB: `BodyLimits::max_error_body` in this repository, the
    /// same value as [`SandboxLimits::max_error_message`].
    pub max_error_message: usize,
    /// The largest buffer that one body read or one file write uses.
    ///
    /// Default 64 KiB: the Linux pipe buffer size. The download holds at most
    /// one buffer of this size.
    pub chunk_bytes: usize,
    /// The deadline for the connection to the artifact server.
    ///
    /// Default 10 s. The transport of Go `net/http` waits 30 s, and the
    /// `dial_timeout` default of Caddy is 5 s. 10 s covers a slow CI host.
    #[serde(with = "duration_millis")]
    pub connect_timeout: Duration,
    /// The longest time that one body read waits for the next bytes.
    ///
    /// Default 60 s: the `client_body_timeout` default of nginx. A body that
    /// sends no byte in this time fails the download.
    #[serde(with = "duration_millis")]
    pub idle_timeout: Duration,
}

impl Default for ArtifactLimits {
    fn default() -> Self {
        Self {
            max_download_bytes: 1024 * 1024 * 1024,
            max_unpacked_bytes: 4 * 1024 * 1024 * 1024,
            max_entries: 1_000_000,
            max_error_message: 64 * 1024,
            chunk_bytes: 64 * 1024,
            connect_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(60),
        }
    }
}

impl ArtifactLimits {
    /// A set of limits with every default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces [`Self::max_download_bytes`].
    #[must_use]
    pub fn with_max_download_bytes(mut self, value: u64) -> Self {
        self.max_download_bytes = value;
        self
    }

    /// Replaces [`Self::max_unpacked_bytes`].
    #[must_use]
    pub fn with_max_unpacked_bytes(mut self, value: u64) -> Self {
        self.max_unpacked_bytes = value;
        self
    }

    /// Replaces [`Self::max_entries`].
    #[must_use]
    pub fn with_max_entries(mut self, value: usize) -> Self {
        self.max_entries = value;
        self
    }

    /// Replaces [`Self::max_error_message`].
    #[must_use]
    pub fn with_max_error_message(mut self, value: usize) -> Self {
        self.max_error_message = value;
        self
    }

    /// Replaces [`Self::chunk_bytes`].
    #[must_use]
    pub fn with_chunk_bytes(mut self, value: usize) -> Self {
        self.chunk_bytes = value;
        self
    }

    /// Replaces [`Self::connect_timeout`].
    #[must_use]
    pub fn with_connect_timeout(mut self, value: Duration) -> Self {
        self.connect_timeout = value;
        self
    }

    /// Replaces [`Self::idle_timeout`].
    #[must_use]
    pub fn with_idle_timeout(mut self, value: Duration) -> Self {
        self.idle_timeout = value;
        self
    }

    /// Whether every time and every size limit is more than zero.
    #[must_use]
    pub fn all_positive(&self) -> bool {
        self.max_download_bytes > 0
            && self.max_unpacked_bytes > 0
            && self.max_entries > 0
            && self.max_error_message > 0
            && self.chunk_bytes > 0
            && !self.connect_timeout.is_zero()
            && !self.idle_timeout.is_zero()
    }
}

impl SandboxLimits {
    /// A set of limits with every default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces [`Self::boot_timeout`].
    #[must_use]
    pub fn with_boot_timeout(mut self, value: Duration) -> Self {
        self.boot_timeout = value;
        self
    }

    /// Replaces [`Self::shutdown_grace`].
    #[must_use]
    pub fn with_shutdown_grace(mut self, value: Duration) -> Self {
        self.shutdown_grace = value;
        self
    }

    /// Replaces [`Self::rpc_timeout`].
    #[must_use]
    pub fn with_rpc_timeout(mut self, value: Duration) -> Self {
        self.rpc_timeout = value;
        self
    }

    /// Replaces [`Self::max_message_bytes`].
    #[must_use]
    pub fn with_max_message_bytes(mut self, value: usize) -> Self {
        self.max_message_bytes = value;
        self
    }

    /// Replaces [`Self::chunk_bytes`].
    #[must_use]
    pub fn with_chunk_bytes(mut self, value: usize) -> Self {
        self.chunk_bytes = value;
        self
    }

    /// Replaces [`Self::max_exec_sessions`].
    #[must_use]
    pub fn with_max_exec_sessions(mut self, value: usize) -> Self {
        self.max_exec_sessions = value;
        self
    }

    /// Replaces [`Self::max_captured_output`].
    #[must_use]
    pub fn with_max_captured_output(mut self, value: usize) -> Self {
        self.max_captured_output = value;
        self
    }

    /// Replaces [`Self::max_error_message`].
    #[must_use]
    pub fn with_max_error_message(mut self, value: usize) -> Self {
        self.max_error_message = value;
        self
    }

    /// Replaces [`Self::output_drain_timeout`].
    #[must_use]
    pub fn with_output_drain_timeout(mut self, value: Duration) -> Self {
        self.output_drain_timeout = value;
        self
    }

    /// Replaces [`Self::upload_idle_timeout`].
    #[must_use]
    pub fn with_upload_idle_timeout(mut self, value: Duration) -> Self {
        self.upload_idle_timeout = value;
        self
    }

    /// Replaces [`Self::input_retry_timeout`].
    #[must_use]
    pub fn with_input_retry_timeout(mut self, value: Duration) -> Self {
        self.input_retry_timeout = value;
        self
    }

    /// Replaces [`Self::max_unpack_entries`].
    #[must_use]
    pub fn with_max_unpack_entries(mut self, value: usize) -> Self {
        self.max_unpack_entries = value;
        self
    }

    /// Replaces [`Self::max_unpack_bytes`].
    #[must_use]
    pub fn with_max_unpack_bytes(mut self, value: u64) -> Self {
        self.max_unpack_bytes = value;
        self
    }

    /// Replaces [`Self::max_unpack_path_bytes`].
    #[must_use]
    pub fn with_max_unpack_path_bytes(mut self, value: usize) -> Self {
        self.max_unpack_path_bytes = value;
        self
    }

    /// Whether every time and every size limit is more than zero.
    #[must_use]
    pub fn all_positive(&self) -> bool {
        !self.boot_timeout.is_zero()
            && !self.shutdown_grace.is_zero()
            && !self.rpc_timeout.is_zero()
            && self.max_message_bytes > 0
            && self.chunk_bytes > 0
            && self.max_exec_sessions > 0
            && self.max_captured_output > 0
            && self.max_error_message > 0
            && !self.output_drain_timeout.is_zero()
            && !self.upload_idle_timeout.is_zero()
            && !self.input_retry_timeout.is_zero()
            && self.max_unpack_entries > 0
            && self.max_unpack_bytes > 0
            && self.max_unpack_path_bytes > 0
    }
}
