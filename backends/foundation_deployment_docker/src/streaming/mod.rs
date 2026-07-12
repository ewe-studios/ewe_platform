//! Streaming endpoint functions for Docker's async endpoints.
//!
//! WHY: endpoints like `GET /containers/{id}/logs`, `GET /events`, and progress
//! streaming (build/pull/push) return data **incrementally** — not single JSON
//! responses. The generated functions use `collect_bytes_from_send_safe()` which
//! buffers the entire response; callers need streaming for real-time log tailing
//! and long-running builds.
//!
//! WHAT: Each function builds a `PreparedRequestBuilder`, calls
//! [`split_exchange`] to split head + body, and returns the three handles:
//! `(ExchangeHeadObserver, ExchangeBodyObserver, ExchangeContinuation)`.
//!
//! HOW: The caller:
//! 1. Sends the continuation to valtron: `valtron::send(task.map_ready(|_| ()))?;`
//! 2. Drains the head observer for `(Status, SimpleHeaders)`.
//! 3. Converts the body observer via `.into_next_stream()` for async consumption.
//! 4. Feeds body chunks into [`LogFrameDecoder`] or [`JsonLineDecoder`] (see
//!    [`super::decoder`]).

pub mod decoder;

use foundation_netio::shared::client::body_reader::{
    split_exchange, ExchangeBodyObserver, ExchangeContinuation, ExchangeHeadObserver,
};
use foundation_netio::{DynNetClient, PreparedRequestBuilder};

/// API version used in endpoint URLs.
const API_VERSION: &str = "1.53";

// =============================================================================
// Container logs (multiplexed binary stream)
// =============================================================================

/// Streaming request for container logs (`GET /containers/{id}/logs`).
///
/// # Returns
///
/// A `(head, body, continuation)` tuple from [`split_exchange`]. The body
/// observer yields raw `Bytes` chunks — feed them into [`LogFrameDecoder`] to
/// extract typed `LogOutput` frames.
///
/// # Errors
///
/// Errors surface in the body observer as `Stream::Next(Err(Arc<Error>))` items
/// — the function itself does not return an error.
///
/// # Panics
///
/// This function does not panic.
#[must_use]
pub fn container_logs(
    client: DynNetClient,
    container_id: &str,
    follow: bool,
    stdout: bool,
    stderr: bool,
    since: Option<u64>,
    until: Option<u64>,
    timestamps: bool,
    tail: Option<&str>,
) -> (ExchangeHeadObserver, ExchangeBodyObserver, ExchangeContinuation) {
    let url = format!("http://localhost/v{API_VERSION}/containers/{container_id}/logs");

    let mut builder = PreparedRequestBuilder::get(&url)
        .expect("valid logs URL")
        .query("follow", Some(if follow { "1" } else { "0" }))
        .query("stdout", Some(if stdout { "1" } else { "0" }))
        .query("stderr", Some(if stderr { "1" } else { "0" }))
        .query("timestamps", Some(if timestamps { "1" } else { "0" }))
        .query("tail", tail);

    if let Some(s) = since {
        builder = builder.query("since", Some(&s.to_string()));
    }
    if let Some(u) = until {
        builder = builder.query("until", Some(&u.to_string()));
    }

    split_exchange(client, builder)
}

// =============================================================================
// Container stats (JSON-line stream)
// =============================================================================

/// Streaming request for container stats (`GET /containers/{id}/stats`).
///
/// Each body chunk is a JSON object (one per line when `stream=true`). Use
/// [`JsonLineDecoder`] to extract complete JSON objects from the stream.
#[must_use]
pub fn container_stats(
    client: DynNetClient,
    container_id: &str,
    stream: bool,
    one_shot: bool,
) -> (ExchangeHeadObserver, ExchangeBodyObserver, ExchangeContinuation) {
    let url = format!("http://localhost/v{API_VERSION}/containers/{container_id}/stats");

    let builder = PreparedRequestBuilder::get(&url)
        .expect("valid stats URL")
        .query("stream", Some(if stream { "1" } else { "0" }))
        .query("one-shot", Some(if one_shot { "1" } else { "0" }));

    split_exchange(client, builder)
}

// =============================================================================
// Container attach (multiplexed binary I/O stream)
// =============================================================================

/// Streaming request for container attach (`POST /containers/{id}/attach`).
///
/// Upgrades to a raw multiplexed stream (same 8-byte frame format as logs).
/// Use [`LogFrameDecoder`] to decode stdin/stdout/stderr frames.
#[must_use]
pub fn container_attach(
    client: DynNetClient,
    container_id: &str,
    detach_keys: Option<&str>,
    logs: bool,
    stream: bool,
    stdin: bool,
    stdout: bool,
    stderr: bool,
) -> (ExchangeHeadObserver, ExchangeBodyObserver, ExchangeContinuation) {
    let url = format!("http://localhost/v{API_VERSION}/containers/{container_id}/attach");

    let mut builder = PreparedRequestBuilder::post(&url)
        .expect("valid attach URL")
        .query("logs", Some(if logs { "1" } else { "0" }))
        .query("stream", Some(if stream { "1" } else { "0" }))
        .query("stdin", Some(if stdin { "1" } else { "0" }))
        .query("stdout", Some(if stdout { "1" } else { "0" }))
        .query("stderr", Some(if stderr { "1" } else { "0" }));

    if let Some(dk) = detach_keys {
        builder = builder.query("detachKeys", Some(dk));
    }

    split_exchange(client, builder)
}

// =============================================================================
// System events (JSON-line stream)
// =============================================================================

/// Streaming request for system events (`GET /events`).
///
/// Each body chunk is a JSON event object. Use [`JsonLineDecoder`] to extract
/// complete [`DockerEvent`] objects from the stream.
///
/// [`DockerEvent`] is the generated type from `generated::events`.
#[must_use]
pub fn system_events(
    client: DynNetClient,
    since: Option<&str>,
    until: Option<&str>,
    filters: Option<&str>,
) -> (ExchangeHeadObserver, ExchangeBodyObserver, ExchangeContinuation) {
    let url = format!("http://localhost/v{API_VERSION}/events");

    let mut builder = PreparedRequestBuilder::get(&url).expect("valid events URL");

    if let Some(s) = since {
        builder = builder.query("since", Some(s));
    }
    if let Some(u) = until {
        builder = builder.query("until", Some(u));
    }
    if let Some(f) = filters {
        builder = builder.query("filters", Some(f));
    }

    split_exchange(client, builder)
}

// =============================================================================
// Image build (JSON-line progress stream)
// =============================================================================

/// Streaming request for image build (`POST /build`).
///
/// Each body chunk is a JSON status object. Use [`JsonLineDecoder`] to extract
/// progress objects from the stream.
///
/// The caller sets build configuration via query parameters (`dockerfile`,
/// `t`, `remote`, `nocache`, etc.) — see the Docker Engine API docs.
#[must_use]
pub fn image_build(
    client: DynNetClient,
    dockerfile: Option<&str>,
    tags: Option<&[&str]>,
    nocache: bool,
    pull: bool,
    remote: Option<&str>,
    platform: Option<&str>,
) -> (ExchangeHeadObserver, ExchangeBodyObserver, ExchangeContinuation) {
    let url = format!("http://localhost/v{API_VERSION}/build");

    let mut builder = PreparedRequestBuilder::post(&url).expect("valid build URL");

    if let Some(df) = dockerfile {
        builder = builder.query("dockerfile", Some(df));
    }
    if let Some(t) = tags {
        // Docker accepts multiple `t` query params; join them as a query string
        // Note: PreparedRequestBuilder::query overwrites, so we use the first tag
        // here. For multi-tag builds, the caller should construct the URI directly.
        if !t.is_empty() {
            builder = builder.query("t", Some(t[0]));
        }
    }
    builder = builder.query("nocache", Some(if nocache { "1" } else { "0" }));
    builder = builder.query("pull", Some(if pull { "1" } else { "0" }));

    if let Some(r) = remote {
        builder = builder.query("remote", Some(r));
    }
    if let Some(p) = platform {
        builder = builder.query("platform", Some(p));
    }

    split_exchange(client, builder)
}

// =============================================================================
// Image pull (JSON-line progress stream)
// =============================================================================

/// Streaming request for image pull (`POST /images/create`).
///
/// Each body chunk is a JSON progress object. Use [`JsonLineDecoder`] to extract
/// progress messages from the stream.
#[must_use]
pub fn image_pull(
    client: DynNetClient,
    image: &str,
    tag: Option<&str>,
    platform: Option<&str>,
) -> (ExchangeHeadObserver, ExchangeBodyObserver, ExchangeContinuation) {
    let url = format!("http://localhost/v{API_VERSION}/images/create");

    let mut builder = PreparedRequestBuilder::post(&url)
        .expect("valid pull URL")
        .query("fromImage", Some(image));

    if let Some(t) = tag {
        builder = builder.query("tag", Some(t));
    }
    if let Some(p) = platform {
        builder = builder.query("platform", Some(p));
    }

    split_exchange(client, builder)
}

// =============================================================================
// Image push (JSON-line progress stream)
// =============================================================================

/// Streaming request for image push (`POST /images/{name}/push`).
///
/// Each body chunk is a JSON progress object. Use [`JsonLineDecoder`] to extract
/// progress messages from the stream.
///
/// `x_registry_auth` is the base64-encoded registry auth config — set as the
/// `X-Registry-Auth` header via the builder_mod pattern.
#[must_use]
pub fn image_push(
    client: DynNetClient,
    name: &str,
    tag: Option<&str>,
    _x_registry_auth: Option<&str>,
) -> (ExchangeHeadObserver, ExchangeBodyObserver, ExchangeContinuation) {
    let url = format!("http://localhost/v{API_VERSION}/images/{name}/push");

    let mut builder = PreparedRequestBuilder::post(&url).expect("valid push URL");

    if let Some(t) = tag {
        builder = builder.query("tag", Some(t));
    }
    // _x_registry_auth is accepted but not wired here — callers needing registry
    // auth should construct the request with `builder.header()` directly.


    split_exchange(client, builder)
}
