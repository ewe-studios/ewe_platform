//! The Docker Engine calls that the driver and the container control need.
//!
//! The trait has one method for each call of `DockerClient` that this feature
//! uses, with the same argument and result types (feature 10, section 6.7). The
//! real implementation delegates to `DockerClient`; a test gives a fake that
//! records the calls (section 7).
//!
//! The slave goes into the container as a tar stream. [`slave_tar`] reads the
//! slave file in bounded chunks, so the driver never holds the whole archive.

use std::collections::VecDeque;
use std::io::Read as _;
use std::path::Path;

use foundation_core::extensions::result_ext::BoxedError;
use foundation_core::io::readers::Data;
use foundation_core::valtron::BoxedSendableDataIterator;
use foundation_deployment_docker::client::body::{LogStream, ProgressStream};
use foundation_deployment_docker::client::containers_lifecycle::ContainerLogsOptions;
use foundation_deployment_docker::client::{
    ContainerCreateBody, ContainerHostConfig, DockerClient,
};
use foundation_deployment_docker::error::DockerError;

use foundation_deployment_docker::generated::create::{ContainerCreateResponse, CreateImageInfo};
use foundation_deployment_docker::generated::info::SystemInfo;
use foundation_deployment_docker::generated::json::{
    ContainerConfig, ContainerInspectResponse, ContainerSummary, ExecInspectResponse, HostConfig,
    ImageInspect, Resources,
};
use foundation_deployment_docker::generated::shared::IDResponse;
use foundation_deployment_docker::generated::wait::ContainerWaitResponse;
use foundation_netio::shared::http::SendSafeBody;

use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::BoxFuture;

/// The label that names the sandbox of a container.
pub const LABEL_SANDBOX_ID: &str = "ewe.sandbox.id";

/// The label that names the driver of a container.
pub const LABEL_SANDBOX_DRIVER: &str = "ewe.sandbox.driver";

/// The label that marks a container of the container control.
pub const LABEL_CONTAINER_CONTROL: &str = "ewe.container.control";

/// The Docker Engine calls of the driver and the container control.
///
/// `Send + Sync + 'static`: the driver, the platform and the container control
/// hold the trait object in an `Arc` (section 6.7).
pub trait DockerApi: Send + Sync + 'static {
    /// `POST /containers/create`.
    fn create_container<'a>(
        &'a self,
        config: &'a ContainerCreateBody,
        name: Option<&'a str>,
    ) -> BoxFuture<'a, Result<ContainerCreateResponse, DockerError>>;

    /// `POST /containers/{id}/start`.
    fn start_container<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>>;

    /// `POST /containers/{id}/wait`.
    fn wait_container<'a>(
        &'a self,
        id: &'a str,
    ) -> BoxFuture<'a, Result<ContainerWaitResponse, DockerError>>;

    /// `POST /containers/{id}/stop`.
    fn stop_container<'a>(
        &'a self,
        id: &'a str,
        timeout_secs: Option<u32>,
    ) -> BoxFuture<'a, Result<(), DockerError>>;

    /// `DELETE /containers/{id}`.
    fn remove_container<'a>(
        &'a self,
        id: &'a str,
        force: bool,
    ) -> BoxFuture<'a, Result<(), DockerError>>;

    /// `GET /containers/{id}/json`.
    fn inspect_container<'a>(
        &'a self,
        id: &'a str,
    ) -> BoxFuture<'a, Result<ContainerInspectResponse, DockerError>>;

    /// `PUT /containers/{id}/archive`.
    fn upload_to_container<'a>(
        &'a self,
        id: &'a str,
        path: &'a str,
        tar: SendSafeBody,
    ) -> BoxFuture<'a, Result<(), DockerError>>;

    /// `GET /containers/{id}/logs`.
    fn container_logs<'a>(
        &'a self,
        id: &'a str,
        options: &'a ContainerLogsOptions,
    ) -> BoxFuture<'a, Result<LogStream, DockerError>>;

    /// `POST /containers/{id}/kill`.
    fn kill_container<'a>(
        &'a self,
        id: &'a str,
        signal: Option<&'a str>,
    ) -> BoxFuture<'a, Result<(), DockerError>>;

    /// `GET /containers/json`.
    fn container_list<'a>(
        &'a self,
        all: bool,
        filters: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Vec<ContainerSummary>, DockerError>>;

    /// `POST /containers/{id}/exec`.
    fn exec_create<'a>(
        &'a self,
        id: &'a str,
        cmd: &'a [&'a str],
    ) -> BoxFuture<'a, Result<IDResponse, DockerError>>;

    /// `POST /exec/{id}/start`.
    fn exec_start<'a>(
        &'a self,
        id: &'a str,
        detach: bool,
    ) -> BoxFuture<'a, Result<LogStream, DockerError>>;

    /// `GET /exec/{id}/json`.
    fn exec_inspect<'a>(
        &'a self,
        id: &'a str,
    ) -> BoxFuture<'a, Result<ExecInspectResponse, DockerError>>;

    /// `GET /images/{name}/json`.
    fn image_inspect<'a>(
        &'a self,
        name: &'a str,
    ) -> BoxFuture<'a, Result<ImageInspect, DockerError>>;

    /// `POST /images/create` (a pull).
    fn image_pull<'a>(
        &'a self,
        name: &'a str,
    ) -> BoxFuture<'a, Result<ProgressStream<CreateImageInfo>, DockerError>>;

    /// `GET /info`.
    fn system_info(&self) -> BoxFuture<'_, Result<SystemInfo, DockerError>>;

    /// `GET /_ping`.
    fn system_ping(&self) -> BoxFuture<'_, Result<(), DockerError>>;
}

impl DockerApi for DockerClient {
    fn create_container<'a>(
        &'a self,
        config: &'a ContainerCreateBody,
        name: Option<&'a str>,
    ) -> BoxFuture<'a, Result<ContainerCreateResponse, DockerError>> {
        Box::pin(async move { DockerClient::create_container(self, config, name).await })
    }

    fn start_container<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>> {
        Box::pin(async move { DockerClient::start_container(self, id).await })
    }

    fn wait_container<'a>(
        &'a self,
        id: &'a str,
    ) -> BoxFuture<'a, Result<ContainerWaitResponse, DockerError>> {
        Box::pin(async move { DockerClient::wait_container(self, id, None).await })
    }

    fn stop_container<'a>(
        &'a self,
        id: &'a str,
        timeout_secs: Option<u32>,
    ) -> BoxFuture<'a, Result<(), DockerError>> {
        Box::pin(async move { DockerClient::stop_container(self, id, timeout_secs).await })
    }

    fn remove_container<'a>(
        &'a self,
        id: &'a str,
        force: bool,
    ) -> BoxFuture<'a, Result<(), DockerError>> {
        Box::pin(async move { DockerClient::remove_container(self, id, force).await })
    }

    fn inspect_container<'a>(
        &'a self,
        id: &'a str,
    ) -> BoxFuture<'a, Result<ContainerInspectResponse, DockerError>> {
        Box::pin(async move { DockerClient::inspect_container(self, id).await })
    }

    fn upload_to_container<'a>(
        &'a self,
        id: &'a str,
        path: &'a str,
        tar: SendSafeBody,
    ) -> BoxFuture<'a, Result<(), DockerError>> {
        Box::pin(async move {
            DockerClient::upload_to_container(self, id, path, tar, None, Some(true)).await
        })
    }

    fn container_logs<'a>(
        &'a self,
        id: &'a str,
        options: &'a ContainerLogsOptions,
    ) -> BoxFuture<'a, Result<LogStream, DockerError>> {
        Box::pin(async move { DockerClient::container_logs(self, id, options).await })
    }

    fn kill_container<'a>(
        &'a self,
        id: &'a str,
        signal: Option<&'a str>,
    ) -> BoxFuture<'a, Result<(), DockerError>> {
        Box::pin(async move { DockerClient::kill_container(self, id, signal).await })
    }

    fn container_list<'a>(
        &'a self,
        all: bool,
        filters: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Vec<ContainerSummary>, DockerError>> {
        Box::pin(async move { DockerClient::container_list(self, all, None, false, filters).await })
    }

    fn exec_create<'a>(
        &'a self,
        id: &'a str,
        cmd: &'a [&'a str],
    ) -> BoxFuture<'a, Result<IDResponse, DockerError>> {
        Box::pin(async move { DockerClient::exec_create(self, id, cmd).await })
    }

    fn exec_start<'a>(
        &'a self,
        id: &'a str,
        detach: bool,
    ) -> BoxFuture<'a, Result<LogStream, DockerError>> {
        Box::pin(async move { DockerClient::exec_start(self, id, detach).await })
    }

    fn exec_inspect<'a>(
        &'a self,
        id: &'a str,
    ) -> BoxFuture<'a, Result<ExecInspectResponse, DockerError>> {
        Box::pin(async move { DockerClient::exec_inspect(self, id).await })
    }

    fn image_inspect<'a>(
        &'a self,
        name: &'a str,
    ) -> BoxFuture<'a, Result<ImageInspect, DockerError>> {
        Box::pin(async move { DockerClient::image_inspect(self, name).await })
    }

    fn image_pull<'a>(
        &'a self,
        name: &'a str,
    ) -> BoxFuture<'a, Result<ProgressStream<CreateImageInfo>, DockerError>> {
        Box::pin(async move {
            DockerClient::image_pull(self, Some(name), None, None, None, None, None).await
        })
    }

    fn system_info(&self) -> BoxFuture<'_, Result<SystemInfo, DockerError>> {
        Box::pin(async move { DockerClient::system_info(self).await })
    }

    fn system_ping(&self) -> BoxFuture<'_, Result<(), DockerError>> {
        Box::pin(async move { DockerClient::system_ping(self).await })
    }
}

/// Maps a Docker client error to the typed error of this crate.
///
/// A daemon status becomes [`SandboxError::SubstrateApi`] with the daemon's
/// message. A failed pull has no status of its own; a missing image gives
/// status 404 (section 5). A transport or parse error is an input or output
/// error.
#[must_use]
pub fn map_error(error: DockerError) -> SandboxError {
    match error {
        DockerError::Api { status, message } => SandboxError::SubstrateApi { status, message },
        DockerError::PullFailed(message) => SandboxError::SubstrateApi {
            status: 404,
            message,
        },
        other => SandboxError::Io {
            op: "docker".to_string(),
            message: other.to_string(),
        },
    }
}

/// Whether this error is the daemon's "not found" answer.
#[must_use]
pub fn is_not_found(error: &DockerError) -> bool {
    matches!(error, DockerError::Api { status: 404, .. })
}

/// Whether this error means that no daemon answered.
#[must_use]
pub fn is_unreachable(error: &DockerError) -> bool {
    error.is_unreachable()
}

/// The create body of one sandbox container (feature 10, section 6.4).
///
/// The caller fills the generated fields; this function only merges the
/// generated `HostConfig` and `Resources` into the one object that the daemon
/// reads.
#[must_use]
pub fn create_body(
    config: ContainerConfig,
    host_config: HostConfig,
    resources: Resources,
) -> ContainerCreateBody {
    ContainerCreateBody {
        config,
        host_config: Some(ContainerHostConfig {
            base: host_config,
            resources,
        }),
        networking_config: None,
    }
}

/// The tar stream of the slave file: the directory `.ewe/` and the file
/// `.ewe/agent-slave`.
///
/// The file contents are read in bounded chunks as the daemon reads the body,
/// so the driver never holds the whole slave in memory.
///
/// # Errors
/// [`SandboxError::BinaryNotFound`] when the slave file does not exist, or
/// [`SandboxError::Io`] when its metadata cannot be read.
pub fn slave_tar(path: &Path) -> SandboxResult<SendSafeBody> {
    let file = std::fs::File::open(path).map_err(|error| SandboxError::Io {
        op: "open slave".to_string(),
        message: format!("{}: {error}", path.display()),
    })?;
    let size = file
        .metadata()
        .map_err(|error| SandboxError::Io {
            op: "stat slave".to_string(),
            message: format!("{}: {error}", path.display()),
        })?
        .len();
    let directory = header(".ewe/", 0, 0o755, tar::EntryType::Directory);
    let slave = header(".ewe/agent-slave", size, 0o755, tar::EntryType::Regular);
    Ok(SendSafeBody::Stream(Some(Box::new(TarStream {
        blocks: VecDeque::from([directory, slave]),
        file: Some(file),
        remaining: size,
        padding: padding_for(size),
        tail: 0,
    }))))
}

/// The tar block count that pads `size` bytes to a 512-byte boundary.
fn padding_for(size: u64) -> usize {
    let remainder = size % 512;
    if remainder == 0 {
        0
    } else {
        usize::try_from(512 - remainder).unwrap_or(0)
    }
}

/// One 512-byte tar header.
fn header(name: &str, size: u64, mode: u32, entry_type: tar::EntryType) -> Vec<u8> {
    let mut header = tar::Header::new_gnu();
    header.set_size(size);
    header.set_mode(mode);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_entry_type(entry_type);
    // The two names are short: GNU and ustar both fit them in the 100-byte
    // name field, so this never needs a long-name extension.
    header
        .set_path(name)
        .expect("a short tar entry name fits the header");
    header.set_cksum();
    header.as_bytes().to_vec()
}

/// A tar stream that reads the slave file one chunk at a time.
struct TarStream {
    /// The headers that come before the file contents.
    blocks: VecDeque<Vec<u8>>,
    /// The slave file, until its last byte is read.
    file: Option<std::fs::File>,
    /// The bytes of the file not read yet.
    remaining: u64,
    /// The zero bytes that pad the file to a 512-byte boundary.
    padding: usize,
    /// How many of the two end blocks were already given.
    tail: u8,
}

impl Iterator for TarStream {
    type Item = Result<Data, BoxedError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(block) = self.blocks.pop_front() {
            return Some(Ok(Data::Bytes(block)));
        }
        if self.remaining > 0 {
            let Some(file) = self.file.as_mut() else {
                return Some(Ok(Data::Bytes(Vec::new())));
            };
            let want = usize::try_from(self.remaining.min(64 * 1024)).unwrap_or(64 * 1024);
            let mut buffer = vec![0_u8; want];
            return match file.read(&mut buffer) {
                Ok(0) => Some(Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "the slave file ended before its size",
                )))),
                Ok(read) => {
                    buffer.truncate(read);
                    self.remaining -= u64::try_from(read).unwrap_or(0);
                    if self.remaining == 0 {
                        self.file = None;
                    }
                    Some(Ok(Data::Bytes(buffer)))
                }
                Err(error) => Some(Err(Box::new(error))),
            };
        }
        if self.padding > 0 {
            let count = self.padding;
            self.padding = 0;
            return Some(Ok(Data::Bytes(vec![0_u8; count])));
        }
        if self.tail < 2 {
            self.tail += 1;
            return Some(Ok(Data::Bytes(vec![0_u8; 512])));
        }
        None
    }
}

/// The `SendSafeBody` type is not `Debug`, so this wrapper names it in logs.
impl std::fmt::Debug for TarStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TarStream")
            .field("remaining", &self.remaining)
            .finish_non_exhaustive()
    }
}

/// The boxed iterator type that `SendSafeBody::Stream` holds. It exists so the
/// compiler checks the exact type in one place.
#[allow(dead_code, reason = "the alias documents the body type of the trait")]
type TarIterator = BoxedSendableDataIterator<BoxedError>;
