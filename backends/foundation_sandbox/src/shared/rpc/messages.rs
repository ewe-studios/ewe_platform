//! Every request and response type of the two RPC services.
//!
//! A field with raw bytes is base64 text in JSON.

use serde::{Deserialize, Serialize};

use crate::shared::driver::{CreateProgress, HostReport};
use crate::shared::serde_helpers::base64_bytes;
use crate::shared::spec::SandboxSpec;

pub use crate::shared::control::{ExitReport, SandboxInfo, SandboxState};
pub use crate::shared::spec::{FsType, GuestMount, RootMode, RootSetup, RootSource};

/// An empty request or response.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Empty {}

/// The request of `Info`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfoRequest {}

/// What the slave reports about itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlaveInfo {
    /// The slave version.
    pub slave_version: String,
    /// The architecture: `x86_64` or `aarch64`.
    pub arch: String,
    /// The kernel release from `uname`.
    pub kernel: String,
    /// Whether the slave is process identifier 1.
    pub is_pid1: bool,
    /// Whether the slave received `Prepare`.
    pub prepared: bool,
    /// Whether the slave also serves `SandboxService` (feature 11).
    pub can_control: bool,
}

/// The request of `Prepare`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrepareRequest {
    /// Where the root comes from.
    pub root: RootSetup,
    /// The work directory.
    pub workdir: String,
    /// The base environment.
    pub env: Vec<(String, String)>,
    /// The guest mounts.
    pub mounts: Vec<GuestMount>,
    /// The DNS servers. Empty means that the slave leaves
    /// `/etc/resolv.conf` alone.
    pub dns: Vec<String>,
    /// The limits of the slave.
    pub limits: SlaveLimits,
}

/// The limits that the control plane gives the slave.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlaveLimits {
    /// The largest byte chunk.
    pub chunk_bytes: u32,
    /// The most open sessions.
    pub max_exec_sessions: u32,
    /// The output drain time in milliseconds.
    pub output_drain_timeout_ms: u32,
    /// The upload idle time in milliseconds.
    pub upload_idle_timeout_ms: u32,
    /// The input queue size in bytes. It is four times `chunk_bytes`.
    pub input_queue_bytes: u32,
    /// The capacity of the standard-input pipe in bytes.
    ///
    /// The kernel rounds it up to a page multiple. Zero keeps the system
    /// default. Default 64 KiB: the Linux pipe default (16 pages).
    #[serde(default = "default_input_pipe_bytes")]
    pub input_pipe_bytes: u32,
    /// The largest request that the server reads.
    pub max_message_bytes: u32,
    /// The events held for one `Exec` stream before the pumps wait. Default 4.
    pub event_queue_len: u32,
    /// The most entries that one image unpack applies (feature 12).
    pub max_unpack_entries: u32,
    /// The most bytes that one image unpack writes (feature 12).
    pub max_unpack_bytes: u64,
    /// The longest path that one image unpack accepts (feature 12).
    pub max_unpack_path_bytes: u32,
}

/// The default of [`SlaveLimits::input_pipe_bytes`]: 64 KiB, the Linux pipe
/// default of 16 pages.
fn default_input_pipe_bytes() -> u32 {
    64 * 1024
}

/// The response of `Prepare`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrepareResponse {
    /// The root mode that the slave applied.
    pub root_mode: RootMode,
}

/// The request of `Exec`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecRequest {
    /// The execution identifier that the caller chose, in `ExecId` shape.
    pub exec_id: String,
    /// The argument vector.
    pub argv: Vec<String>,
    /// The environment entries.
    pub env: Vec<(String, String)>,
    /// The working directory, or `None` for the sandbox work directory.
    pub cwd: Option<String>,
    /// Whether the program reads input. `false` uses `/dev/null`.
    pub stdin: bool,
    /// The user of the program, in the forms of feature 12. `None` runs as
    /// root, or as the image user when the command has no user.
    #[serde(default)]
    pub user: Option<String>,
}

/// One event of a running program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecEventMsg {
    /// The sequence number. It starts at zero and grows by one.
    pub seq: u64,
    /// The event.
    pub event: ExecEventKind,
}

/// The event kinds of a running program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecEventKind {
    /// The program started. The sequence number is always zero.
    Started {
        /// The process identifier in the guest.
        pid: u32,
    },
    /// Bytes on standard output.
    Stdout {
        /// The bytes.
        #[serde(with = "base64_bytes")]
        data: Vec<u8>,
    },
    /// Bytes on standard error.
    Stderr {
        /// The bytes.
        #[serde(with = "base64_bytes")]
        data: Vec<u8>,
    },
    /// The program ended. It is always the last event.
    Exit {
        /// The exit code, when the program exited with one.
        code: Option<i32>,
        /// The signal, when a signal ended the program.
        signal: Option<i32>,
    },
}

/// The request of `ExecInput`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecInputRequest {
    /// The execution identifier.
    pub exec_id: String,
    /// The sequence number of this input chunk.
    pub seq: u64,
    /// The input bytes.
    #[serde(with = "base64_bytes")]
    pub data: Vec<u8>,
    /// Whether this chunk ends the input.
    pub eof: bool,
}

/// The response of `ExecInput`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecInputResponse {
    /// The next sequence number that the slave accepts.
    pub next_seq: u64,
}

/// The request of `ExecSignal`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecSignalRequest {
    /// The execution identifier.
    pub exec_id: String,
    /// The signal number.
    pub signal: i32,
}

/// The request of `FileWriteBegin`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileWriteBeginRequest {
    /// The target path in the guest.
    pub path: String,
    /// The file mode.
    pub mode: u32,
}

/// The response of `FileWriteBegin`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileWriteBeginResponse {
    /// The upload identifier.
    pub upload_id: String,
}

/// The request of `FileWriteChunk`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileWriteChunkRequest {
    /// The upload identifier.
    pub upload_id: String,
    /// The byte offset of this chunk.
    pub offset: u64,
    /// The bytes.
    #[serde(with = "base64_bytes")]
    pub data: Vec<u8>,
}

/// The response of `FileWriteChunk`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileWriteChunkResponse {
    /// The next offset that the slave accepts.
    pub next_offset: u64,
}

/// The request of `FileWriteCommit`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileWriteCommitRequest {
    /// The upload identifier.
    pub upload_id: String,
    /// The final size in bytes.
    pub size: u64,
}

/// The response of `FileWriteCommit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileWriteCommitResponse {
    /// The size of the written file.
    pub size: u64,
}

/// The request of `FileWriteAbort`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileWriteAbortRequest {
    /// The upload identifier.
    pub upload_id: String,
}

/// The request of `UnpackBegin` (feature 12).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnpackBeginRequest {
    /// The target directory in the guest.
    pub dest: String,
    /// Whether OCI whiteout entries remove earlier entries.
    pub oci_whiteouts: bool,
    /// Exact paths, relative to `dest`, that the reader drops.
    #[serde(default)]
    pub skip: Vec<String>,
    /// Keep the mode of a directory that already exists (feature 14).
    #[serde(default)]
    pub keep_existing_dirs: bool,
}

/// The response of `UnpackBegin`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnpackBeginResponse {
    /// The unpack identifier.
    pub upload_id: String,
}

/// The request of `UnpackChunk`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnpackChunkRequest {
    /// The unpack identifier.
    pub upload_id: String,
    /// The byte offset of this chunk.
    pub offset: u64,
    /// The uncompressed tar bytes.
    #[serde(with = "base64_bytes")]
    pub data: Vec<u8>,
}

/// The response of `UnpackChunk`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnpackChunkResponse {
    /// The next offset that the slave accepts.
    pub next_offset: u64,
}

/// The request of `UnpackCommit`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnpackCommitRequest {
    /// The unpack identifier.
    pub upload_id: String,
    /// The final size of the tar stream in bytes.
    pub size: u64,
}

/// The response of `UnpackCommit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnpackCommitResponse {
    /// The number of applied entries.
    pub entries: u64,
    /// The number of file data bytes.
    pub bytes: u64,
}

/// The request of `UnpackAbort`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnpackAbortRequest {
    /// The unpack identifier.
    pub upload_id: String,
}

/// The request of `FileRead`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileReadRequest {
    /// The path in the guest.
    pub path: String,
}

/// One chunk of a file read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChunk {
    /// The byte offset of this chunk.
    pub offset: u64,
    /// The bytes.
    #[serde(with = "base64_bytes")]
    pub data: Vec<u8>,
    /// Whether this chunk is the last one.
    pub eof: bool,
}

/// The request of `Shutdown`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShutdownRequest {
    /// The grace period in milliseconds.
    pub grace_ms: u32,
}

/// One message of a service, with the identifier of its sandbox.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scoped<T> {
    /// The sandbox identifier.
    pub sandbox_id: String,
    /// The message of `SlaveService`.
    #[serde(flatten)]
    pub inner: T,
}

/// A reference to one sandbox.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxRef {
    /// The sandbox identifier.
    pub sandbox_id: String,
}

/// The request of `Create`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateRequest {
    /// The spec of the sandbox.
    pub spec: SandboxSpec,
}

/// One event of `Create`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[allow(
    clippy::large_enum_variant,
    reason = "the created sandbox carries its manifest; a create sends one event, so the box would save nothing"
)]
pub enum CreateEvent {
    /// The create reached a progress step.
    Progress(CreateProgress),
    /// The sandbox is ready. This event is last.
    Created(SandboxInfo),
}

/// The response of `Probe`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeResponse {
    /// One report for each registered driver.
    pub reports: Vec<HostReport>,
}

/// The response of `List`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListResponse {
    /// The live sandboxes.
    pub sandboxes: Vec<SandboxInfo>,
}
