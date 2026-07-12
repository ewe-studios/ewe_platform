//! BuildKit gRPC message types — hand-written stubs pending proto codegen.
//!
//! WHY: The `foundation_connectrpc_codegen` pipeline needs to process 17 vendored
//! protos to emit proper `buffa::Message` impls + `ProcedureCodecs`. Until that
//! runs, these stubs satisfy the trait bounds required by `Client<Req, Res>` so
//! the client structure compiles. We use `ProcedureCodecs::only(JsonCodec)` which
//! only needs `Serialize + DeserializeOwned` — no `buffa::Message` required.
//!
//! WHAT: P0 RPC types (Solve, Status, Info) and their nested messages.
//!
//! HOW: Replace this entire module with the output of `protoc-gen-connect-ewe`
//! when codegen is run, then switch the client to `ProcedureCodecs::defaults()`.

use serde::{Deserialize, Serialize};

// =============================================================================
// Control/Info (unary)
// =============================================================================

/// Request for [`super::BuildKitClient::info`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InfoRequest {}

/// Response from BuildKit's `Info` RPC.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InfoResponse {
    /// BuildKit version.
    pub buildkit_version: Option<BuildKitVersion>,
}

/// BuildKit version info.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildKitVersion {
    /// Semantic version string (e.g. "0.12.0").
    pub version: String,
    /// Git revision.
    pub revision: String,
    /// Package name.
    pub package: String,
}

// =============================================================================
// Control/Status (server-stream)
// =============================================================================

/// Request for BuildKit's `Status` RPC.
///
/// Send this once; the server streams [`StatusResponse`]s back.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusRequest {
    /// Ref to filter status by (optional).
    pub r#ref: String,
}

/// A status update from an in-progress build.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusResponse {
    /// Vertex status updates.
    pub vertexes: Vec<Vertex>,
    /// Vertex log output.
    pub logs: Vec<VertexLog>,
    /// Vertex status events.
    pub statuses: Vec<VertexStatus>,
    /// Warnings.
    pub warnings: Vec<VertexWarning>,
}

/// A vertex in the build DAG.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Vertex {
    /// Vertex digest.
    pub digest: String,
    /// Input digests.
    pub inputs: Vec<String>,
    /// Vertex name (e.g. "RUN /bin/sh -c apt-get update").
    pub name: String,
    /// Whether the vertex is cached.
    pub cached: bool,
    /// Start time as Unix nanos.
    pub started: Option<i64>,
    /// Finish time as Unix nanos.
    pub completed: Option<i64>,
    /// Vertex error message.
    pub error: String,
}

/// Log output from a build vertex.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VertexLog {
    /// Vertex digest.
    pub vertex: String,
    /// Timestamp as Unix nanos.
    pub timestamp: i64,
    /// Stream number (1=stdout, 2=stderr).
    pub stream: i64,
    /// Log bytes.
    pub msg: Vec<u8>,
}

/// Status event for a vertex.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VertexStatus {
    /// Vertex id.
    pub id: String,
    /// Vertex digest.
    pub vertex: String,
    /// Status name.
    pub name: String,
    /// Current step (1-based).
    pub current: i64,
    /// Total steps.
    pub total: i64,
    /// Start time as Unix nanos.
    pub timestamp: i64,
    /// Start time for the current step.
    pub started: Option<i64>,
    /// Finish time for the current step.
    pub completed: Option<i64>,
}

/// Warning from a build vertex.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VertexWarning {
    /// Vertex digest.
    pub vertex: String,
    /// Warning severity level.
    pub level: i64,
    /// Short description.
    pub short: Vec<u8>,
    /// Detailed description.
    pub detail: Vec<Vec<u8>>,
    /// URL for more info.
    pub url: String,
    /// Source location.
    pub info: Option<SourceInfo>,
    /// Ranges in source files.
    pub ranges: Vec<Range>,
}

/// Source location info.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceInfo {
    /// File name.
    pub filename: String,
    /// Additional data.
    pub data: Vec<u8>,
    /// Language definition.
    pub definition: Option<Definition>,
    /// Language name.
    pub language: String,
}

/// A language definition.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Definition {
    /// Ops definition bytes.
    pub def: Vec<Vec<u8>>,
    /// Metadata.
    pub metadata: Vec<u8>,
}

/// Source range.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Range {
    /// Start position.
    pub start: Option<Position>,
    /// End position.
    pub end: Option<Position>,
}

/// Position in a file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Position {
    /// 1-indexed line.
    pub line: i32,
    /// 1-indexed character.
    pub character: i32,
}

// =============================================================================
// Control/Solve (bidi-stream)
// =============================================================================

/// Request for BuildKit's `Solve` RPC (bidi-streaming).
///
/// Send one or more requests; the server replies with [`StatusResponse`]s.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SolveRequest {
    /// Inline Dockerfile content (frontend="dockerfile.v0" + inline).
    pub frontend: String,
    /// Frontend attributes.
    pub frontend_attrs: Vec<(String, String)>,
    /// Exporter spec (e.g. "image.name=myimage:tag").
    pub exporter: String,
    /// Exporter attributes.
    pub exporter_attrs: Vec<(String, String)>,
    /// Session identifier.
    pub session: String,
    /// Source policies.
    pub source_policies: Vec<SolveRequestPolicy>,
    /// Cache import refs.
    pub cache_imports: Vec<CacheOptionsEntry>,
    /// Internal flag.
    pub internal: bool,
    /// Entitlements.
    pub entitlements: Vec<String>,
    /// Frontend inputs.
    pub frontend_inputs: Vec<(String, SolveRequestInput)>,
    /// Evaluate only.
    pub evaluate: bool,
}

/// Policy for source resolution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SolveRequestPolicy {
    /// Reference to a policy rule.
    pub ref_: String,
    /// Replacement source ref.
    pub replace: String,
}

/// Cache options entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheOptionsEntry {
    /// Cache type.
    pub r#type: String,
    /// Attributes.
    pub attrs: Vec<(String, String)>,
}

/// Frontend input reference.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SolveRequestInput {
    /// Ref identifier.
    pub ref_: String,
}
/// Response from BuildKit's `Solve` RPC (bidi-streaming).
/// Alias for StatusResponse — the solve stream emits status updates.
pub type SolveResponse = StatusResponse;
