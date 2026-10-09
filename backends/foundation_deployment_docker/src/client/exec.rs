//! Hand-written `DockerClient` wrappers for the Exec endpoints.
//!
//! WHY: The generated functions for exec endpoints either discard the response
//! body (exec_start, exec_inspect) or need JSON bodies injected through
//! `builder_mod` (exec_create). This module provides ergonomic wrappers that
//! handle these cases.
//!
//! WHAT: `exec_create`, `exec_start`, `exec_inspect`, `exec_resize`.
//!
//! HOW: Functions take `&DockerClient` + params, construct the args struct,
//! call the generated async fn (or build a manual request where the generated
//! fn discards the body), and map errors to `DockerError`.

use foundation_netio::shared::http::SimpleHeader;
use foundation_netio::PreparedRequestBuilder;

use crate::error::DockerError;
use crate::generated::exec::{container_exec_request, ContainerExecArgs};
use crate::generated::resize::{exec_resize_request, ExecResizeArgs};
use crate::generated::shared::IDResponse;

/// A closure type for the (empty) `builder_mod` argument — no request mutation.
type NoMod = fn(&mut PreparedRequestBuilder);

/// Create an exec instance in a running container (`POST /containers/{id}/exec`).
///
/// WHY: Sets up a command to run inside the container and returns an exec id
/// that can be started with [`exec_start`].
///
/// WHAT: Posts `{"Cmd": cmd, "AttachStdout": true, "AttachStderr": true}` to
/// the daemon.
///
/// HOW: Uses the generated `container_exec_request` with a `builder_mod` that
/// sets the JSON body.
///
/// # Errors
///
/// Returns [`DockerError`] on transport failure or non-2xx status.
pub async fn exec_create(
    client: &super::DockerClient,
    id: &str,
    cmd: &[&str],
) -> Result<IDResponse, DockerError> {
    let body = serde_json::json!({
        "Cmd": cmd,
        "AttachStdout": true,
        "AttachStderr": true,
    });
    let args = ContainerExecArgs {
        id: id.to_string(),
    };
    let response = container_exec_request(
        client.http(),
        &args,
        &client.base_url(),
        Some(move |b: &mut PreparedRequestBuilder| {
            b.set_header(SimpleHeader::CONTENT_TYPE, "application/json");
            let _ = b.set_body_json(&body);
        }),
    )
    .await?;
    Ok(response.body)
}

/// Start a previously-created exec instance (`POST /exec/{id}/start`).
///
/// WHY: Runs the command configured by [`exec_create`]; the response is a
/// multiplexed stream (stdout/stderr). The generated function discards the
/// response body, so we build the request manually to capture raw bytes.
///
/// WHAT: Posts `{"Detach": detach}` and returns the raw response body.
///
/// HOW: Manual `PreparedRequestBuilder::post` + `send_async`.
///
/// # Errors
///
/// Returns [`DockerError`] on transport failure or non-2xx status.
pub async fn exec_start(
    client: &super::DockerClient,
    id: &str,
    detach: bool,
) -> Result<Vec<u8>, DockerError> {
    let body = serde_json::json!({ "Detach": detach });
    let endpoint_url = format!("{}/exec/{}/start", client.base_url(), id);
    let mut builder = PreparedRequestBuilder::post(&endpoint_url)
        .map_err(|e| DockerError::Transport(e.to_string()))?;
    builder.set_header(SimpleHeader::CONTENT_TYPE, "application/json");
    let _ = builder.set_body_json(&body);
    let response = client
        .http()
        .send_async(builder.build())
        .await
        .map_err(|e| DockerError::Transport(e.to_string()))?;
    let status: usize = response.get_status().into();
    if status < 200 || status >= 300 {
        return Err(DockerError::Api {
            status: status as u16,
            message: format!("exec_start failed with status {status}"),
        });
    }
    let body_bytes =
        foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe(
            response.take_body(),
        );
    Ok(body_bytes)
}

/// Inspect an exec instance (`GET /exec/{id}/json`).
///
/// WHY: Returns metadata about an exec instance (running, exit code, etc.).
/// The generated `exec_inspect_request` returns `ApiResponse<()>` and discards
/// the response body, so we build the request manually.
///
/// WHAT: Returns the parsed JSON body as a [`serde_json::Value`].
///
/// HOW: Manual `PreparedRequestBuilder::get` + `send_async` + JSON parse.
///
/// # Errors
///
/// Returns [`DockerError`] on transport failure, non-2xx status, or JSON parse
/// failure.
pub async fn exec_inspect(
    client: &super::DockerClient,
    id: &str,
) -> Result<serde_json::Value, DockerError> {
    let endpoint_url = format!("{}/exec/{}/json", client.base_url(), id);
    let builder = PreparedRequestBuilder::get(&endpoint_url)
        .map_err(|e| DockerError::Transport(e.to_string()))?;
    let response = client
        .http()
        .send_async(builder.build())
        .await
        .map_err(|e| DockerError::Transport(e.to_string()))?;
    let status: usize = response.get_status().into();
    if status < 200 || status >= 300 {
        return Err(DockerError::Api {
            status: status as u16,
            message: format!("exec_inspect failed with status {status}"),
        });
    }
    let body_bytes =
        foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe(
            response.take_body(),
        );
    let parsed: serde_json::Value = serde_json::from_slice(&body_bytes)
        .map_err(|e| DockerError::JsonParse(format!("exec_inspect: {e}")))?;
    Ok(parsed)
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
    exec_resize_request(client.http(), &args, &client.base_url(), None::<NoMod>).await?;
    Ok(())
}
