// Modules for containing static.

use axum::response::IntoResponse;
use http::StatusCode;
use std::{net::SocketAddr, pin, sync, time::Duration};
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use foundation_nostd::embeddable::DirectoryData;
use foundation_runtimes::js_runtimes::AssetReloader;

use axum::{
    body,
    response::sse::{Event, KeepAlive, Sse},
};

use crate::FileChange;

/// `RELOADER_SCRIPT_ENDPOINT` is the relevant script path to be used in our html
/// to define where the reloading script can be found.
pub static RELOADER_SCRIPT_ENDPOINT: &str = "/static/sse/reloader.js";

/// `RELOADER_SSE_ENDPOINT` is the relevant endpoint we should use when
/// setting up the http route to be used to connect to our SSE endpoint.
pub static RELOADER_SSE_ENDPOINT: &str = "/static/sse/reload";

pub fn sse_endpoint_script(
    _addr: SocketAddr,
    _request: crate::types::HyperRequest,
) -> pin::Pin<Box<crate::types::HyperFuture>> {
    Box::pin(async move {
        let instance = AssetReloader::default();
        match instance.read_utf8_for("reloader.js") {
            Some(data) => {
                let body = body::Body::new(crate::full(bytes::Bytes::from(data)));
                Ok(hyper::Response::builder()
                    .header("Content-Type", "text/javascript")
                    .status(StatusCode::OK)
                    .body(body)
                    .unwrap())
            }
            None => {
                let body = body::Body::new(crate::empty());
                Ok(hyper::Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(body)
                    .unwrap())
            }
        }
    })
}

fn sse_endpoint_reloader(
    _addr: SocketAddr,
    _request: crate::types::HyperRequest,
    running_notification: broadcast::Receiver<FileChange>,
) -> pin::Pin<Box<crate::types::HyperFuture>> {
    Box::pin(async move {
        let running_stream = BroadcastStream::new(running_notification);
        Ok(Sse::new(
            // when declaring Result types for such cases, the error type must be explicit
            // else you will have type inference compiler errors
            running_stream.map(|_| -> Result<Event, crate::types::BoxedError> {
                Ok(Event::default()
                    .data("ready")
                    .comment("indicates we should reload page")
                    .event("reload"))
            }),
        )
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(1))
                .text("keep-alive"),
        )
        .into_response())
    })
}

/// `create_sse_endpoint_handler` takes a `broadcast::Sender<()>`
/// which might suprise you till you figure out that the following rules
/// are involved:
/// 1. You are defining a Fn(addr, request) which can be called multiple times.
/// 2. Tokio's `broadcast::Receiver<T>` does not implement clone which means after
///    the first call it is moved out and in essence is owned by the
///    inner function and therefore can not be reused on the next one.
/// 3. Tokio's `broadcast::Sender<T>` implements Clone and we can create a new receiver
///    on each re-call.
pub fn create_sse_endpoint_handler(
    reload_notification: broadcast::Sender<FileChange>,
) -> sync::Arc<crate::types::HyperFunc> {
    sync::Arc::new(move |addr, request| {
        sse_endpoint_reloader(addr, request, reload_notification.subscribe())
    })
}
