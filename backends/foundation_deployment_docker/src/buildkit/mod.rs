//! BuildKit gRPC client — communicates with `buildkitd` over a Unix socket.
//!
//! WHY: BuildKit is Docker's next-gen build engine. Unlike the classic Docker
//! API, BuildKit uses gRPC (`moby.buildkit.v1.Control`) on a Unix socket
//! (`/run/buildkit/buildkitd.sock`). This module wraps our own
//! `foundation_connectrpc::Client<Req, Res>` per service method — no tokio, no
//! bollard.
//!
//! WHAT: [`BuildKitClient`] holds one typed `Client` per RPC we drive:
//!   - `Info`   — unary, buildkitd version/worker info.
//!   - `Solve`  — unary, kick off a build (the request references a `session`
//!     id whose sidecar streams the build context / secrets / auth).
//!   - `Status` — server-streaming, live build progress (vertexes + logs).
//!
//! HOW: [`BuildKitClient::connect`] builds an [`H2Transport`] dialing the
//! buildkitd Unix socket (h2c prior-knowledge — gRPC requires HTTP/2), then
//! constructs each `Client<Req, Res>` with `ProcedureCodecs::defaults()`
//! (proto + json) — the request/response types are real `buffa::Message`
//! types generated from the vendored BuildKit protos (see [`generated`] and
//! `build.rs`). [`connect_tcp`](BuildKitClient::connect_tcp) does the same
//! over TCP (`--addr tcp://…`).
//!
//! ## Scope
//!
//! The `Session` bidi RPC — the callback channel over which buildkitd pulls
//! the build context via FileSync/Auth/Secrets/SSH sidecar services — is
//! implemented in [`session`] ([`session::SessionServer`]). End-to-end local
//! Dockerfile builds work: start a `SessionServer` for the context directory,
//! then `Solve` with `SolveRequest.Session = session.id`.

pub mod generated;
pub mod services;
pub mod session;
pub mod types;

use foundation_connectrpc::{
    Client, ClientOptions, Ctx, H2Transport, ProcedureCodecs, Request,
    ServerStream, Transport,
};
use std::path::Path;
use std::sync::Arc;

use crate::buildkit::types::{
    BuildHistoryEvent, BuildHistoryRequest, DiskUsageRequest, DiskUsageResponse, InfoRequest,
    InfoResponse, ListWorkersRequest, ListWorkersResponse, PruneRequest, SolveRequest,
    SolveResponse, StatusRequest, StatusResponse, UpdateBuildHistoryRequest,
    UpdateBuildHistoryResponse, UsageRecord,
};

/// BuildKit daemon client — gRPC over a Unix socket or TCP.
///
/// Holds one [`Client<Req, Res>`] per gRPC method — all sharing the same
/// h2c [`Transport`] (Unix socket via [`connect`](Self::connect), TCP via
/// [`connect_tcp`](Self::connect_tcp)).
pub struct BuildKitClient {
    /// The transport shared by all per-RPC clients — exposed so callers can
    /// open additional streams (e.g. Session bidi) on the same connection.
    pub transport: Arc<dyn Transport>,
    /// The `host:port` (or placeholder `localhost` for Unix sockets) every
    /// per-RPC URL was built against — kept so additional clients (e.g. the
    /// Gateway API) can target the same endpoint.
    authority: String,
    /// Unary build execution.
    pub solve: Client<SolveRequest, SolveResponse>,
    /// Server-streaming build progress.
    pub status: Client<StatusRequest, StatusResponse>,
    /// Unary server info.
    pub info: Client<InfoRequest, InfoResponse>,
    /// Unary build-cache disk usage.
    pub disk_usage: Client<DiskUsageRequest, DiskUsageResponse>,
    /// Server-streaming build-cache prune.
    pub prune: Client<PruneRequest, UsageRecord>,
    /// Unary worker listing.
    pub list_workers: Client<ListWorkersRequest, ListWorkersResponse>,
    /// Server-streaming build-history feed.
    pub listen_build_history: Client<BuildHistoryRequest, BuildHistoryEvent>,
    /// Unary build-history record update (pin/unpin, delete, finalize).
    pub update_build_history: Client<UpdateBuildHistoryRequest, UpdateBuildHistoryResponse>,
}

/// A fresh random build ref (32 lowercase-hex chars, like buildx's
/// `identity.NewID()`). Set it as `SolveRequest.Ref`, then pass the same
/// value to [`BuildKitClient::status`] to stream that build's progress.
#[must_use]
pub fn new_build_ref() -> String {
    session::new_session_id()
}

impl BuildKitClient {
    /// Connect to `buildkitd` at the given Unix socket path
    /// (e.g. `/run/buildkit/buildkitd.sock` — buildkitd's default listener).
    ///
    /// Uses [`H2Transport::unix`] — h2c prior-knowledge over the Unix domain
    /// socket, since buildkitd's Control service is gRPC and gRPC requires
    /// HTTP/2. The `:authority` pseudo-header is a fixed `localhost` (gRPC
    /// servers on Unix sockets ignore it, matching grpc-go's own dialer).
    ///
    /// # Errors
    ///
    /// Returns an error if a client cannot be constructed. (The socket is
    /// dialed lazily, per call.)
    pub fn connect(socket_path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let transport: Arc<dyn Transport> = Arc::new(H2Transport::unix(socket_path.as_ref()));
        Self::from_transport(transport, "localhost")
    }

    /// Connect to a `buildkitd` listening on TCP (`--addr tcp://host:port`).
    ///
    /// Uses [`H2Transport`] (h2c prior-knowledge) — buildkitd's Control service
    /// is gRPC, which requires HTTP/2. `authority` is the `host:port` buildkitd
    /// is published on (e.g. `127.0.0.1:1234`). buildkitd's TCP listener is
    /// plaintext h2c when started without TLS — intended for local/testbed use.
    ///
    /// Each RPC opens a **new** TCP connection. (A multiplexed shared-connection
    /// transport was prototyped and removed — per-call connections are simpler
    /// and validated end-to-end; buildkitd accepts them fine.)
    ///
    /// # Errors
    ///
    /// Returns an error if a client cannot be constructed.
    pub fn connect_tcp(authority: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
        Self::from_transport(transport, authority)
    }

    /// The underlying [`Transport`] this client was built with. Useful for
    /// passing to [`SessionServer::start_with`] so the Session bidi dials the
    /// same endpoint (e.g. the Unix socket) as the control-plane RPCs.
    #[must_use]
    pub fn transport(&self) -> &Arc<dyn Transport> {
        &self.transport
    }

    /// Build the six per-RPC clients over an already-configured transport.
    fn from_transport(
        transport: Arc<dyn Transport>,
        authority: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let base_url = format!("http://{authority}/moby.buildkit.v1.Control");
        let base_url = base_url.as_str();
        let opts = || ClientOptions::new().with_grpc();

        let info = Client::<InfoRequest, InfoResponse>::new(
            Arc::clone(&transport),
            &format!("{base_url}/Info"),
            ProcedureCodecs::defaults(),
            opts(),
        )?;

        let status = Client::<StatusRequest, StatusResponse>::new(
            Arc::clone(&transport),
            &format!("{base_url}/Status"),
            ProcedureCodecs::defaults(),
            opts(),
        )?;

        let solve = Client::<SolveRequest, SolveResponse>::new(
            Arc::clone(&transport),
            &format!("{base_url}/Solve"),
            ProcedureCodecs::defaults(),
            opts(),
        )?;

        let disk_usage = Client::<DiskUsageRequest, DiskUsageResponse>::new(
            Arc::clone(&transport),
            &format!("{base_url}/DiskUsage"),
            ProcedureCodecs::defaults(),
            opts(),
        )?;

        let prune = Client::<PruneRequest, UsageRecord>::new(
            Arc::clone(&transport),
            &format!("{base_url}/Prune"),
            ProcedureCodecs::defaults(),
            opts(),
        )?;

        let list_workers = Client::<ListWorkersRequest, ListWorkersResponse>::new(
            Arc::clone(&transport),
            &format!("{base_url}/ListWorkers"),
            ProcedureCodecs::defaults(),
            opts(),
        )?;

        let listen_build_history = Client::<BuildHistoryRequest, BuildHistoryEvent>::new(
            Arc::clone(&transport),
            &format!("{base_url}/ListenBuildHistory"),
            ProcedureCodecs::defaults(),
            opts(),
        )?;

        let update_build_history =
            Client::<UpdateBuildHistoryRequest, UpdateBuildHistoryResponse>::new(
                Arc::clone(&transport),
                &format!("{base_url}/UpdateBuildHistory"),
                ProcedureCodecs::defaults(),
                opts(),
            )?;

        Ok(Self {
            transport,
            authority: authority.to_string(),
            solve,
            status,
            info,
            disk_usage,
            prune,
            list_workers,
            listen_build_history,
            update_build_history,
        })
    }

    /// A Gateway (`moby.buildkit.v1.frontend.LLBBridge`) client for the build
    /// identified by `build_ref` — the frontend API buildkitd serves on this
    /// same Control endpoint while a `Frontend = ""` Solve with that `Ref` is
    /// running (the code-first `client.Build` flow). Every call carries the
    /// `buildkit-controlapi-buildid` header that routes it to that build.
    ///
    /// Choreography (mirrors buildkit's `client/build.go`): start the Solve
    /// concurrently, drive gateway calls (`resolve_image_config`, `solve`,
    /// `read_file`, …), then finish with `r#return` — the Solve completes when
    /// the result is returned.
    ///
    /// # Errors
    ///
    /// Returns an error if a client cannot be constructed.
    pub fn gateway_for_build(
        &self,
        build_ref: impl Into<String>,
    ) -> Result<services::gateway::LLBBridgeClient, Box<dyn std::error::Error + Send + Sync>>
    {
        let client = services::gateway::LLBBridgeClient::new(
            Arc::clone(&self.transport),
            &format!("http://{}", self.authority),
            ClientOptions::new()
                .with_grpc()
                .with_header("buildkit-controlapi-buildid".to_string(), build_ref.into()),
        )?;
        Ok(client)
    }

    /// Get buildkitd info (unary): version, worker records, and capabilities.
    ///
    /// # Errors
    ///
    /// Returns a [`ConnectError`](foundation_connectrpc) trace on transport
    /// failure or non-OK gRPC status.
    pub async fn info(&self) -> Result<InfoResponse, Box<dyn std::error::Error + Send + Sync>> {
        let ctx = Ctx::background();
        let response = self.info.unary(ctx, Request::new(InfoRequest::default())).await?;
        Ok(response.msg)
    }

    /// Kick off a build (unary `Solve`).
    ///
    /// The `SolveRequest` carries the frontend (e.g. `dockerfile.v0`), exporter,
    /// and — for builds needing a client-side context — a `session` id. Progress
    /// is observed separately via [`Self::status`] using the same build ref.
    ///
    /// # Errors
    ///
    /// Returns a `ConnectError` trace on transport failure or non-OK gRPC status.
    pub async fn solve(
        &self,
        mut request: SolveRequest,
    ) -> Result<SolveResponse, Box<dyn std::error::Error + Send + Sync>> {
        // buildkitd uses `Ref` as the job id — an empty ref collides with any
        // other in-flight empty-ref solve ("job ID exists"). Always send one.
        if request.Ref.is_empty() {
            request.Ref = new_build_ref();
        }
        let ctx = Ctx::background();
        let response = self.solve.unary(ctx, Request::new(request)).await?;
        Ok(response.msg)
    }

    /// Stream build progress for a given build ref (server-streaming `Status`).
    ///
    /// Returns a [`ServerStream`] — call `.receive().await` in a loop to pull
    /// [`StatusResponse`] updates (vertex state, logs, warnings) until it yields
    /// `None`.
    ///
    /// # Errors
    ///
    /// Returns a `ConnectError` trace on transport failure or non-OK gRPC status.
    pub async fn status(
        &self,
        build_ref: impl Into<String>,
    ) -> Result<ServerStream<StatusResponse>, Box<dyn std::error::Error + Send + Sync>> {
        let ctx = Ctx::background();
        let request = StatusRequest { Ref: build_ref.into(), ..Default::default() };
        let stream = self.status.server_stream(ctx, Request::new(request)).await?;
        Ok(stream)
    }

    /// Report build-cache disk usage (unary `DiskUsage`).
    ///
    /// `filters` are BuildKit cache filters (e.g. `type==regular`); empty for
    /// everything. Matches bollard's buildkit `DiskUsage`.
    ///
    /// # Errors
    ///
    /// Returns a `ConnectError` trace on transport failure or non-OK gRPC status.
    pub async fn disk_usage(
        &self,
        filters: Vec<String>,
    ) -> Result<DiskUsageResponse, Box<dyn std::error::Error + Send + Sync>> {
        let ctx = Ctx::background();
        let request = DiskUsageRequest { filter: filters, ..Default::default() };
        let response = self.disk_usage.unary(ctx, Request::new(request)).await?;
        Ok(response.msg)
    }

    /// List buildkitd workers (unary `ListWorkers`).
    ///
    /// # Errors
    ///
    /// Returns a `ConnectError` trace on transport failure or non-OK gRPC status.
    pub async fn list_workers(
        &self,
        filters: Vec<String>,
    ) -> Result<ListWorkersResponse, Box<dyn std::error::Error + Send + Sync>> {
        let ctx = Ctx::background();
        let request = ListWorkersRequest { filter: filters, ..Default::default() };
        let response = self.list_workers.unary(ctx, Request::new(request)).await?;
        Ok(response.msg)
    }

    /// Prune the build cache (server-streaming `Prune`).
    ///
    /// Returns a [`ServerStream`] of [`UsageRecord`]s — one per reclaimed cache
    /// entry. Call `.receive().await` until it yields `None`. `all` prunes
    /// internal/unshared cache too; `keep_bytes` caps how much is freed.
    ///
    /// # Errors
    ///
    /// Returns a `ConnectError` trace on transport failure or non-OK gRPC status.
    pub async fn prune(
        &self,
        all: bool,
        keep_bytes: i64,
        filters: Vec<String>,
    ) -> Result<ServerStream<UsageRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let ctx = Ctx::background();
        let request = PruneRequest {
            all,
            reservedSpace: keep_bytes,
            filter: filters,
            ..Default::default()
        };
        let stream = self.prune.server_stream(ctx, Request::new(request)).await?;
        Ok(stream)
    }

    /// Stream build-history records (server-streaming `ListenBuildHistory`).
    ///
    /// With `early_exit` the stream replays existing records and ends —
    /// without it, it stays open and follows new builds. `build_ref` filters
    /// to a single record (empty = all).
    ///
    /// # Errors
    ///
    /// Returns a `ConnectError` trace on transport failure or non-OK gRPC status.
    pub async fn listen_build_history(
        &self,
        build_ref: impl Into<String>,
        early_exit: bool,
    ) -> Result<ServerStream<BuildHistoryEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let ctx = Ctx::background();
        let request = BuildHistoryRequest {
            Ref: build_ref.into(),
            EarlyExit: early_exit,
            ..Default::default()
        };
        let stream = self
            .listen_build_history
            .server_stream(ctx, Request::new(request))
            .await?;
        Ok(stream)
    }

    /// Update one build-history record (unary `UpdateBuildHistory`): pin it,
    /// or delete it.
    ///
    /// # Errors
    ///
    /// Returns a `ConnectError` trace on transport failure or non-OK gRPC status.
    pub async fn update_build_history(
        &self,
        build_ref: impl Into<String>,
        pinned: bool,
        delete: bool,
    ) -> Result<UpdateBuildHistoryResponse, Box<dyn std::error::Error + Send + Sync>> {
        let ctx = Ctx::background();
        let request = UpdateBuildHistoryRequest {
            Ref: build_ref.into(),
            Pinned: pinned,
            Delete: delete,
            ..Default::default()
        };
        let response = self
            .update_build_history
            .unary(ctx, Request::new(request))
            .await?;
        Ok(response.msg)
    }
}
