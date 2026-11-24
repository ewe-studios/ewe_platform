//! The generated `RegistrationService` and `RegistrarEnrollmentService`.
//!
//! WHY: the wire contract of section 4 reaches the registry and the enrollment
//! authority through these handlers. One handler instance serves the mutual-TLS
//! (mTLS) listener and another serves the local Unix socket, so the caller's
//! identity comes from a client certificate in one case and from the request in
//! the other.
//!
//! WHAT: [`RegistrationHandler`] (register, report, deregister, renew) and
//! [`EnrollmentHandler`] (enroll), plus the mapping of
//! [`RegistrationError`]/[`EnrollmentError`] to Connect codes.
//!
//! HOW: the mTLS mode reads the peer certificate fingerprint from
//! `RpcCtx.request.connection`, looks the client up in the trust store and takes
//! its `client_name` as the host. A request whose `session.host_id` disagrees is
//! `UNAUTHENTICATED`.

use std::sync::Arc;

use foundation_auth::mtls::{peer_identity, PeerCertificate};
use foundation_connectrpc::{Code, ConnectError, ConnectResult, Request, Response, RpcCtx};
use foundation_errstacks::ErrorTrace;

use super::enroll::{EnrollmentAuthority, EnrollmentError};
use super::proto::v1::registrar_enrollment_service::RegistrarEnrollmentService;
use super::proto::v1::registration_service::RegistrationService;
use super::proto::v1::{
    DeregisterRequest, DeregisterResponse, EnrollRequest, EnrollResponse, RegisterRequest,
    RegisterResponse, RenewCertificateRequest, RenewCertificateResponse, ReportRequest,
    ReportResponse,
};
use super::registry::{BackendRegistry, PeerIdentity, RegistrationError};

/// Where a call arrived, which decides how the peer identity is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    /// The listener requires a client certificate from the registration CA.
    MutualTls,
    /// The local Unix socket; file permissions stand in for the certificate.
    Local,
}

/// Map a registry error to the Connect code of section 4.
#[must_use]
pub fn registration_error(error: &RegistrationError) -> ErrorTrace<ConnectError> {
    let code = match error {
        RegistrationError::Unauthenticated => Code::Unauthenticated,
        RegistrationError::NotOwner { .. } => Code::PermissionDenied,
        RegistrationError::SessionSuperseded { .. } => Code::Aborted,
        RegistrationError::TooManyBackends { .. } => Code::ResourceExhausted,
        RegistrationError::Invalid(_) => Code::InvalidArgument,
    };
    ErrorTrace::new(ConnectError::new(code, error.to_string()))
}

/// Map an enrollment error to the Connect code of section 4.
#[must_use]
pub fn enrollment_error(error: &EnrollmentError) -> ErrorTrace<ConnectError> {
    let code = match error {
        EnrollmentError::Unauthenticated(_) => Code::Unauthenticated,
        EnrollmentError::Invalid(_) => Code::InvalidArgument,
        EnrollmentError::Internal(_) => Code::Internal,
    };
    ErrorTrace::new(ConnectError::new(code, error.to_string()))
}

fn unauthenticated() -> ErrorTrace<ConnectError> {
    ErrorTrace::new(ConnectError::new(
        Code::Unauthenticated,
        "caller is not an enrolled sidecar",
    ))
}

/// The generated `RegistrationService`.
pub struct RegistrationHandler {
    registry: Arc<BackendRegistry>,
    enrollment: Arc<EnrollmentAuthority>,
    mode: AccessMode,
}

impl std::fmt::Debug for RegistrationHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistrationHandler")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl RegistrationHandler {
    /// A handler over `registry` in `mode`.
    #[must_use]
    pub fn new(
        registry: Arc<BackendRegistry>,
        enrollment: Arc<EnrollmentAuthority>,
        mode: AccessMode,
    ) -> Self {
        Self {
            registry,
            enrollment,
            mode,
        }
    }

    /// The peer identity of the call.
    async fn peer(&self, ctx: &RpcCtx) -> Result<PeerIdentity, ErrorTrace<ConnectError>> {
        if self.mode == AccessMode::Local {
            return Ok(PeerIdentity::local());
        }
        let Some(PeerCertificate { fingerprint_sha256 }) = peer_identity(&ctx.request.connection)
        else {
            return Err(unauthenticated());
        };
        let name = self
            .enrollment
            .client_name_for(&fingerprint_sha256)
            .await
            .map_err(|error| enrollment_error(&error))?
            .ok_or_else(unauthenticated)?;
        // A best-effort last-seen write; a failed write must not fail the call.
        if let Err(error) = self.enrollment.touch(&fingerprint_sha256).await {
            tracing::debug!(%error, "registrar last-seen write failed");
        }
        Ok(PeerIdentity::from_certificate(name, fingerprint_sha256))
    }
}

impl RegistrationService for RegistrationHandler {
    async fn register(
        &self,
        ctx: RpcCtx,
        request: Request<RegisterRequest>,
    ) -> ConnectResult<Response<RegisterResponse>> {
        let peer = self.peer(&ctx).await?;
        let response = self
            .registry
            .register(&peer, &request.msg)
            .await
            .map_err(|error| registration_error(&error))?;
        Ok(Response::new(response))
    }

    async fn report(
        &self,
        ctx: RpcCtx,
        request: Request<ReportRequest>,
    ) -> ConnectResult<Response<ReportResponse>> {
        let peer = self.peer(&ctx).await?;
        let response = self
            .registry
            .report(&peer, &request.msg)
            .map_err(|error| registration_error(&error))?;
        Ok(Response::new(response))
    }

    async fn deregister(
        &self,
        ctx: RpcCtx,
        request: Request<DeregisterRequest>,
    ) -> ConnectResult<Response<DeregisterResponse>> {
        let peer = self.peer(&ctx).await?;
        let response = self
            .registry
            .deregister(&peer, &request.msg)
            .map_err(|error| registration_error(&error))?;
        Ok(Response::new(response))
    }

    async fn renew_certificate(
        &self,
        ctx: RpcCtx,
        request: Request<RenewCertificateRequest>,
    ) -> ConnectResult<Response<RenewCertificateResponse>> {
        let peer = self.peer(&ctx).await?;
        let Some(client_name) = peer.host_id() else {
            return Err(ErrorTrace::new(ConnectError::new(
                Code::FailedPrecondition,
                "renewal needs a client certificate",
            )));
        };
        let cert_pem = self
            .enrollment
            .renew(&request.msg.csr_pem, client_name)
            .await
            .map_err(|error| enrollment_error(&error))?;
        Ok(Response::new(RenewCertificateResponse {
            client_cert_pem: cert_pem,
            ..Default::default()
        }))
    }
}

/// The generated `RegistrarEnrollmentService`.
pub struct EnrollmentHandler {
    enrollment: Arc<EnrollmentAuthority>,
}

impl std::fmt::Debug for EnrollmentHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnrollmentHandler").finish_non_exhaustive()
    }
}

impl EnrollmentHandler {
    /// A handler over `enrollment`.
    #[must_use]
    pub fn new(enrollment: Arc<EnrollmentAuthority>) -> Self {
        Self { enrollment }
    }
}

impl RegistrarEnrollmentService for EnrollmentHandler {
    async fn enroll(
        &self,
        _ctx: RpcCtx,
        request: Request<EnrollRequest>,
    ) -> ConnectResult<Response<EnrollResponse>> {
        let response = self
            .enrollment
            .enroll(&request.msg)
            .await
            .map_err(|error| enrollment_error(&error))?;
        Ok(Response::new(response))
    }
}
