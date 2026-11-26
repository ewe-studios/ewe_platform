//! The native HTTP adapter for Durable Streams.
//!
//! The adapter owns the wire rules. It calls the transport-neutral `DurableService`.

use crate::native::serve::{Serve, ServeFactory};
use crate::native::writer::{
    render_head, render_message, write_response, ResponseWriter, WriterConfig,
};
use crate::shared::durable::parse::{
    decode_offset, encode_offset, parse_expires_at, parse_offset, parse_producer,
    parse_stream_closed, parse_stream_seq, parse_sub_offset, parse_ttl,
};
use crate::shared::durable::status::{header, status_for};
use crate::shared::serve::ServeOutcome;
use base64::Engine as _;
use foundation_compact::rng::Rng;
use foundation_context::Ctx;
use foundation_core::durable::cache::{etag, generate_cursor, CachePolicy, CursorConfig};
use foundation_core::durable::{
    AppendRequest, Chunk, CloseRequest, ContentType, CreateSpec, DurableLimits, DurableService,
    DurableStream, Expiry, ForkSpec, ReadFrom, ReadLimit, ReadPage, ReadRequest, StoreError,
    StreamName, StreamOffset,
};
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::timer::sleep_for;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::client::body_reader::collect_bytes_bounded;
use foundation_netio::shared::http::{
    SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, Status,
};
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, UNIX_EPOCH};

/// The largest request body the adapter reads. 8 MiB: the wasm bridge default, well below the
/// Cloudflare 100 MB cap and above the nginx 1 MiB default.
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// The Durable Streams HTTP handler.
pub struct DurableHandler {
    service: Option<Arc<DurableService<DurableStream>>>,
}

impl ServeFactory for DurableHandler {
    fn create(ctx: &Ctx) -> Self {
        let limits = ctx
            .get::<DurableLimits>()
            .map_or_else(DurableLimits::default, |limits| *limits);
        Self {
            service: ctx.get::<DurableStream>().map(|stream| {
                Arc::new(DurableService::with_limits((*stream).clone(), limits))
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

/// How a GET reads the stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveMode {
    /// Read what exists now and return.
    CatchUp,
    /// Wait for new data, then return it.
    LongPoll,
    /// Keep the connection open and send events.
    Sse,
}

/// Read the `live` query parameter.
///
/// # Errors
/// Returns [`StoreError::BadRequest`] for an unknown live mode.
pub fn live_mode(queries: Option<&BTreeMap<String, String>>) -> Result<LiveMode, StoreError> {
    match queries.and_then(|queries| queries.get("live")).map(String::as_str) {
        None => Ok(LiveMode::CatchUp),
        Some("long-poll") => Ok(LiveMode::LongPoll),
        Some("sse") => Ok(LiveMode::Sse),
        Some(_) => Err(StoreError::BadRequest(
            foundation_core::durable::BadRequestKind::Header,
        )),
    }
}

/// Answers one request against the service.
///
/// The adapter calls this from `serve`. A test calls it directly. Server-Sent
/// Events (SSE) needs the connection, so `serve` handles it before this call.
///
/// # Errors
/// Returns the [`StoreError`] the service reported. The caller maps it to a
/// status with [`status_for`].
pub async fn dispatch(
    service: &DurableService<DurableStream>,
    ctx: &Ctx,
    request: DurableRequest,
) -> Result<DurableResponse, StoreError> {
    match request.method {
        SimpleMethod::GET => {
            handle_get(
                service,
                ctx,
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
        SimpleMethod::OPTIONS => Ok(DurableResponse {
            status: 204,
            headers: cors_preflight_headers(),
            body: Vec::new(),
        }),
        other => {
            tracing::warn!(method = ?other, "durable: unsupported request method");
            Ok(DurableResponse {
                status: 405,
                headers: base_headers(),
                body: b"method not allowed".to_vec(),
            })
        }
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
        let host = header_value(&req.headers, "host");
        let empty_offset = has_empty_offset(&req.request_uri.to_string());
        let declared_body_len = header_value(&req.headers, "content-length")
            .and_then(|value| value.parse::<usize>().ok());
        let body = req.take_async_body();

        crate::shared::serve::ConnectionResult::Async(Box::pin(async move {
            let body = match body {
                Some(body) => collect_bytes_bounded(body, MAX_BODY_BYTES).await,
                None => Vec::new(),
            };

            // A short body leaves its tail on the connection. That tail then
            // corrupts the next request line, so close the connection.
            if short_body(declared_body_len, body.len()) {
                if let Some(expected) = declared_body_len {
                    tracing::warn!(
                        expected,
                        read = body.len(),
                        "durable: short request body, closing the connection"
                    );
                    let message = "request body is shorter than its declared length";
                    let headers = vec![(
                        SimpleHeader::CONTENT_TYPE,
                        "text/plain; charset=utf-8".to_string(),
                    )];
                    if let Ok(head) = render_head(400, &headers, message.len()) {
                        let mut writer =
                            ResponseWriter::new(conn, WriterConfig::default(), ctx.clone());
                        let _ = writer.write_all(&head).await;
                        let _ = writer.write_all(message.as_bytes()).await;
                        let _ = writer.flush().await;
                    }
                    return ServeOutcome::Close(None);
                }
            }

            if empty_offset && method == SimpleMethod::GET {
                let error = StoreError::BadRequest(
                    foundation_core::durable::BadRequestKind::Offset,
                );
                return respond_error(conn, &ctx, &error).await;
            }

            let name = match stream_name_from_path(&path) {
                Ok(name) => name,
                Err(error) => return respond_error(conn, &ctx, &error).await,
            };

            if method == SimpleMethod::GET {
                match live_mode(queries.as_ref()) {
                    Ok(LiveMode::Sse) => {
                        return serve_sse(&service, &ctx, &name, queries.as_ref(), conn).await;
                    }
                    Ok(_) => {}
                    Err(error) => return respond_error(conn, &ctx, &error).await,
                }
            }

            let request = DurableRequest {
                method,
                name,
                headers,
                queries,
                body,
            };

            match dispatch(&service, &ctx, request).await {
                Ok(mut response) => {
                    absolutize_location(&mut response, host.as_deref());
                    respond(conn, &ctx, response).await
                }
                Err(error) => respond_error(conn, &ctx, &error).await,
            }
        }))
    }
}

/// True when the read body is shorter or longer than its declared length.
#[must_use]
pub fn short_body(declared: Option<usize>, read: usize) -> bool {
    declared.is_some_and(|declared| declared != read)
}

/// True when the raw query string names `offset` with an empty value.
///
/// The query parser drops an empty value, so the adapter reads the raw URI to
/// refuse `?offset=` with `400`.
fn has_empty_offset(uri: &str) -> bool {
    uri.split_once('?')
        .map(|(_, query)| {
            query
                .split('&')
                .any(|pair| pair == "offset=")
        })
        .unwrap_or(false)
}

/// Make a relative `Location` absolute for the request host.
fn absolutize_location(response: &mut DurableResponse, host: Option<&str>) {
    let Some(host) = host else {
        return;
    };
    for (key, value) in &mut response.headers {
        if *key == SimpleHeader::LOCATION && value.starts_with('/') {
            *value = format!("http://{host}{value}");
        }
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
    headers.extend(error_headers(error));
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

/// The protocol headers an error response must carry.
fn error_headers(error: &StoreError) -> Vec<(SimpleHeader, String)> {
    match error {
        StoreError::Fenced { current_epoch } => vec![(
            SimpleHeader::custom(header::PRODUCER_EPOCH),
            current_epoch.to_string(),
        )],
        StoreError::SequenceGap { expected, received } => vec![
            (
                SimpleHeader::custom(header::PRODUCER_EXPECTED_SEQ),
                expected.to_string(),
            ),
            (
                SimpleHeader::custom(header::PRODUCER_RECEIVED_SEQ),
                received.to_string(),
            ),
        ],
        StoreError::Closed { tail } => vec![
            (
                SimpleHeader::custom(header::STREAM_CLOSED),
                "true".to_string(),
            ),
            (
                SimpleHeader::custom(header::STREAM_NEXT_OFFSET),
                offset_text(tail),
            ),
        ],
        _ => Vec::new(),
    }
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
        // A browser client reads the protocol headers on a cross-origin read.
        (
            SimpleHeader::custom("access-control-expose-headers"),
            "Stream-Next-Offset, Stream-Cursor, Stream-Up-To-Date, Stream-Closed, ETag, Location"
                .to_string(),
        ),
    ]
}

/// The cross-origin preflight answer. `*` satisfies a request without credentials.
fn cors_preflight_headers() -> Vec<(SimpleHeader, String)> {
    let mut headers = base_headers();
    headers.push((
        SimpleHeader::custom("access-control-allow-origin"),
        "*".to_string(),
    ));
    headers.push((
        SimpleHeader::custom("access-control-allow-methods"),
        "GET, POST, PUT, HEAD, DELETE, OPTIONS".to_string(),
    ));
    headers.push((
        SimpleHeader::custom("access-control-allow-headers"),
        "*".to_string(),
    ));
    headers.push((
        SimpleHeader::custom("access-control-max-age"),
        "86400".to_string(),
    ));
    headers
}

fn header_value(headers: &SimpleHeaders, name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(key, _)| key.to_string().eq_ignore_ascii_case(name))
        .and_then(|(_, values)| values.first().cloned())
}

/// The wire body for one read page.
///
/// JSON streams wrap the page in a JSON array. Every other content type
/// concatenates the chunks.
#[must_use]
pub fn render_page_body(content_type: &ContentType, page: &ReadPage) -> Vec<u8> {
    let mut body = Vec::new();
    if content_type.is_json() {
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
    body
}

fn cursor_value(ctx: &Ctx, client: Option<u64>) -> u64 {
    let now = ctx
        .clock()
        .wall_now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let jitter = {
        let rng = ctx.rng();
        let mut rng = rng
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (rng.next_u64() % 3600) + 1
    };
    generate_cursor(client, now, &CursorConfig::default(), jitter)
}

fn client_cursor(queries: Option<&BTreeMap<String, String>>) -> Option<u64> {
    queries
        .and_then(|queries| queries.get("cursor"))
        .and_then(|value| value.parse::<u64>().ok())
}

/// A one-shot wake when a stream appends or closes.
struct AppendWait {
    fired: Arc<AtomicBool>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl Future for AppendWait {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.fired.load(Ordering::SeqCst) {
            return Poll::Ready(());
        }
        *self
            .waker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(cx.waker().clone());
        if self.fired.load(Ordering::SeqCst) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

/// Register a wake on the stream notifier before the first read.
///
/// A store without a notifier fires at once, so a live read then answers
/// without data instead of hanging.
fn append_wait(service: &DurableService<DurableStream>, name: &StreamName) -> AppendWait {
    let fired = Arc::new(AtomicBool::new(false));
    let waker: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(None));
    match service.notifier(name) {
        Some(notifier) => {
            let fired = fired.clone();
            let slot = waker.clone();
            notifier.register(Box::new(move || {
                fired.store(true, Ordering::SeqCst);
                if let Some(waker) = slot
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take()
                {
                    waker.wake();
                }
            }));
        }
        None => fired.store(true, Ordering::SeqCst),
    }
    AppendWait { fired, waker }
}

/// A future that completes when either input completes.
struct First<A, B> {
    first: A,
    second: B,
}

impl<A, B> Future for First<A, B>
where
    A: Future<Output = ()> + Unpin,
    B: Future<Output = ()> + Unpin,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        if Pin::new(&mut this.first).poll(cx).is_ready() {
            return Poll::Ready(());
        }
        if Pin::new(&mut this.second).poll(cx).is_ready() {
            return Poll::Ready(());
        }
        Poll::Pending
    }
}

async fn wait_or_timeout(wait: AppendWait, timeout: Duration, ctx: &Ctx) {
    First {
        first: wait,
        second: sleep_for(timeout, ctx),
    }
    .await;
}

/// Build a live response for one page.
///
/// A page with no chunks answers `204`; a page with chunks answers `200`.
fn live_response(
    ctx: &Ctx,
    content_type: &ContentType,
    page: &ReadPage,
    client: Option<u64>,
) -> DurableResponse {
    let closed = page.cursor.closed;
    let mut headers = base_headers();
    headers.push((
        SimpleHeader::custom(header::STREAM_NEXT_OFFSET),
        offset_text(&page.cursor.next),
    ));
    if page.cursor.up_to_date || closed {
        headers.push((
            SimpleHeader::custom(header::STREAM_UP_TO_DATE),
            "true".to_string(),
        ));
    }
    if closed {
        headers.push((
            SimpleHeader::custom(header::STREAM_CLOSED),
            "true".to_string(),
        ));
    } else {
        headers.push((
            SimpleHeader::custom(header::STREAM_CURSOR),
            cursor_value(ctx, client).to_string(),
        ));
    }
    headers.push((
        SimpleHeader::CACHE_CONTROL,
        CachePolicy::NoCache.header_value().to_string(),
    ));

    if page.chunks.is_empty() {
        return DurableResponse {
            status: 204,
            headers,
            body: Vec::new(),
        };
    }

    headers.push((
        SimpleHeader::CONTENT_TYPE,
        content_type.as_str().to_string(),
    ));
    DurableResponse {
        status: 200,
        headers,
        body: render_page_body(content_type, page),
    }
}

async fn handle_get(
    service: &DurableService<DurableStream>,
    ctx: &Ctx,
    name: &StreamName,
    headers: &SimpleHeaders,
    queries: Option<&BTreeMap<String, String>>,
) -> Result<DurableResponse, StoreError> {
    match live_mode(queries)? {
        LiveMode::CatchUp => handle_catch_up(service, ctx, name, headers, queries).await,
        LiveMode::LongPoll => handle_long_poll(service, ctx, name, queries).await,
        // SSE needs the connection, so `serve` handles it before dispatch.
        LiveMode::Sse => Err(StoreError::Unsupported),
    }
}

async fn handle_catch_up(
    service: &DurableService<DurableStream>,
    ctx: &Ctx,
    name: &StreamName,
    headers: &SimpleHeaders,
    queries: Option<&BTreeMap<String, String>>,
) -> Result<DurableResponse, StoreError> {
    let from = match queries.and_then(|queries| queries.get("offset")) {
        Some(value) => parse_offset(value)?,
        None => ReadFrom::Start,
    };
    // `offset=now` must never be stored: the tail offset changes as data arrives.
    let from_tail = matches!(from, ReadFrom::Tail);

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

    let body = render_page_body(&head.content_type, &page);
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
    } else {
        response_headers.push((
            SimpleHeader::custom(header::STREAM_CURSOR),
            cursor_value(ctx, client_cursor(queries)).to_string(),
        ));
    }
    response_headers.push((SimpleHeader::ETAG, tag));
    response_headers.push((
        SimpleHeader::CACHE_CONTROL,
        if from_tail {
            CachePolicy::NoStore
        } else {
            CachePolicy::Public
        }
        .header_value()
        .to_string(),
    ));

    Ok(DurableResponse {
        status: 200,
        headers: response_headers,
        body,
    })
}

async fn handle_long_poll(
    service: &DurableService<DurableStream>,
    ctx: &Ctx,
    name: &StreamName,
    queries: Option<&BTreeMap<String, String>>,
) -> Result<DurableResponse, StoreError> {
    let Some(value) = queries.and_then(|queries| queries.get("offset")) else {
        return Err(StoreError::BadRequest(
            foundation_core::durable::BadRequestKind::Offset,
        ));
    };
    let from = parse_offset(value)?;
    let client = client_cursor(queries);
    let content_type = service.head(name).await?.content_type;

    let signal = append_wait(service, name);
    let mut page = service
        .read(
            name,
            ReadRequest {
                from,
                limit: ReadLimit::default(),
            },
        )
        .await?;

    if page.chunks.is_empty() && !page.cursor.closed {
        let timeout = Duration::from_millis(service.limits().long_poll_timeout_ms);
        wait_or_timeout(signal, timeout, ctx).await;
        page = service
            .read(
                name,
                ReadRequest {
                    from: ReadFrom::Offset(page.cursor.next.clone()),
                    limit: ReadLimit::default(),
                },
            )
            .await?;
    }

    Ok(live_response(ctx, &content_type, &page, client))
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
    let content_type = header_value(headers, "content-type")
        .map(|value| ContentType::parse(&value))
        .transpose()?;
    let closed = parse_stream_closed(header_value(headers, "stream-closed").as_deref());
    let ttl = header_value(headers, "stream-ttl")
        .map(|value| parse_ttl(&value))
        .transpose()?;
    let expires_at = header_value(headers, "stream-expires-at")
        .map(|value| parse_expires_at(&value))
        .transpose()?;
    let expires = Expiry::from_headers(ttl, expires_at)?;

    let fork_from = header_value(headers, "stream-forked-from");
    let fork_sub = header_value(headers, "stream-fork-sub-offset");
    if fork_sub.is_some() && fork_from.is_none() {
        return Err(StoreError::BadRequest(
            foundation_core::durable::BadRequestKind::Fork,
        ));
    }
    let fork = fork_from
        .map(|source| -> Result<ForkSpec, StoreError> {
            let source = StreamName::parse(source.trim_start_matches('/'))?;
            let at = header_value(headers, "stream-fork-offset")
                .map(|value| decode_offset(&value))
                .transpose()?;
            let sub_offset = match fork_sub {
                Some(ref value) => parse_sub_offset(value)?,
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
        content_type,
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

    let close_only = body.is_empty();
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
        // A non-empty append must name its content type.
        let Some(value) = header_value(headers, "content-type") else {
            return Err(StoreError::BadRequest(
                foundation_core::durable::BadRequestKind::Body,
            ));
        };
        let content_type = ContentType::parse(&value)?;
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

    // A plain append and a close-only request answer 204. A producer's first
    // accepted write answers 200; a duplicate answers 204.
    let status = if close_only
        || producer.is_none()
        || matches!(outcome.kind, foundation_core::durable::AppendKind::Duplicate)
    {
        204
    } else {
        200
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

/// The wire text of one Server-Sent Events (SSE) `data` event.
///
/// A payload with newlines becomes one `data:` line per piece, as the SSE
/// format requires.
#[must_use]
pub fn sse_data_event(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 16);
    out.extend_from_slice(b"event: data\n");
    for line in sse_lines(payload) {
        out.extend_from_slice(b"data:");
        out.extend_from_slice(line);
        out.push(b'\n');
    }
    out.push(b'\n');
    out
}

/// Split a payload into SSE data lines.
///
/// Every carriage return and line feed ends a line, so a payload cannot break
/// the event framing. An empty payload is one empty line.
fn sse_lines(payload: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < payload.len() {
        match payload[index] {
            b'\r' | b'\n' => {
                lines.push(&payload[start..index]);
                if payload[index] == b'\r' && payload.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    if start < payload.len() || lines.is_empty() {
        lines.push(&payload[start..]);
    }
    lines
}

/// The wire text of one SSE `control` event.
#[must_use]
pub fn sse_control_event(json: &str) -> Vec<u8> {
    format!("event: control\ndata:{json}\n\n").into_bytes()
}

/// The control payload for one SSE step.
#[must_use]
pub fn sse_control_json(
    next: &StreamOffset,
    cursor: Option<u64>,
    up_to_date: bool,
    closed: bool,
) -> String {
    let mut object = serde_json::Map::new();
    object.insert(
        "streamNextOffset".to_string(),
        serde_json::Value::String(offset_text(next)),
    );
    if let Some(cursor) = cursor {
        object.insert("streamCursor".to_string(), cursor.into());
    }
    if up_to_date {
        object.insert("upToDate".to_string(), true.into());
    }
    if closed {
        object.insert("streamClosed".to_string(), true.into());
    }
    serde_json::Value::Object(object).to_string()
}

/// Render a response head with no content length, so the body runs to close.
fn streaming_head(headers: &[(SimpleHeader, String)]) -> Result<Vec<u8>, String> {
    let mut builder = SimpleOutgoingResponse::builder().with_status(Status::OK);
    for (name, value) in headers {
        builder = builder.add_header(name.clone(), value.clone());
    }
    let response = builder
        .with_body(SendSafeBody::SseStream(None))
        .build()
        .map_err(|error| error.to_string())?;
    render_message(response)
}

async fn serve_sse(
    service: &DurableService<DurableStream>,
    ctx: &Ctx,
    name: &StreamName,
    queries: Option<&BTreeMap<String, String>>,
    conn: SharedByteBufferStream<RawStream>,
) -> ServeOutcome {
    let Some(value) = queries.and_then(|queries| queries.get("offset")) else {
        let error = StoreError::BadRequest(foundation_core::durable::BadRequestKind::Offset);
        return respond_error(conn, ctx, &error).await;
    };
    let mut next = match parse_offset(value) {
        Ok(from) => from,
        Err(error) => return respond_error(conn, ctx, &error).await,
    };
    let head = match service.head(name).await {
        Ok(head) => head,
        Err(error) => return respond_error(conn, ctx, &error).await,
    };
    let binary = !(head.content_type.is_json()
        || head.content_type.as_str().starts_with("text/"));
    let client = client_cursor(queries);

    let mut headers = base_headers();
    headers.push((
        SimpleHeader::CONTENT_TYPE,
        "text/event-stream".to_string(),
    ));
    headers.push((
        SimpleHeader::CACHE_CONTROL,
        CachePolicy::NoCache.header_value().to_string(),
    ));
    headers.push((SimpleHeader::CONNECTION, "close".to_string()));
    headers.push((
        SimpleHeader::custom("x-accel-buffering"),
        "no".to_string(),
    ));
    if binary {
        headers.push((
            SimpleHeader::custom(header::STREAM_SSE_DATA_ENCODING),
            "base64".to_string(),
        ));
    }
    let head_bytes = match streaming_head(&headers) {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::error!("durable sse head build failed: {error}");
            return ServeOutcome::Close(None);
        }
    };

    let mut writer = ResponseWriter::new(conn, WriterConfig::default(), ctx.clone());
    if let Err(error) = writer.write_all(&head_bytes).await {
        tracing::debug!("durable sse head write ended: {error}");
        return ServeOutcome::Close(None);
    }

    let deadline =
        ctx.clock().now() + Duration::from_millis(service.limits().sse_lifetime_ms);
    let mut first = true;
    loop {
        let signal = append_wait(service, name);
        let page = match service
            .read(
                name,
                ReadRequest {
                    from: next.clone(),
                    limit: ReadLimit::default(),
                },
            )
            .await
        {
            Ok(page) => page,
            Err(error) => {
                tracing::debug!("durable sse read ended: {error}");
                return ServeOutcome::Close(None);
            }
        };

        let sent_data = !page.chunks.is_empty();
        if sent_data {
            let body = render_page_body(&head.content_type, &page);
            let payload = if binary {
                base64::engine::general_purpose::STANDARD
                    .encode(body)
                    .into_bytes()
            } else {
                body
            };
            if let Err(error) = writer.write_all(&sse_data_event(&payload)).await {
                tracing::debug!("durable sse data write ended: {error}");
                return ServeOutcome::Close(None);
            }
        }

        let closed = page.cursor.closed;
        // A control event follows every data event. An idle open stream sends
        // one on connect, so a client learns the tail offset at once.
        let send_control = sent_data || closed || first;
        first = false;
        if send_control {
            let cursor = if closed {
                None
            } else {
                Some(cursor_value(ctx, client))
            };
            let control = sse_control_json(
                &page.cursor.next,
                cursor,
                page.cursor.up_to_date || closed,
                closed,
            );
            if let Err(error) = writer.write_all(&sse_control_event(&control)).await {
                tracing::debug!("durable sse control write ended: {error}");
                return ServeOutcome::Close(None);
            }
            if let Err(error) = writer.flush().await {
                tracing::debug!("durable sse flush ended: {error}");
                return ServeOutcome::Close(None);
            }
        }

        if closed {
            return ServeOutcome::Close(None);
        }

        next = ReadFrom::Offset(page.cursor.next.clone());
        let now = ctx.clock().now();
        if now >= deadline {
            return ServeOutcome::Close(None);
        }
        if !sent_data {
            wait_or_timeout(signal, deadline.saturating_duration_since(now), ctx).await;
        }
    }
}
