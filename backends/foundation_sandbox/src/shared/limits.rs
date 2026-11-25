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
        }
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
    }
}
