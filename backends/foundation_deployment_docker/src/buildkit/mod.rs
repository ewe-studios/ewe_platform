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
//!                id whose sidecar streams the build context / secrets / auth).
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
    Client, ClientOptions, Ctx, H2PooledTransport, H2Transport, ProcedureCodecs, Request,
    ServerStream, Transport,
};
use std::path::Path;
use std::sync::Arc;

use crate::buildkit::types::{
    DiskUsageRequest, DiskUsageResponse, InfoRequest, InfoResponse, ListWorkersRequest,
    ListWorkersResponse, PruneRequest, SolveRequest, SolveResponse, StatusRequest, StatusResponse,
    UsageRecord,
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
    /// Each RPC opens a **new** TCP connection. Prefer [`connect_tcp_pooled`]
    /// when making multiple calls (e.g. Session + Solve) — it multiplexes H2
    /// streams on one persistent connection, matching how gRPC is designed.
    ///
    /// # Errors
    ///
    /// Returns an error if a client cannot be constructed.
    pub fn connect_tcp(authority: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
        Self::from_transport(transport, authority)
    }

    /// Connect to a `buildkitd` listening on TCP using a **persistent**,
    /// multiplexed H2 connection ([`H2PooledTransport`]).
    ///
    /// Unlike [`connect_tcp`] (new TCP per RPC), this opens ONE TCP connection
    /// and creates new H2 **streams** for each call — matching how gRPC is
    /// designed and how bollard's tonic `Channel` operates. The [`Transport`]
    /// is also accessible via [`transport`](Self::transport) so
    /// [`SessionServer::start_with`] can open the Session bidi on the same
    /// connection as the control-plane RPCs.
    ///
    /// # Errors
    ///
    /// Returns an error if the TCP connect, H2 handshake, or client construction
    /// fails.
    pub fn connect_tcp_pooled(authority: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let transport: Arc<dyn Transport> = Arc::new(H2PooledTransport::connect(authority)?);
        Self::from_transport(transport, authority)
    }

    /// The underlying [`Transport`] this client was built with. Useful for
    /// passing to [`SessionServer`] so the Session bidi shares the same H2
    /// connection as the control-plane RPCs.
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

        Ok(Self { transport, solve, status, info, disk_usage, prune, list_workers })
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
        request: SolveRequest,
    ) -> Result<SolveResponse, Box<dyn std::error::Error + Send + Sync>> {
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
}
