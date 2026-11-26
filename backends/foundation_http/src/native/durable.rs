//! The native HTTP adapter for Durable Streams.
//!
//! The adapter owns the wire rules. It calls the transport-neutral `DurableService`.

use crate::native::serve::{Serve, ServeFactory};
use crate::native::writer::{render_head, write_response, ResponseWriter, WriterConfig};
use crate::shared::durable::parse::{
    encode_offset, parse_offset, parse_producer, parse_stream_closed, parse_stream_seq, parse_ttl,
};
use crate::shared::durable::status::{header, status_for};
use crate::shared::serve::ServeOutcome;
use foundation_context::Ctx;
use foundation_core::durable::cache::{etag, CachePolicy};
use foundation_core::durable::{
    AppendRequest, Chunk, CloseRequest, ContentType, CreateSpec, DurableLimits, DurableService,
    DurableStream, Expiry, ForkSpec, ReadFrom, ReadLimit, ReadRequest, StoreError,
    StreamName, StreamOffset,
};
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::client::body_reader::collect_bytes_bounded;
use foundation_netio::shared::http::{
    SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The largest request body the adapter reads. 8 MiB: the wasm bridge default, well below the
/// Cloudflare 100 MB cap and above the nginx 1 MiB default.
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// The Durable Streams HTTP handler.
pub struct DurableHandler {
    service: Option<Arc<DurableService<DurableStream>>>,
}

impl ServeFactory for DurableHandler {
    fn create(ctx: &Ctx) -> Self {
        Self {
            service: ctx.get::<DurableStream>().map(|stream| {
                Arc::new(DurableService::with_limits(
                    (*stream).clone(),
                    DurableLimits::default(),
                ))
            }),
        }
    }
}

/// One request, after the path has become a stream name.
///
/// This is the adapter's unit: a transport hands it one of these and gets a
/// [`DurableResponse`] or a [`StoreError`]. A test builds it directly, so the
/// wire rules are tested without a socket.
#[derive(Debug)]
pub struct DurableRequest {
    /// The HTTP method.
    pub method: SimpleMethod,
    /// The stream the path names.
    pub name: StreamName,
    /// The request headers.
    pub headers: SimpleHeaders,
    /// The query parameters, if the URL had any.
    pub queries: Option<BTreeMap<String, String>>,
    /// The request body. Empty when the request had none.
    pub body: Vec<u8>,
}

/// One response, ready for the wire.
#[derive(Debug)]
pub struct DurableResponse {
    /// The HTTP status code.
    pub status: u16,
    /// The response headers, in the order they go on the wire.
    pub headers: Vec<(SimpleHeader, String)>,
    /// The response body.
    pub body: Vec<u8>,
}

impl DurableResponse {
    /// The first value of a response header, matched without case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.to_string().eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Turns a URL path into a stream name.
///
/// The name is the path without its leading `/`. A query string is dropped.
///
/// # Errors
/// Returns [`StoreError::BadRequest`] when the name is not valid.
pub fn stream_name_from_path(path: &str) -> Result<StreamName, StoreError> {
    let path = path.split('?').next().unwrap_or(path);
    let path = path.strip_prefix('/').unwrap_or(path);
    StreamName::parse(path)
}

/// Answers one request against the service.
///
/// The adapter calls this from `serve`. A test calls it directly.
///
/// # Errors
/// Returns the [`StoreError`] the service reported. The caller maps it to a
/// status with [`status_for`].
pub async fn dispatch(
    service: &DurableService<DurableStream>,
    request: DurableRequest,
) -> Result<DurableResponse, StoreError> {
    match request.method {
        SimpleMethod::GET => {
            handle_get(
                service,
                &request.name,
                &request.headers,
                request.queries.as_ref(),
            )
            .await
        }
        SimpleMethod::HEAD => handle_head(service, &request.name).await,
        SimpleMethod::PUT => {
            handle_put(service, request.name, &request.headers, request.body).await
        }
        SimpleMethod::POST => {
            handle_post(service, &request.name, &request.headers, request.body).await
        }
        SimpleMethod::DELETE => handle_delete(service, &request.name).await,
        _ => Ok(DurableResponse {
            status: 405,
            headers: base_headers(),
            body: b"method not allowed".to_vec(),
        }),
    }
}

impl Serve for DurableHandler {
    fn serve(
        &self,
        ctx: Ctx,
        mut req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> crate::shared::serve::ConnectionResult {
        let Some(service) = self.service.clone() else {
            return crate::native::writer::send_text(
                conn,
                500,
                "durable store is not in the context",
                &ctx,
            );
        };

        let method = req.method.clone();
        let path = req.request_url.url.clone();
        let headers = req.headers.clone();
        let queries = req.request_url.queries.clone();
        let body = req.take_async_body();

        crate::shared::serve::ConnectionResult::Async(Box::pin(async move {
            let body = match body {
                Some(body) => collect_bytes_bounded(body, MAX_BODY_BYTES).await,
                None => Vec::new(),
            };

            let name = match stream_name_from_path(&path) {
                Ok(name) => name,
                Err(error) => return respond_error(conn, &ctx, &error).await,
            };

            let request = DurableRequest {
                method,
                name,
                headers,
                queries,
                body,
            };

            match dispatch(&service, request).await {
                Ok(response) => respond(conn, &ctx, response).await,
                Err(error) => respond_error(conn, &ctx, &error).await,
            }
        }))
    }
}

async fn respond(
    conn: SharedByteBufferStream<RawStream>,
    ctx: &Ctx,
    response: DurableResponse,
) -> ServeOutcome {
    let head = match render_head(response.status, &response.headers, response.body.len()) {
        Ok(head) => head,
        Err(error) => {
            tracing::error!("durable response build failed: {error}");
            return ServeOutcome::Close(None);
        }
    };
    let writer = ResponseWriter::new(conn, WriterConfig::default(), ctx.clone());
    write_response(writer, head, response.body).await
}

async fn respond_error(
    conn: SharedByteBufferStream<RawStream>,
    ctx: &Ctx,
    error: &StoreError,
) -> ServeOutcome {
    let mut headers = base_headers();
    headers.push((
        SimpleHeader::CONTENT_TYPE,
        "text/plain; charset=utf-8".to_string(),
    ));
    respond(
        conn,
        ctx,
        DurableResponse {
            status: status_for(error),
            headers,
            body: error.to_string().into_bytes(),
        },
    )
    .await
}

fn base_headers() -> Vec<(SimpleHeader, String)> {
    vec![
        (
            SimpleHeader::X_CONTENT_TYPE_OPTIONS,
            "nosniff".to_string(),
        ),
        (
            SimpleHeader::custom("cross-origin-resource-policy"),
            "cross-origin".to_string(),
        ),
    ]
}

fn header_value(headers: &SimpleHeaders, name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(key, _)| key.to_string().eq_ignore_ascii_case(name))
        .and_then(|(_, values)| values.first().cloned())
}

fn content_type(headers: &SimpleHeaders) -> Result<ContentType, StoreError> {
    match header_value(headers, "content-type") {
        Some(value) => ContentType::parse(&value),
        None => Ok(ContentType::default()),
    }
}

async fn handle_get(
    service: &DurableService<DurableStream>,
    name: &StreamName,
    headers: &SimpleHeaders,
    queries: Option<&BTreeMap<String, String>>,
) -> Result<DurableResponse, StoreError> {
    if queries.and_then(|queries| queries.get("live")).is_some() {
        return Err(StoreError::Unsupported);
    }

    let offset = queries.and_then(|queries| queries.get("offset"));
    let from = match offset {
        Some(value) => parse_offset(value)?,
        None => ReadFrom::Start,
    };

    let head = service.head(name).await?;
    let page = service
        .read(
            name,
            ReadRequest {
                from,
                limit: ReadLimit::default(),
            },
        )
        .await?;

    let mut body = Vec::new();
    if head.content_type.is_json() {
        body.push(b'[');
        for (index, chunk) in page.chunks.iter().enumerate() {
            if index > 0 {
                body.push(b',');
            }
            body.extend_from_slice(chunk.as_bytes());
        }
        body.push(b']');
    } else {
        for chunk in &page.chunks {
            body.extend_from_slice(chunk.as_bytes());
        }
    }

    let closed = page.cursor.closed;
    let tag = etag(name.as_str(), 0, body.len() as u64, closed);
    if header_value(headers, "if-none-match").as_deref() == Some(tag.as_str()) {
        return Ok(DurableResponse {
            status: 304,
            headers: base_headers(),
            body: Vec::new(),
        });
    }

    let mut response_headers = base_headers();
    response_headers.push((
        SimpleHeader::CONTENT_TYPE,
        head.content_type.as_str().to_string(),
    ));
    response_headers.push((
        SimpleHeader::custom(header::STREAM_NEXT_OFFSET),
        offset_text(&page.cursor.next),
    ));
    if page.cursor.up_to_date {
        response_headers.push((
            SimpleHeader::custom(header::STREAM_UP_TO_DATE),
            "true".to_string(),
        ));
    }
    if closed {
        response_headers.push((
            SimpleHeader::custom(header::STREAM_CLOSED),
            "true".to_string(),
        ));
    }
    response_headers.push((SimpleHeader::ETAG, tag));
    response_headers.push((
        SimpleHeader::CACHE_CONTROL,
        CachePolicy::Public.header_value().to_string(),
    ));

    Ok(DurableResponse {
        status: 200,
        headers: response_headers,
        body,
    })
}

async fn handle_head(
    service: &DurableService<DurableStream>,
    name: &StreamName,
) -> Result<DurableResponse, StoreError> {
    let head = service.head(name).await?;
    let mut headers = base_headers();
    headers.push((
        SimpleHeader::CONTENT_TYPE,
        head.content_type.as_str().to_string(),
    ));
    headers.push((
        SimpleHeader::custom(header::STREAM_NEXT_OFFSET),
        offset_text(&head.tail),
    ));
    headers.push((
        SimpleHeader::CACHE_CONTROL,
        CachePolicy::NoStore.header_value().to_string(),
    ));
    if head.closed {
        headers.push((
            SimpleHeader::custom(header::STREAM_CLOSED),
            "true".to_string(),
        ));
    }
    if let Some(Expiry::Ttl(ttl)) = head.expires {
        headers.push((
            SimpleHeader::custom(header::STREAM_TTL),
            ttl.as_secs().to_string(),
        ));
    }

    Ok(DurableResponse {
        status: 200,
        headers,
        body: Vec::new(),
    })
}

async fn handle_put(
    service: &DurableService<DurableStream>,
    name: StreamName,
    headers: &SimpleHeaders,
    body: Vec<u8>,
) -> Result<DurableResponse, StoreError> {
    let content_type = content_type(headers)?;
    let closed = parse_stream_closed(header_value(headers, "stream-closed").as_deref());
    let ttl = header_value(headers, "stream-ttl")
        .map(|value| parse_ttl(&value))
        .transpose()?;
    let expires = Expiry::from_headers(ttl, None)?;

    let fork = header_value(headers, "stream-forked-from")
        .map(|source| -> Result<ForkSpec, StoreError> {
            let source = StreamName::parse(source.trim_start_matches('/'))?;
            let at = header_value(headers, "stream-fork-offset")
                .map(|value| StreamOffset::from_bytes(value.into_bytes()));
            let sub_offset = match header_value(headers, "stream-fork-sub-offset") {
                Some(value) => value
                    .parse::<u32>()
                    .map_err(|_| StoreError::BadRequest(foundation_core::durable::BadRequestKind::Fork))?,
                None => 0,
            };
            Ok(ForkSpec {
                source,
                at,
                sub_offset,
            })
        })
        .transpose()?;

    let spec = CreateSpec {
        content_type: Some(content_type),
        expires,
        closed,
        initial: (!body.is_empty()).then(|| Chunk::new(body)),
        fork,
    };

    let outcome = service.create(name.clone(), spec).await?;
    let status = if outcome.created { 201 } else { 200 };
    let mut response_headers = base_headers();
    response_headers.push((
        SimpleHeader::CONTENT_TYPE,
        outcome.head.content_type.as_str().to_string(),
    ));
    response_headers.push((SimpleHeader::LOCATION, format!("/{}", name.as_str())));
    response_headers.push((
        SimpleHeader::custom(header::STREAM_NEXT_OFFSET),
        offset_text(&outcome.head.tail),
    ));
    if outcome.head.closed {
        response_headers.push((
            SimpleHeader::custom(header::STREAM_CLOSED),
            "true".to_string(),
        ));
    }

    Ok(DurableResponse {
        status,
        headers: response_headers,
        body: Vec::new(),
    })
}

async fn handle_post(
    service: &DurableService<DurableStream>,
    name: &StreamName,
    headers: &SimpleHeaders,
    body: Vec<u8>,
) -> Result<DurableResponse, StoreError> {
    let close = parse_stream_closed(header_value(headers, "stream-closed").as_deref());
    let producer = parse_producer(
        header_value(headers, "producer-id").as_deref(),
        header_value(headers, "producer-epoch").as_deref(),
        header_value(headers, "producer-seq").as_deref(),
    )?;
    let stream_seq = header_value(headers, "stream-seq").map(|value| parse_stream_seq(&value));

    let outcome = if body.is_empty() {
        if !close {
            return Err(StoreError::BadRequest(
                foundation_core::durable::BadRequestKind::Body,
            ));
        }
        service
            .close(name, CloseRequest {
                final_chunk: None,
                producer: producer.clone(),
            })
            .await?
    } else {
        let content_type = content_type(headers)?;
        service
            .append(
                name,
                AppendRequest {
                    chunk: Chunk::new(body),
                    content_type: Some(content_type),
                    stream_seq,
                    producer: producer.clone(),
                    close,
                },
            )
            .await?
    };

    let mut response_headers = base_headers();
    response_headers.push((
        SimpleHeader::custom(header::STREAM_NEXT_OFFSET),
        offset_text(&outcome.cursor.next),
    ));
    if let Some(producer) = &producer {
        response_headers.push((
            SimpleHeader::custom(header::PRODUCER_EPOCH),
            producer.epoch.to_string(),
        ));
        response_headers.push((
            SimpleHeader::custom(header::PRODUCER_SEQ),
            outcome
                .producer_seq
                .unwrap_or(producer.seq)
                .to_string(),
        ));
    }
    if outcome.closed {
        response_headers.push((
            SimpleHeader::custom(header::STREAM_CLOSED),
            "true".to_string(),
        ));
    }

    let status = if producer.is_some() {
        if matches!(outcome.kind, foundation_core::durable::AppendKind::Duplicate) {
            204
        } else {
            200
        }
    } else {
        204
    };

    Ok(DurableResponse {
        status,
        headers: response_headers,
        body: Vec::new(),
    })
}

async fn handle_delete(
    service: &DurableService<DurableStream>,
    name: &StreamName,
) -> Result<DurableResponse, StoreError> {
    service.delete(name).await?;
    Ok(DurableResponse {
        status: 204,
        headers: base_headers(),
        body: Vec::new(),
    })
}

fn offset_text(offset: &StreamOffset) -> String {
    encode_offset(offset)
}
