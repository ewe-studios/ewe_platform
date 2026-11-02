//! Hand-written `DockerClient` wrappers for the Exec endpoints.
//!
//! WHY: Ergonomic wrappers over the generated exec functions. `exec_start`'s
//! answer is the command's output stream, returned unread as a [`LogStream`];
//! `exec_inspect` returns the generated [`ExecInspectResponse`].
//!
//! WHAT: `exec_create`, `exec_start`, `exec_inspect`, `exec_resize`.
//!
//! HOW: Functions take `&DockerClient` + params, construct the args struct,
//! call the generated async fn, and map errors to `DockerError`.

use foundation_netio::PreparedRequestBuilder;

use crate::client::body::LogStream;
use crate::error::DockerError;
use crate::generated::exec::{container_exec_request, ContainerExecArgs, ContainerExecRequest};
use crate::generated::json::{exec_inspect_request, ExecInspectArgs, ExecInspectResponse};
use crate::generated::resize::{exec_resize_request, ExecResizeArgs};
use crate::generated::shared::IDResponse;
use crate::generated::start::{exec_start_request, ExecStartArgs, ExecStartRequest};

/// A closure type for the (empty) `builder_mod` argument — no request mutation.
type NoMod = fn(&mut PreparedRequestBuilder);

/// Create an exec instance in a running container (`POST /containers/{id}/exec`).
///
/// WHY: Sets up a command to run inside the container and returns an exec id
/// that can be started with [`exec_start`].
///
/// WHAT: Sends the generated `ContainerExecRequest` with `Cmd` and stdout and
/// stderr attached.
///
/// # Errors
///
/// Returns [`DockerError`] on transport failure or non-2xx status.
pub async fn exec_create(
    client: &super::DockerClient,
    id: &str,
    cmd: &[&str],
) -> Result<IDResponse, DockerError> {
    let args = ContainerExecArgs {
        id: id.to_string(),
        body: ContainerExecRequest {
            cmd: Some(cmd.iter().map(|c| (*c).to_string()).collect()),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..ContainerExecRequest::default()
        },
    };
    let response =
        container_exec_request(client.api(), &args, &client.api_url().await?, None::<NoMod>)
            .await?;
    Ok(response.body)
}

/// Start a previously-created exec instance (`POST /exec/{id}/start`).
///
/// WHY: Runs the command configured by [`exec_create`]. Attached, the daemon
/// streams the command's output for as long as it runs.
///
/// WHAT: Sends the generated `ExecStartRequest` (`Detach`) and returns the
/// output unread, as a [`LogStream`] of demultiplexed stdout/stderr frames. A
/// detached start has an empty body: the stream simply ends.
///
/// # Errors
///
/// Returns [`DockerError`] on transport failure or non-2xx status (checked
/// before the body is read); read errors come out of the stream.
pub async fn exec_start(
    client: &super::DockerClient,
    id: &str,
    detach: bool,
) -> Result<LogStream, DockerError> {
    let args = ExecStartArgs {
        id: id.to_string(),
        body: ExecStartRequest {
            detach: Some(detach),
            ..ExecStartRequest::default()
        },
    };
    let response =
        exec_start_request(client.api(), &args, &client.api_url().await?, None::<NoMod>).await?;
    Ok(LogStream::new(response.body, client.limits().max_log_frame))
}

/// Inspect an exec instance (`GET /exec/{id}/json`).
///
/// WHY: Returns metadata about an exec instance (running, exit code, etc.).
///
/// WHAT: Calls the generated `exec_inspect_request` and returns the generated
/// [`ExecInspectResponse`].
///
/// HOW: `ExecInspectArgs { id }`.
///
/// # Errors
///
/// Returns [`DockerError`] on transport failure, non-2xx status, or a body
/// that is not an `ExecInspectResponse`.
pub async fn exec_inspect(
    client: &super::DockerClient,
    id: &str,
) -> Result<ExecInspectResponse, DockerError> {
    let args = ExecInspectArgs { id: id.to_string() };
    let response =
        exec_inspect_request(client.api(), &args, &client.api_url().await?, None::<NoMod>).await?;
    Ok(response.body)
}

/// Resize the TTY of an exec instance (`POST /exec/{id}/resize`).
///
/// WHY: Adjusts the terminal dimensions for an interactive exec session.
///
/// WHAT: Calls `exec_resize_request` with height/width query params.
///
/// HOW: Uses the generated function.
///
/// # Errors
///
/// Returns [`DockerError`] on transport failure or non-2xx status.
pub async fn exec_resize(
    client: &super::DockerClient,
    id: &str,
    h: u32,
    w: u32,
) -> Result<(), DockerError> {
    let args = ExecResizeArgs {
        id: id.to_string(),
        h: Some(h.to_string()),
        w: Some(w.to_string()),
    };
    exec_resize_request(client.api(), &args, &client.api_url().await?, None::<NoMod>).await?;
    Ok(())
}
