// Modules for containing static.

use axum::response::IntoResponse;
use http::StatusCode;
use std::{net::SocketAddr, pin, sync, time::Duration};
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use axum::{
    body,
    response::sse::{Event, KeepAlive, Sse},
};

/// The static embedded reloading script for SSE dev server that the
/// RELOADER_SCRIPT_ENDPOINT should load up when the endpoint gets hit
/// on whatever html page is relevant.
pub static RELOADER_SCRIPT_BYTES: &'static [u8] = include_bytes!("./reloader.js");

/// RELOADER_SCRIPT_ENDPOINT is the relevant script path to be used in our html
/// to define where the reloading script can be found.
pub static RELOADER_SCRIPT_ENDPOINT: &'static str = "/static/sse/reloader.js";

/// RELOADER_SSE_ENDPOINT is the relevant endpoint we should use when
/// setting up the http route to be used to connect to our SSE endpoint.
pub static RELOADER_SSE_ENDPOINT: &'static str = "/static/sse/reload";

pub fn sse_endpoint_script(
    addr: SocketAddr,
    _request: crate::types::HyperRequest,
) -> pin::Pin<Box<crate::types::HyperFuture>> {
    ewe_logs::info!("Received request for SSE endpoint script from {}", addr);

    Box::pin(async move {
        let body = body::Body::new(crate::full(bytes::Bytes::from(RELOADER_SCRIPT_BYTES)));
        Ok(hyper::Response::builder()
            .header("Content-Type", "text/javascript")
            .status(StatusCode::OK)
            .body(body)
            .unwrap())
    })
}

fn sse_endpoint_reloader(
    addr: SocketAddr,
    _request: crate::types::HyperRequest,
    running_notification: broadcast::Receiver<()>,
) -> pin::Pin<Box<crate::types::HyperFuture>> {
    ewe_logs::info!("Seeing request for addr: {}", addr);
    Box::pin(async move {
        let running_stream = BroadcastStream::new(running_notification);
        Ok(Sse::new(
            // when declaring Result types for such cases, the error type must be explicit
            // else you will have type inference compiler errors
            running_stream.map(|_| -> Result<Event, crate::types::BoxedError> {
                Ok(Event::default()
                    .data("ready\n")
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

/// create_sse_endpoint_handler takes a `broadcast::Sender<()>`
/// which might suprise you till you figure out that the following rules
/// are involved:
/// 1. You are defining a Fn(addr, request) which can be called multiple times.
/// 2. Tokio's `broadcast::Receiver<T>` does not implement clone which means after
///    the first call it is moved out and in essence is owned by the
///    inner function and therefore can not be reused on the next one.
/// 3. Tokio's `broadcast::Sender<T>` implements Clone and we can create a new receiver
///    on each re-call.
pub fn create_sse_endpoint_handler(
    running_notification: broadcast::Sender<()>,
) -> sync::Arc<crate::types::HyperFunc> {
    sync::Arc::new(move |addr, request| {
        sse_endpoint_reloader(addr, request, running_notification.subscribe())
    })
}
