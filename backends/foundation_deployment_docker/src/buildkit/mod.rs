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
//! HOW: [`BuildKitClient::connect`] builds an [`H1Transport`] over a
//! `DynNetClient` dialing the buildkitd socket, then constructs each
//! `Client<Req, Res>` with `ProcedureCodecs::defaults()` (proto + json) — the
//! request/response types are real `buffa::Message` types generated from the
//! vendored BuildKit protos (see [`generated`] and `build.rs`).
//!
//! ## Scope
//!
//! The `Session` bidi RPC (the callback channel over which buildkitd pulls the
//! build context via FileSync/Auth/Secrets/SSH sidecar services) is not yet
//! implemented — a `Solve` therefore only succeeds for builds that need no
//! client-provided session (e.g. a fully remote context). Wiring the session
//! sidecar is the remaining step for end-to-end local Dockerfile builds.

pub mod generated;
pub mod types;

use foundation_connectrpc::{
    Client, ClientOptions, Ctx, H1Transport, ProcedureCodecs, Request, ServerStream, Transport,
};
use foundation_netio::{DynNetClient, HttpClientBuilder};
use std::path::Path;
use std::sync::Arc;

use crate::buildkit::types::{
    InfoRequest, InfoResponse, SolveRequest, SolveResponse, StatusRequest, StatusResponse,
};

/// BuildKit daemon client over a Unix socket.
///
/// Holds one [`Client<Req, Res>`] per gRPC method — all sharing the same
/// `H1Transport` (which wraps a `DynNetClient` dialing the buildkitd socket).
pub struct BuildKitClient {
    /// Unary build execution.
    pub solve: Client<SolveRequest, SolveResponse>,
    /// Server-streaming build progress.
    pub status: Client<StatusRequest, StatusResponse>,
    /// Unary server info.
    pub info: Client<InfoRequest, InfoResponse>,
}

impl BuildKitClient {
    /// Connect to `buildkitd` at the given Unix socket path.
    ///
    /// # Errors
    ///
    /// Returns an error if the socket cannot be dialed or a client cannot be
    /// constructed.
    pub fn connect(socket_path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let path = socket_path.as_ref();
        let http: DynNetClient = HttpClientBuilder::new().unix_socket(path).build();

        let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(http));

        let base_url = "http://localhost/moby.buildkit.v1.Control";
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
            transport,
            &format!("{base_url}/Solve"),
            ProcedureCodecs::defaults(),
            opts(),
        )?;

        Ok(Self { solve, status, info })
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
}
