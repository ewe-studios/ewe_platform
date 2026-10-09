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
//! HOW: Every function takes `base_url: &str` (e.g. `"http://localhost/v1.53"`),
//! matching the generated `*_request` functions. Pass
//! [`DockerClient::base_url()`](crate::client::DockerClient::base_url) for the
//! configured host + version prefix.

pub mod decoder;

use foundation_netio::shared::client::body_reader::{
    split_exchange, ExchangeBodyObserver, ExchangeContinuation, ExchangeHeadObserver,
};
use foundation_netio::{DynNetClient, PreparedRequestBuilder};

/// Streaming request for container logs (`GET /containers/{id}/logs`).
///
/// # Returns
///
/// A `(head, body, continuation)` tuple from [`split_exchange`]. The body
/// observer yields raw `Bytes` chunks — feed them into [`LogFrameDecoder`] to
/// extract typed `LogOutput` frames.
#[must_use]
pub fn container_logs(
    client: DynNetClient,
    base_url: &str,
    container_id: &str,
    follow: bool,
    stdout: bool,
    stderr: bool,
    since: Option<u64>,
    until: Option<u64>,
    timestamps: bool,
    tail: Option<&str>,
) -> (ExchangeHeadObserver, ExchangeBodyObserver, ExchangeContinuation) {
    let url = format!("{base_url}/containers/{container_id}/logs");

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

/// Streaming request for container stats (`GET /containers/{id}/stats`).
///
/// Each body chunk is a JSON object (one per line when `stream=true`). Use
/// [`JsonLineDecoder`] to extract complete JSON objects from the stream.
#[must_use]
pub fn container_stats(
    client: DynNetClient,
    base_url: &str,
    container_id: &str,
    stream: bool,
    one_shot: bool,
) -> (ExchangeHeadObserver, ExchangeBodyObserver, ExchangeContinuation) {
    let url = format!("{base_url}/containers/{container_id}/stats");

    let builder = PreparedRequestBuilder::get(&url)
        .expect("valid stats URL")
        .query("stream", Some(if stream { "1" } else { "0" }))
        .query("one-shot", Some(if one_shot { "1" } else { "0" }));

    split_exchange(client, builder)
}

/// Streaming request for container attach (`POST /containers/{id}/attach`).
///
/// Upgrades to a raw multiplexed stream (same 8-byte frame format as logs).
/// Use [`LogFrameDecoder`] to decode stdin/stdout/stderr frames.
#[must_use]
pub fn container_attach(
    client: DynNetClient,
    base_url: &str,
    container_id: &str,
    detach_keys: Option<&str>,
    logs: bool,
    stream: bool,
    stdin: bool,
    stdout: bool,
    stderr: bool,
) -> (ExchangeHeadObserver, ExchangeBodyObserver, ExchangeContinuation) {
    let url = format!("{base_url}/containers/{container_id}/attach");

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

/// Streaming request for system events (`GET /events`).
///
/// Each body chunk is a JSON event object. Use [`JsonLineDecoder`] to extract
/// complete [`DockerEvent`] objects from the stream.
#[must_use]
pub fn system_events(
    client: DynNetClient,
    base_url: &str,
    since: Option<&str>,
    until: Option<&str>,
    filters: Option<&str>,
) -> (ExchangeHeadObserver, ExchangeBodyObserver, ExchangeContinuation) {
    let url = format!("{base_url}/events");

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

/// Streaming request for image build (`POST /build`).
///
/// Each body chunk is a JSON status object. Use [`JsonLineDecoder`] to extract
/// progress objects from the stream.
#[must_use]
pub fn image_build(
    client: DynNetClient,
    base_url: &str,
    dockerfile: Option<&str>,
    tags: Option<&[&str]>,
    nocache: bool,
    pull: bool,
    remote: Option<&str>,
    platform: Option<&str>,
) -> (ExchangeHeadObserver, ExchangeBodyObserver, ExchangeContinuation) {
    let url = format!("{base_url}/build");

    let mut builder = PreparedRequestBuilder::post(&url).expect("valid build URL");

    if let Some(df) = dockerfile {
        builder = builder.query("dockerfile", Some(df));
    }
    if let Some(t) = tags {
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

/// Streaming request for image pull (`POST /images/create`).
///
/// Each body chunk is a JSON progress object. Use [`JsonLineDecoder`] to extract
/// progress messages from the stream.
#[must_use]
pub fn image_pull(
    client: DynNetClient,
    base_url: &str,
    image: &str,
    tag: Option<&str>,
    platform: Option<&str>,
) -> (ExchangeHeadObserver, ExchangeBodyObserver, ExchangeContinuation) {
    let url = format!("{base_url}/images/create");

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

/// Streaming request for image push (`POST /images/{name}/push`).
///
/// Each body chunk is a JSON progress object. Use [`JsonLineDecoder`] to extract
/// progress messages from the stream.
#[must_use]
pub fn image_push(
    client: DynNetClient,
    base_url: &str,
    name: &str,
    tag: Option<&str>,
    _x_registry_auth: Option<&str>,
) -> (ExchangeHeadObserver, ExchangeBodyObserver, ExchangeContinuation) {
    let url = format!("{base_url}/images/{name}/push");

    let mut builder = PreparedRequestBuilder::post(&url).expect("valid push URL");

    if let Some(t) = tag {
        builder = builder.query("tag", Some(t));
    }

    split_exchange(client, builder)
}
