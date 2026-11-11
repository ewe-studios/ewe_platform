//! Hand-written `DockerClient` wrappers for the System endpoints.
//!
//! WHY: `system_events` is a feed, returned unread; the other endpoints
//! (`system_info`, `system_ping`, `system_data_usage`, `system_auth`) are
//! ergonomic wrappers over their generated `*_request` functions.
//!
//! WHAT: `system_info`, `system_ping`, `system_events`, `system_data_usage`,
//! `system_auth`. (`system_version` lives in `client.rs`.)
//!
//! HOW: Functions take `&DockerClient` + params, construct the args struct,
//! call the generated async fn, and map errors to `DockerError`.

use foundation_netio::PreparedRequestBuilder;

use crate::client::body::EventStream;
use crate::error::DockerError;
use crate::generated::auth::{system_auth_request, AuthConfig, AuthResponse, SystemAuthArgs};
use crate::generated::df::{
    system_data_usage_request, SystemDataUsageArgs, SystemDataUsageResponse,
};
use crate::generated::events::{system_events_request, SystemEventsArgs};
use crate::generated::info::{system_info_request, SystemInfo, SystemInfoArgs};
use crate::generated::ping::{system_ping_request, SystemPingArgs};

/// A closure type for the (empty) `builder_mod` argument — no request mutation.
type NoMod = fn(&mut PreparedRequestBuilder);

/// Get system information (`GET /info`).
///
/// WHY: Returns daemon metadata: container counts, OS, kernel version, etc.
///
/// WHAT: Calls the generated `system_info_request` and returns the generated
/// [`SystemInfo`].
///
/// HOW: `SystemInfoArgs::default()` — no query params needed.
///
/// # Errors
///
/// Returns [`DockerError`] on transport failure or non-2xx status.
pub async fn system_info(client: &super::DockerClient) -> Result<SystemInfo, DockerError> {
    let response = system_info_request(
        client.api(),
        &SystemInfoArgs::default(),
        &client.api_url().await?,
        None::<NoMod>,
    )
    .await?;
    Ok(response.body)
}

/// Ping the daemon (`GET /_ping`).
///
/// WHY: The daemon returns the text `"OK"` with `Content-Type: text/plain`.
///
/// WHAT: Calls the generated `system_ping_request` and drains the `"OK"` body.
///
/// HOW: `SystemPingArgs::default()` — no query params needed.
///
/// # Errors
///
/// Returns [`DockerError`] on transport failure or non-2xx status.
pub async fn system_ping(client: &super::DockerClient) -> Result<(), DockerError> {
    let response = system_ping_request(
        client.api(),
        &SystemPingArgs::default(),
        &client.api_url().await?,
        None::<NoMod>,
    )
    .await?;
    let _ = response.body.drain().await;
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
/// WHY: Returns how much disk the daemon's images, containers, volumes and
/// build cache use.
///
/// WHAT: Calls the generated `system_data_usage_request` and returns the
/// generated [`SystemDataUsageResponse`]. `types` is the optional single
/// `type` filter (image, container, volume, build-cache).
///
/// HOW: `SystemDataUsageArgs` with the optional filter.
///
/// # Errors
///
/// Returns [`DockerError`] on transport failure, non-2xx status, or a body
/// that is not a `SystemDataUsageResponse`.
pub async fn system_data_usage(
    client: &super::DockerClient,
    types: Option<&str>,
) -> Result<SystemDataUsageResponse, DockerError> {
    let args = SystemDataUsageArgs {
        r#type: types.map_or_default(|t| vec![t.to_string()]),
        verbose: None,
    };
    let response =
        system_data_usage_request(client.api(), &args, &client.api_url().await?, None::<NoMod>)
            .await?;
    Ok(response.body)
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
    let response =
        system_auth_request(client.api(), &args, &client.api_url().await?, None::<NoMod>).await?;
    Ok(response.body)
}
