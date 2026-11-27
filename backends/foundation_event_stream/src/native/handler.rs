//! The reserved `__ds` subscription control endpoints.
//!
//! The handler is a [`Serve`] handler beside the Durable Streams handler. It owns the wire rules:
//! routing, status codes, the bearer header and the response headers. The engine owns the
//! protocol state.

use crate::shared::engine::SubscriptionEngine;
use crate::shared::error::{SubscriptionError, SubscriptionErrorCode};
use crate::shared::model::{AckRequest, ClaimRequest, CreateRequest, StreamsRequest};
use foundation_context::Ctx;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::native::serve::{Serve, ServeFactory};
use foundation_http::native::writer::{
    render_head, send_text, write_response, ResponseWriter, WriterConfig,
};
use foundation_http::shared::serve::{ConnectionResult, ServeOutcome};
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::client::body_reader::collect_bytes_bounded;
use foundation_netio::shared::http::{
    SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
};
use serde::Serialize;
use std::sync::Arc;

/// The subscription control handler.
pub struct SubscriptionHandler {
    engine: Option<Arc<SubscriptionEngine>>,
}

impl ServeFactory for SubscriptionHandler {
    fn create(ctx: &Ctx) -> Self {
        Self {
            engine: ctx.get::<SubscriptionEngine>(),
        }
    }
}

impl Serve for SubscriptionHandler {
    fn serve(
        &self,
        ctx: Ctx,
        mut req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let Some(engine) = self.engine.clone() else {
            return send_text(
                conn,
                500,
                "subscriptions are not installed in the context",
                &ctx,
            );
        };
        let config = engine.config().clone();
        let method = req.method.clone();
        let path = req.request_url.url.clone();
        let headers = req.headers.clone();
        let host = header_value(&headers, "host");
        let declared_body_len =
            header_value(&headers, "content-length").and_then(|value| value.parse::<usize>().ok());
        let body = req.take_async_body();

        ConnectionResult::Async(Box::pin(async move {
            if declared_body_len.is_some_and(|length| length > config.max_body_bytes) {
                return refuse(conn, &ctx, 413, "subscription request body is too large").await;
            }
            let body = match body {
                Some(body) => collect_bytes_bounded(body, config.max_body_bytes).await,
                None => Vec::new(),
            };
            let base_url = config.public_base_url.clone().unwrap_or_else(|| {
                format!(
                    "{}://{}",
                    config.public_scheme,
                    host.as_deref().unwrap_or("localhost")
                )
            });
            handle(engine, ctx, conn, method, &path, &headers, &base_url, &body).await
        }))
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle(
    engine: Arc<SubscriptionEngine>,
    ctx: Ctx,
    conn: SharedByteBufferStream<RawStream>,
    method: SimpleMethod,
    path: &str,
    headers: &SimpleHeaders,
    base_url: &str,
    body: &[u8],
) -> ServeOutcome {
    let config = engine.config().clone();
    let path = path.split('?').next().unwrap_or(path);
    let control = format!("/{}", config.control_prefix());
    let Some(rest) = path.strip_prefix(&control) else {
        return respond_error(conn, &ctx, 404, &SubscriptionError::not_found()).await;
    };

    if rest == "/jwks.json" {
        if method != SimpleMethod::GET {
            return method_not_allowed(conn, &ctx).await;
        }
        return match engine.jwks_json() {
            Ok(json) => {
                let headers = vec![
                    (
                        SimpleHeader::CONTENT_TYPE,
                        "application/jwk-set+json".to_string(),
                    ),
                    (
                        SimpleHeader::CACHE_CONTROL,
                        "public, max-age=300".to_string(),
                    ),
                ];
                respond_bytes(conn, &ctx, 200, headers, json.into_bytes()).await
            }
            Err(error) => respond_error(conn, &ctx, 500, &error).await,
        };
    }

    let Some(rest) = rest.strip_prefix("/subscriptions/") else {
        return respond_error(conn, &ctx, 404, &SubscriptionError::not_found()).await;
    };
    let (raw_id, tail) = rest
        .split_once('/')
        .map_or((rest, ""), |(id, tail)| (id, tail));
    let id = percent_decode(raw_id);
    if id.is_empty() {
        return respond_error(conn, &ctx, 404, &SubscriptionError::not_found()).await;
    }

    if tail.is_empty() {
        return handle_base(engine, ctx, conn, method, &id, base_url, body).await;
    }
    if tail == "streams" {
        if method != SimpleMethod::POST {
            return method_not_allowed(conn, &ctx).await;
        }
        return handle_add_streams(engine, ctx, conn, &id, body).await;
    }
    if let Some(stream_path) = tail.strip_prefix("streams/") {
        if method != SimpleMethod::DELETE {
            return method_not_allowed(conn, &ctx).await;
        }
        let stream_path = percent_decode(stream_path);
        return match engine.remove_explicit_stream(&id, &stream_path).await {
            Ok(()) => respond_empty(conn, &ctx, 204).await,
            Err(error) => respond_error(conn, &ctx, error.status(), &error).await,
        };
    }
    match tail {
        "callback" => handle_callback(engine, ctx, conn, &id, headers, method, body).await,
        "claim" => handle_claim(engine, ctx, conn, &id, method, body).await,
        "ack" => handle_ack(engine, ctx, conn, &id, headers, method, body).await,
        "release" => handle_release(engine, ctx, conn, &id, headers, method, body).await,
        _ => respond_error(conn, &ctx, 404, &SubscriptionError::not_found()).await,
    }
}

async fn handle_callback(
    engine: Arc<SubscriptionEngine>,
    ctx: Ctx,
    conn: SharedByteBufferStream<RawStream>,
    id: &str,
    headers: &SimpleHeaders,
    method: SimpleMethod,
    body: &[u8],
) -> ServeOutcome {
    let Some(token) = bearer_token(headers) else {
        return respond_error(conn, &ctx, 401, &missing_bearer()).await;
    };
    if method != SimpleMethod::POST {
        return method_not_allowed(conn, &ctx).await;
    }
    let request = match parse_body::<AckRequest>(body) {
        Ok(request) => request,
        Err(error) => return respond_error(conn, &ctx, 400, &error).await,
    };
    match engine.callback(id, &token, request).await {
        Ok(response) => respond_json(conn, &ctx, 200, &response).await,
        Err(error) => respond_error(conn, &ctx, error.status(), &error).await,
    }
}

async fn handle_claim(
    engine: Arc<SubscriptionEngine>,
    ctx: Ctx,
    conn: SharedByteBufferStream<RawStream>,
    id: &str,
    method: SimpleMethod,
    body: &[u8],
) -> ServeOutcome {
    if method != SimpleMethod::POST {
        return method_not_allowed(conn, &ctx).await;
    }
    let request = match parse_body::<ClaimRequest>(body) {
        Ok(request) => request,
        Err(error) => return respond_error(conn, &ctx, 400, &error).await,
    };
    if request.worker.is_empty() {
        return respond_error(
            conn,
            &ctx,
            400,
            &SubscriptionError::invalid("worker must be a non-empty string"),
        )
        .await;
    }
    match engine.claim(id, &request.worker).await {
        Ok(response) => respond_json(conn, &ctx, 200, &response).await,
        Err(error) => respond_error(conn, &ctx, error.status(), &error).await,
    }
}

async fn handle_ack(
    engine: Arc<SubscriptionEngine>,
    ctx: Ctx,
    conn: SharedByteBufferStream<RawStream>,
    id: &str,
    headers: &SimpleHeaders,
    method: SimpleMethod,
    body: &[u8],
) -> ServeOutcome {
    let Some(token) = bearer_token(headers) else {
        return respond_error(conn, &ctx, 401, &missing_bearer()).await;
    };
    if method != SimpleMethod::POST {
        return method_not_allowed(conn, &ctx).await;
    }
    let request = match parse_body::<AckRequest>(body) {
        Ok(request) => request,
        Err(error) => return respond_error(conn, &ctx, 400, &error).await,
    };
    match engine.ack(id, &token, request).await {
        Ok(response) => respond_json(conn, &ctx, 200, &response).await,
        Err(error) => respond_error(conn, &ctx, error.status(), &error).await,
    }
}

async fn handle_release(
    engine: Arc<SubscriptionEngine>,
    ctx: Ctx,
    conn: SharedByteBufferStream<RawStream>,
    id: &str,
    headers: &SimpleHeaders,
    method: SimpleMethod,
    body: &[u8],
) -> ServeOutcome {
    let Some(token) = bearer_token(headers) else {
        return respond_error(conn, &ctx, 401, &missing_bearer()).await;
    };
    if method != SimpleMethod::POST {
        return method_not_allowed(conn, &ctx).await;
    }
    let request = match parse_body::<AckRequest>(body) {
        Ok(request) => request,
        Err(error) => return respond_error(conn, &ctx, 400, &error).await,
    };
    match engine.release(id, &token, request).await {
        Ok(()) => respond_empty(conn, &ctx, 204).await,
        Err(error) => respond_error(conn, &ctx, error.status(), &error).await,
    }
}

/// The error for a missing or malformed `Authorization` header.
fn missing_bearer() -> SubscriptionError {
    SubscriptionError::new(
        SubscriptionErrorCode::TokenInvalid,
        "Missing or malformed Authorization header",
    )
}

async fn handle_base(
    engine: Arc<SubscriptionEngine>,
    ctx: Ctx,
    conn: SharedByteBufferStream<RawStream>,
    method: SimpleMethod,
    id: &str,
    base_url: &str,
    body: &[u8],
) -> ServeOutcome {
    match method {
        SimpleMethod::PUT => {
            let request = match parse_body::<CreateRequest>(body) {
                Ok(request) => request,
                Err(error) => return respond_error(conn, &ctx, 400, &error).await,
            };
            match engine.create_or_confirm(id, request, base_url).await {
                Ok((view, created)) => {
                    let status = if created { 201 } else { 200 };
                    respond_json(conn, &ctx, status, &view).await
                }
                Err(error) => respond_error(conn, &ctx, error.status(), &error).await,
            }
        }
        SimpleMethod::GET => match engine.read(id).await {
            Ok(view) => respond_json(conn, &ctx, 200, &view).await,
            Err(error) => respond_error(conn, &ctx, error.status(), &error).await,
        },
        SimpleMethod::DELETE => {
            engine.delete(id).await;
            respond_empty(conn, &ctx, 204).await
        }
        _ => method_not_allowed(conn, &ctx).await,
    }
}

async fn handle_add_streams(
    engine: Arc<SubscriptionEngine>,
    ctx: Ctx,
    conn: SharedByteBufferStream<RawStream>,
    id: &str,
    body: &[u8],
) -> ServeOutcome {
    let request = match parse_body::<StreamsRequest>(body) {
        Ok(request) => request,
        Err(error) => return respond_error(conn, &ctx, 400, &error).await,
    };
    match engine.add_explicit_streams(id, request.streams).await {
        Ok(()) => respond_empty(conn, &ctx, 204).await,
        Err(error) => respond_error(conn, &ctx, error.status(), &error).await,
    }
}

fn parse_body<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, SubscriptionError> {
    if body.is_empty() {
        return serde_json::from_slice(b"{}")
            .map_err(|_| SubscriptionError::invalid("Invalid JSON body"));
    }
    serde_json::from_slice(body).map_err(|_| SubscriptionError::invalid("Invalid JSON body"))
}

fn header_value(headers: &SimpleHeaders, name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(key, _)| key.to_string().eq_ignore_ascii_case(name))
        .and_then(|(_, values)| values.first().cloned())
}

fn bearer_token(headers: &SimpleHeaders) -> Option<String> {
    header_value(headers, "authorization")
        .and_then(|value| value.strip_prefix("Bearer ").map(str::to_string))
}

async fn respond_json(
    conn: SharedByteBufferStream<RawStream>,
    ctx: &Ctx,
    status: u16,
    value: &impl Serialize,
) -> ServeOutcome {
    let body = match serde_json::to_vec(value) {
        Ok(body) => body,
        Err(error) => {
            tracing::error!("subscription response encode failed: {error}");
            return ServeOutcome::Close(None);
        }
    };
    let headers = vec![(SimpleHeader::CONTENT_TYPE, "application/json".to_string())];
    respond_bytes(conn, ctx, status, headers, body).await
}

async fn respond_error(
    conn: SharedByteBufferStream<RawStream>,
    ctx: &Ctx,
    status: u16,
    error: &SubscriptionError,
) -> ServeOutcome {
    let mut detail = serde_json::Map::new();
    detail.insert(
        "code".to_string(),
        serde_json::Value::String(error.code.as_str().to_string()),
    );
    detail.insert(
        "message".to_string(),
        serde_json::Value::String(error.message.clone()),
    );
    if let Some(holder) = &error.current_holder {
        detail.insert(
            "current_holder".to_string(),
            serde_json::Value::String(holder.clone()),
        );
    }
    if let Some(generation) = error.generation {
        detail.insert(
            "generation".to_string(),
            serde_json::Value::from(generation),
        );
    }
    let body = serde_json::json!({ "error": detail });
    respond_json(conn, ctx, status, &body).await
}

async fn respond_empty(
    conn: SharedByteBufferStream<RawStream>,
    ctx: &Ctx,
    status: u16,
) -> ServeOutcome {
    respond_bytes(conn, ctx, status, Vec::new(), Vec::new()).await
}

async fn respond_bytes(
    conn: SharedByteBufferStream<RawStream>,
    ctx: &Ctx,
    status: u16,
    headers: Vec<(SimpleHeader, String)>,
    body: Vec<u8>,
) -> ServeOutcome {
    let head = match render_head(status, &headers, body.len()) {
        Ok(head) => head,
        Err(error) => {
            tracing::error!("subscription response head build failed: {error}");
            return ServeOutcome::Close(None);
        }
    };
    let writer = ResponseWriter::new(conn, WriterConfig::default(), ctx.clone());
    write_response(writer, head, body).await
}

async fn refuse(
    conn: SharedByteBufferStream<RawStream>,
    ctx: &Ctx,
    status: u16,
    message: &str,
) -> ServeOutcome {
    let headers = vec![(
        SimpleHeader::CONTENT_TYPE,
        "text/plain; charset=utf-8".to_string(),
    )];
    if let Ok(head) = render_head(status, &headers, message.len()) {
        let mut writer = ResponseWriter::new(conn, WriterConfig::default(), ctx.clone());
        let _ = writer.write_all(&head).await;
        let _ = writer.write_all(message.as_bytes()).await;
        let _ = writer.flush().await;
    }
    ServeOutcome::Close(None)
}

async fn method_not_allowed(conn: SharedByteBufferStream<RawStream>, ctx: &Ctx) -> ServeOutcome {
    respond_bytes(
        conn,
        ctx,
        405,
        vec![(
            SimpleHeader::CONTENT_TYPE,
            "text/plain; charset=utf-8".to_string(),
        )],
        b"Method Not Allowed".to_vec(),
    )
    .await
}

/// Decode percent escapes in a path segment.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_digit(bytes[index + 1]), hex_digit(bytes[index + 2]))
            {
                out.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
