//! The Firecracker API client over a Unix domain socket.
//!
//! WHY: The control plane tells a Firecracker process how the virtual machine
//! looks, starts it and asks for its state. The generated request functions
//! give the typed calls; this module adds the transport, the time limit and the
//! mapping to [`SandboxError`].
//!
//! WHAT: [`FirecrackerClient`] holds one `DynNetClient` over the socket, the
//! generated client with its body limits, and the injected timer.
//!
//! HOW: Each method calls the matching generated `*_request` function and races
//! its future with a timer at `rpc_timeout` on the clock of the [`Ctx`]. A
//! non-2xx answer becomes [`SandboxError::SubstrateApi`] with the vendor's
//! `fault_message`; a silent server becomes [`SandboxError::ApiTimeout`].

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use foundation_context::env::Timer;
use foundation_context::Ctx;
use foundation_netio::HttpClientBuilder;
use futures::future::{select, Either};

use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::limits::SandboxLimits;

use super::generated::actions::{
    create_sync_action_request, CreateSyncActionArgs, InstanceActionInfo,
    InstanceActionInfoActionType,
};
use super::generated::boot_source::{
    put_guest_boot_source_request, BootSource, PutGuestBootSourceArgs,
};
use super::generated::default::{describe_instance_request, DescribeInstanceArgs, InstanceInfo};
use super::generated::drives::{put_guest_drive_by_id_request, Drive, PutGuestDriveByIDArgs};
use super::generated::machine_config::{
    put_machine_configuration_request, MachineConfiguration, PutMachineConfigurationArgs,
};
// The error schema is the same `Error { fault_message }` for every operation.
use super::generated::machine_config::Error as ApiFault;
use super::generated::shared::{ApiClient, ApiError, ApiResponse, BodyLimits};
use super::generated::version::{
    get_firecracker_version_request, FirecrackerVersion, GetFirecrackerVersionArgs,
};
use super::generated::vsock::{put_guest_vsock_request, PutGuestVsockArgs, Vsock};

/// The base URL of the request line. The transport dials the Unix socket and
/// ignores the host; the generated functions still need a valid URL.
const BASE_URL: &str = "http://localhost";

/// A closure type for the (empty) `builder_mod` argument — no request mutation.
type NoMod = fn(&mut foundation_netio::PreparedRequestBuilder);

/// Every size and time limit of the Firecracker API client.
///
/// Build a value with [`FirecrackerLimits::default`] and change the fields that
/// matter with the `with_` methods. [`FirecrackerClient::new`] derives one from
/// the sandbox limits; [`FirecrackerClient::with_limits`] takes this type
/// directly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FirecrackerLimits {
    /// The most bytes of a non-2xx answer that the client reads for its
    /// message.
    ///
    /// Default 64 KiB: the [`BodyLimits::max_error_body`] default, and the
    /// same value as [`SandboxLimits::max_error_message`].
    pub max_error_message: usize,
    /// The largest JSON success answer that the generated request reads.
    ///
    /// Default 1 MiB: the [`BodyLimits::max_json_body`] default (nginx's
    /// `client_max_body_size`). A Firecracker answer is a short document.
    pub max_json_body: usize,
    /// The deadline for one unary call on the clock of the [`Ctx`].
    ///
    /// Default 5 s: the [`SandboxLimits::rpc_timeout`] default. The calls are
    /// local to one host.
    pub rpc_timeout: Duration,
    /// The longest accepted socket path in bytes.
    ///
    /// Default 107: the Linux `sockaddr_un.sun_path` array is 108 bytes, and
    /// one byte is the terminating NUL. Firecracker runs on Linux.
    pub max_socket_path: usize,
    /// The longest one transport read waits for bytes.
    ///
    /// Default 60 s: nginx's `send_timeout` default. It is longer than
    /// `rpc_timeout` on purpose, so the injected timer ends a silent call first
    /// and this bound is only a safety net for a broken timer.
    pub transport_timeout: Duration,
}

impl Default for FirecrackerLimits {
    fn default() -> Self {
        let body = BodyLimits::default();
        Self {
            max_error_message: body.max_error_body,
            max_json_body: body.max_json_body,
            rpc_timeout: Duration::from_secs(5),
            max_socket_path: 107,
            transport_timeout: Duration::from_secs(60),
        }
    }
}

impl FirecrackerLimits {
    /// The limits of one sandbox: the shared error-message and call-time limits,
    /// with the API-specific defaults for the rest.
    #[must_use]
    pub fn from_sandbox(limits: &SandboxLimits) -> Self {
        Self {
            max_error_message: limits.max_error_message,
            rpc_timeout: limits.rpc_timeout,
            ..Self::default()
        }
    }

    /// Read at most `bytes` of an error body.
    #[must_use]
    pub fn with_max_error_message(mut self, bytes: usize) -> Self {
        self.max_error_message = bytes;
        self
    }

    /// Read at most `bytes` of a JSON success body.
    #[must_use]
    pub fn with_max_json_body(mut self, bytes: usize) -> Self {
        self.max_json_body = bytes;
        self
    }

    /// End one call after `timeout` on the clock of the `Ctx`.
    #[must_use]
    pub fn with_rpc_timeout(mut self, timeout: Duration) -> Self {
        self.rpc_timeout = timeout;
        self
    }

    /// Accept a socket path of at most `bytes`.
    #[must_use]
    pub fn with_max_socket_path(mut self, bytes: usize) -> Self {
        self.max_socket_path = bytes;
        self
    }

    /// Wait at most `timeout` for one transport read.
    #[must_use]
    pub fn with_transport_timeout(mut self, timeout: Duration) -> Self {
        self.transport_timeout = timeout;
        self
    }

    /// The limits the generated request functions read answers under.
    #[must_use]
    pub fn body_limits(&self) -> BodyLimits {
        BodyLimits::default()
            .with_max_error_body(self.max_error_message)
            .with_max_json_body(self.max_json_body)
    }
}

/// The Firecracker API client over one Unix domain socket.
///
/// One client talks to one Firecracker process. It is cheap to clone: the
/// transport and the timer are shared.
#[derive(Clone)]
pub struct FirecrackerClient {
    /// The generated client: the transport and the body limits.
    api: ApiClient,
    /// The `--api-sock` path this client dials.
    socket: PathBuf,
    /// The limits of every call.
    limits: FirecrackerLimits,
    /// The injected timer that ends a silent call.
    timer: Arc<dyn Timer>,
    /// The context, for the clock that measures the deadline.
    ctx: Ctx,
}

impl std::fmt::Debug for FirecrackerClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FirecrackerClient")
            .field("socket", &self.socket)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl FirecrackerClient {
    /// A client for the API socket at `socket`.
    ///
    /// `socket` is the `--api-sock` path. The shared sandbox limits give
    /// `rpc_timeout` and `max_error_message`; the other limits keep their
    /// documented default.
    ///
    /// # Errors
    /// [`SandboxError::SocketPathTooLong`] when the path does not fit a Unix
    /// socket address, and [`SandboxError::HostNotConfigured`] when the context
    /// holds no timer.
    pub fn new(ctx: &Ctx, socket: &Path, limits: &SandboxLimits) -> SandboxResult<Self> {
        Self::with_limits(ctx, socket, &FirecrackerLimits::from_sandbox(limits))
    }

    /// A client with explicit Firecracker limits.
    ///
    /// # Errors
    /// As [`FirecrackerClient::new`].
    pub fn with_limits(
        ctx: &Ctx,
        socket: &Path,
        limits: &FirecrackerLimits,
    ) -> SandboxResult<Self> {
        use std::os::unix::ffi::OsStrExt as _;

        let path_len = socket.as_os_str().as_bytes().len();
        if path_len > limits.max_socket_path {
            return Err(SandboxError::SocketPathTooLong {
                path: socket.display().to_string(),
                max: limits.max_socket_path,
            }
            .into());
        }
        let timer = ctx
            .require_env()
            .map_err(|_| SandboxError::HostNotConfigured)?
            .timer();
        let http = HttpClientBuilder::new()
            .unix_socket(socket)
            .read_timeout(limits.transport_timeout)
            // Firecracker sends no `100 Continue`; waiting for one would delay
            // every request that carries a body.
            .expect_continue(false)
            .build();
        let api = ApiClient::new(http).with_limits(limits.body_limits());
        Ok(Self {
            api,
            socket: socket.to_path_buf(),
            limits: limits.clone(),
            timer,
            ctx: ctx.clone(),
        })
    }

    /// The API socket path this client dials.
    #[must_use]
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// The limits of every call.
    #[must_use]
    pub fn limits(&self) -> &FirecrackerLimits {
        &self.limits
    }

    /// General information about the instance (`GET /`). A read-only call.
    ///
    /// # Errors
    /// [`SandboxError::SubstrateApi`] for a non-2xx answer,
    /// [`SandboxError::ApiTimeout`] for a silent server, and
    /// [`SandboxError::Io`] for a transport or parse failure.
    pub async fn describe_instance(&self) -> SandboxResult<InstanceInfo> {
        let args = DescribeInstanceArgs::default();
        let response = self
            .call(describe_instance_request(
                self.api.clone(),
                &args,
                BASE_URL,
                None::<NoMod>,
            ))
            .await?;
        Ok(response.body)
    }

    /// The Firecracker build version (`GET /version`). A read-only call.
    ///
    /// # Errors
    /// As [`FirecrackerClient::describe_instance`].
    pub async fn version(&self) -> SandboxResult<FirecrackerVersion> {
        let args = GetFirecrackerVersionArgs::default();
        let response = self
            .call(get_firecracker_version_request(
                self.api.clone(),
                &args,
                BASE_URL,
                None::<NoMod>,
            ))
            .await?;
        Ok(response.body)
    }

    /// Set the machine configuration (`PUT /machine-config`).
    ///
    /// # Errors
    /// As [`FirecrackerClient::describe_instance`].
    pub async fn put_machine_config(&self, body: &MachineConfiguration) -> SandboxResult<()> {
        let args = PutMachineConfigurationArgs { body: body.clone() };
        self.call(put_machine_configuration_request(
            self.api.clone(),
            &args,
            BASE_URL,
            None::<NoMod>,
        ))
        .await?;
        Ok(())
    }

    /// Set the boot source (`PUT /boot-source`).
    ///
    /// # Errors
    /// As [`FirecrackerClient::describe_instance`].
    pub async fn put_boot_source(&self, body: &BootSource) -> SandboxResult<()> {
        let args = PutGuestBootSourceArgs { body: body.clone() };
        self.call(put_guest_boot_source_request(
            self.api.clone(),
            &args,
            BASE_URL,
            None::<NoMod>,
        ))
        .await?;
        Ok(())
    }

    /// Add or replace a drive (`PUT /drives/{drive_id}`). The path takes the
    /// `drive_id` of the body.
    ///
    /// # Errors
    /// As [`FirecrackerClient::describe_instance`].
    pub async fn put_drive(&self, body: &Drive) -> SandboxResult<()> {
        let args = PutGuestDriveByIDArgs {
            drive_id: body.drive_id.clone(),
            body: body.clone(),
        };
        self.call(put_guest_drive_by_id_request(
            self.api.clone(),
            &args,
            BASE_URL,
            None::<NoMod>,
        ))
        .await?;
        Ok(())
    }

    /// Set the vsock device (`PUT /vsock`).
    ///
    /// # Errors
    /// As [`FirecrackerClient::describe_instance`].
    pub async fn put_vsock(&self, body: &Vsock) -> SandboxResult<()> {
        let args = PutGuestVsockArgs { body: body.clone() };
        self.call(put_guest_vsock_request(
            self.api.clone(),
            &args,
            BASE_URL,
            None::<NoMod>,
        ))
        .await?;
        Ok(())
    }

    /// Start the instance (`PUT /actions` with `InstanceStart`).
    ///
    /// # Errors
    /// As [`FirecrackerClient::describe_instance`].
    pub async fn instance_start(&self) -> SandboxResult<()> {
        let args = CreateSyncActionArgs {
            body: InstanceActionInfo {
                action_type: InstanceActionInfoActionType::InstanceStart,
            },
        };
        self.call(create_sync_action_request(
            self.api.clone(),
            &args,
            BASE_URL,
            None::<NoMod>,
        ))
        .await?;
        Ok(())
    }

    /// Races one generated request with the call deadline and maps its error.
    async fn call<T, F>(&self, request: F) -> SandboxResult<ApiResponse<T>>
    where
        F: Future<Output = Result<ApiResponse<T>, ApiError>> + Send,
    {
        let deadline = self.ctx.clock().now() + self.limits.rpc_timeout;
        let timer = self.timer.sleep_until(self.ctx.clone(), deadline);
        match select(Box::pin(request), timer).await {
            Either::Left((result, _)) => result.map_err(|error| Self::map_error(error).into()),
            Either::Right(((), _)) => Err(SandboxError::ApiTimeout {
                timeout_ms: u64::try_from(self.limits.rpc_timeout.as_millis()).unwrap_or(u64::MAX),
            }
            .into()),
        }
    }

    /// Maps a generated API error to the typed sandbox error.
    ///
    /// A non-2xx answer carries the vendor's body. Firecracker sends
    /// `{"fault_message": "..."}`; the body is read at most
    /// `max_error_message` bytes, and an unparseable body reaches the caller as
    /// that bounded text.
    fn map_error(error: ApiError) -> SandboxError {
        match error {
            ApiError::HttpStatus { code, body, .. } => {
                let message = body
                    .as_deref()
                    .and_then(|text| serde_json::from_str::<ApiFault>(text).ok())
                    .and_then(|fault| fault.fault_message)
                    .or(body)
                    .unwrap_or_default();
                SandboxError::SubstrateApi {
                    status: code,
                    message,
                }
            }
            ApiError::RequestBuildFailed(message) | ApiError::RequestSendFailed(message) => {
                SandboxError::Io {
                    op: "firecracker api call".to_string(),
                    message,
                }
            }
            ApiError::ParseFailed(message) => SandboxError::Io {
                op: "firecracker api response".to_string(),
                message,
            },
        }
    }
}
