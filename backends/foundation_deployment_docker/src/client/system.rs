//! Hand-written `DockerClient` wrappers for the System endpoints.
//!
//! WHY: The generated functions for system endpoints either discard the
//! response body (system_data_usage), fail on non-JSON responses
//! (system_ping); `system_events` is a feed, returned unread. This module
//! provides ergonomic wrappers that handle these cases.
//!
//! WHAT: `system_info`, `system_ping`, `system_events`, `system_data_usage`,
//! `system_auth`. (`system_version` lives in `client.rs`.)
//!
//! HOW: Functions take `&DockerClient` + params, construct the args struct,
//! call the generated async fn (or build a manual request where needed), and
//! map errors to `DockerError`.

use foundation_netio::PreparedRequestBuilder;

use crate::client::body::EventStream;
use crate::error::DockerError;
use crate::generated::events::{system_events_request, SystemEventsArgs};
use crate::generated::auth::{
    system_auth_request, AuthConfig, AuthResponse, SystemAuthArgs,
};

/// A closure type for the (empty) `builder_mod` argument — no request mutation.
type NoMod = fn(&mut PreparedRequestBuilder);

/// Get system information (`GET /info`).
///
/// WHY: Returns daemon metadata: container counts, OS, kernel version, etc.
///
/// WHAT: Calls the generated `system_info_request`.
///
/// HOW: `SystemInfoArgs::default()` — no query params needed.
///
/// # Errors
///
/// Returns [`DockerError`] on transport failure or non-2xx status.
/// Get system info (`GET /info`).
///
/// WHY: The generated `SystemInfo` struct fails to deserialize Docker's
/// `LocalNodeState` field (a raw string like `"inactive"`, not a struct).
/// We return `serde_json::Value` so the caller can inspect what's available.
///
/// WHAT: Manual `PreparedRequestBuilder::get` + `send_async` returning the
/// raw JSON body.
///
/// HOW: Avoids the generated `system_info_request` which uses the broken type.
///
/// # Errors
///
/// Returns [`DockerError`] on transport failure or non-2xx status.
pub async fn system_info(client: &super::DockerClient) -> Result<serde_json::Value, DockerError> {
    use crate::error::DockerError;
    use foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe;

    let base = client.api_url().await?;
    let endpoint_url = format!("{base}/info");
    let builder = foundation_netio::PreparedRequestBuilder::get(&endpoint_url)
        .map_err(|e| crate::generated::shared::ApiError::RequestBuildFailed(e.to_string()))?;

    let response = client.http().send_async(builder.build()).await
        .map_err(|e| crate::generated::shared::ApiError::RequestSendFailed(e.to_string()))?;

    let status: usize = response.get_status().into();
    if status < 200 || status >= 300 {
        return Err(DockerError::Api { status: status as u16, message: "(no body)".into() });
    }
    let body_bytes = collect_bytes_from_send_safe(response.take_body());
    serde_json::from_slice(&body_bytes).map_err(|e| DockerError::JsonParse(e.to_string()))
}

/// Ping the daemon (`GET /_ping`).
///
/// WHY: The daemon returns the text `"OK"` with `Content-Type: text/plain`.
/// The generated function tries to parse the body as `serde_json::Value`,
/// which fails on plain text. This wrapper builds the request manually.
///
/// WHAT: Sends a simple `GET /_ping`, verifies 2xx status, discards body.
///
/// HOW: Manual `PreparedRequestBuilder::get` + `send_async`.
///
/// # Errors
///
/// Returns [`DockerError`] on transport failure or non-2xx status.
pub async fn system_ping(client: &super::DockerClient) -> Result<(), DockerError> {
    let base = client.api_url().await?;
    let endpoint_url = format!("{base}/_ping");
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
            message: format!("system_ping failed with status {status}"),
        });
    }
    Ok(())
}

/// Stream daemon events (`GET /events`).
///
/// WHY: `/events` is a feed, not a document: without `until` the daemon keeps
/// it open indefinitely, writing one JSON event per line.
///
/// WHAT: Returns the feed unread, as an [`EventStream`] of the generated
/// [`EventMessage`](crate::generated::events::EventMessage).
///
/// # Errors
///
/// Returns [`DockerError`] on transport failure or non-2xx status (checked
/// before the body is read); a malformed event or read error comes out of the
/// stream.
pub async fn system_events(
    client: &super::DockerClient,
    since: Option<&str>,
    until: Option<&str>,
    filters: Option<&str>,
) -> Result<EventStream, DockerError> {
    let args = SystemEventsArgs {
        since: since.map(str::to_string),
        until: until.map(str::to_string),
        filters: filters.map(str::to_string),
    };
    let response =
        system_events_request(client.api(), &args, &client.api_url().await?, None::<NoMod>).await?;
    Ok(EventStream::new(response.body))
}

/// Get data usage information (`GET /system/df`).
///
/// WHY: The generated `system_data_usage_request` returns `ApiResponse<()>`
/// and discards the JSON body. This wrapper builds the request manually to
/// return the parsed JSON.
///
/// WHAT: Sends `GET /system/df` with optional `types` filter and returns the
/// parsed JSON as a [`serde_json::Value`].
///
/// HOW: Manual `PreparedRequestBuilder::get` + `send_async` + JSON parse.
///
/// # Errors
///
/// Returns [`DockerError`] on transport failure, non-2xx status, or JSON parse
/// failure.
pub async fn system_data_usage(
    client: &super::DockerClient,
    types: Option<&str>,
) -> Result<serde_json::Value, DockerError> {
    let base = client.api_url().await?;
    let endpoint_url = format!("{base}/system/df");
    let mut builder = PreparedRequestBuilder::get(&endpoint_url)
        .map_err(|e| DockerError::Transport(e.to_string()))?;
    builder = builder.query("type", types);
    let response = client
        .http()
        .send_async(builder.build())
        .await
        .map_err(|e| DockerError::Transport(e.to_string()))?;
    let status: usize = response.get_status().into();
    if status < 200 || status >= 300 {
        return Err(DockerError::Api {
            status: status as u16,
            message: format!("system_data_usage failed with status {status}"),
        });
    }
    let body_bytes =
        foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe(
            response.take_body(),
        );
    let parsed: serde_json::Value = serde_json::from_slice(&body_bytes)
        .map_err(|e| DockerError::JsonParse(format!("system_data_usage: {e}")))?;
    Ok(parsed)
}

/// Check auth configuration (`POST /auth`).
///
/// WHY: Validates registry credentials without performing a push/pull.
///
/// WHAT: Posts `{"username": ..., "password": ...}` to `/auth` and returns
/// the `AuthResponse` (status + optional identity token).
///
/// HOW: Uses the generated `system_auth_request` with `AuthConfig` body.
///
/// # Errors
///
/// Returns [`DockerError`] on transport failure or non-2xx status.
pub async fn system_auth(
    client: &super::DockerClient,
    username: &str,
    password: &str,
) -> Result<AuthResponse, DockerError> {
    let args = SystemAuthArgs {
        body: AuthConfig {
            username: Some(username.to_string()),
            password: Some(password.to_string()),
            serveraddress: None,
        },
    };
    let response = system_auth_request(client.api(), &args, &client.api_url().await?, None::<NoMod>).await?;
    Ok(response.body)
}
