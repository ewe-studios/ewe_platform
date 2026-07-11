//! Type-erased handlers + the object-safe codec-metadata view (Decision 08
//! §Type-Erased Handler Traits).
//!
//! WHY: The router stores handlers of many `Req`/`Res` types in one map, so it
//! must hold them **generic-free**. Unary erases to a bytes function
//! `(Ctx, codec name, Bytes) -> (Bytes, headers, trailers)`; the three streaming
//! kinds erase to the byte-level [`HandlerConn`] seam (Decision 11). The concrete
//! `ProcedureCodecs<Req, Res>` table (Decision 02) lives **inside** each wrapper's
//! closures — it resolves the typed `CodecFor` pair from the per-request codec
//! *name* before building the returned future, so nothing borrowed is captured.
//!
//! WHAT: the [`ProcedureMeta`] dispatch view, the [`ErasedUnaryHandler`] /
//! [`ErasedStreamHandler`] traits + their concrete wrappers, and the `*_handler`
//! builders that turn a Decision 04 async fn/`Stream` into the innermost
//! [`UnaryFunc`]/[`StreamingHandlerFunc`] (the interceptor chain is composed
//! around these once at registration).
//!
//! HOW: streaming drives the typed [`MessageSource`]/[`MessageSink`] facades over
//! the split conn halves; request streams are handed to the handler as a boxed
//! [`RequestStream`] (an [`unfold`](futures::stream::unfold) over the source).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use bytes::Bytes;
use futures::stream::{Stream, StreamExt};
use foundation_netio::shared::http::SimpleHeaders;

use crate::shared::codec::ProcedureCodecs;
use crate::shared::context::Ctx;
use crate::shared::error::{ConnectError, ConnectResult};
use crate::shared::interceptor::{StreamCall, StreamingHandlerFunc, UnaryCall, UnaryFunc, UnaryReply};
use crate::shared::message::{Request, Response};
use crate::shared::transport::{BoxFuture, HandlerConn, MessageSink, MessageSource};

/// A boxed stream of decoded requests handed to a client-/bidi-streaming handler.
/// Boxed so the registration signature can name the input type (our internal
/// `unfold` type is unnameable); the generated `impl Stream` handler accepts it.
pub type RequestStream<Req> = Pin<Box<dyn Stream<Item = ConnectResult<Req>> + Send>>;

/// Object-safe metadata surface over a procedure's [`ProcedureCodecs`] table
/// (Decision 08). Dispatch-time only — typed resolution never happens here.
pub(crate) trait ProcedureMeta: Send + Sync {
    /// Whether `name` is a registered codec (the 415 membership gate).
    fn has_codec(&self, name: &str) -> bool;
    /// Registered wire names in order (Accept-Post / error messages).
    fn codec_names(&self) -> Vec<String>;
    /// Whether the codec is binary (GET query encoding, Decision 07), or `None`
    /// if it is not registered. Consumed by the client GET-encoding path (F24) /
    /// codegen; part of the D08-normative surface.
    #[allow(dead_code)]
    fn is_binary(&self, name: &str) -> Option<bool>;
}

/// The erased codec-metadata view stored on a [`HandlerEntry`](super::HandlerEntry).
/// Shares its `Arc<ProcedureCodecs>` with the erased handler's closures.
pub(crate) struct CodecMeta<Req, Res> {
    codecs: Arc<ProcedureCodecs<Req, Res>>,
}

impl<Req: 'static, Res: 'static> ProcedureMeta for CodecMeta<Req, Res> {
    fn has_codec(&self, name: &str) -> bool {
        self.codecs.contains(name)
    }
    fn codec_names(&self) -> Vec<String> {
        self.codecs.names().map(str::to_string).collect()
    }
    fn is_binary(&self, name: &str) -> Option<bool> {
        self.codecs.is_binary(name)
    }
}

/// Erased async unary handler (Decision 08). `handle` invokes the pre-composed
/// interceptor chain; the innermost func decodes → awaits the handler → encodes.
pub(crate) trait ErasedUnaryHandler: Send + Sync {
    fn handle(
        &self,
        ctx: Ctx,
        codec_name: &str,
        body: Bytes,
    ) -> BoxFuture<'static, ConnectResult<(Bytes, SimpleHeaders, SimpleHeaders)>>;
}

/// Erased async streaming handler (server / client / bidi share the byte-level
/// [`HandlerConn`] seam; the `Spec`'s `StreamType` distinguishes them).
pub(crate) trait ErasedStreamHandler: Send + Sync {
    fn handle(
        &self,
        ctx: Ctx,
        codec_name: &str,
        conn: Box<dyn HandlerConn>,
    ) -> BoxFuture<'static, ConnectResult<()>>;
}

/// Concrete unary wrapper holding the composed chain (innermost = decode/handle/
/// encode). Generic-free — the codec table lives in the closure it was built from.
pub(crate) struct UnaryHandlerImpl {
    func: UnaryFunc,
}

impl ErasedUnaryHandler for UnaryHandlerImpl {
    fn handle(
        &self,
        ctx: Ctx,
        codec_name: &str,
        body: Bytes,
    ) -> BoxFuture<'static, ConnectResult<(Bytes, SimpleHeaders, SimpleHeaders)>> {
        let call = UnaryCall {
            headers: (*ctx.request.headers).clone(),
            codec_name: codec_name.to_string(),
            frame: body,
        };
        let fut = (self.func)(ctx, call);
        Box::pin(async move {
            let reply = fut.await?;
            Ok((reply.frame, reply.headers, reply.trailers))
        })
    }
}

impl UnaryHandlerImpl {
    pub(crate) fn new(func: UnaryFunc) -> Self {
        Self { func }
    }
}

/// Concrete streaming wrapper holding the composed chain.
pub(crate) struct StreamHandlerImpl {
    func: StreamingHandlerFunc,
}

impl ErasedStreamHandler for StreamHandlerImpl {
    fn handle(
        &self,
        ctx: Ctx,
        codec_name: &str,
        conn: Box<dyn HandlerConn>,
    ) -> BoxFuture<'static, ConnectResult<()>> {
        (self.func)(
            ctx,
            StreamCall {
                codec_name: codec_name.to_string(),
                conn,
            },
        )
    }
}

impl StreamHandlerImpl {
    pub(crate) fn new(func: StreamingHandlerFunc) -> Self {
        Self { func }
    }
}

// ── innermost func builders (Decision 04 async shapes → erased funcs) ──────────

/// Build the innermost unary func: resolve the codec pair from the request name,
/// decode → await the handler → encode.
pub(crate) fn unary_handler<Req, Res, F, Fut>(
    codecs: Arc<ProcedureCodecs<Req, Res>>,
    handler: F,
) -> UnaryFunc
where
    Req: Send + 'static,
    Res: Send + 'static,
    F: Fn(Ctx, Request<Req>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ConnectResult<Response<Res>>> + Send + 'static,
{
    let handler = Arc::new(handler);
    Arc::new(move |ctx: Ctx, call: UnaryCall| {
        let codecs = codecs.clone();
        let handler = handler.clone();
        Box::pin(async move {
            let req_codec = codecs.for_request(&call.codec_name)?;
            let res_codec = codecs.for_response(&call.codec_name)?;
            let msg = req_codec.unmarshal(call.frame)?;
            let request =
                Request::with_parts(msg, ctx.spec().clone(), ctx.peer().clone(), call.headers);
            let (out, headers, trailers) = handler(ctx.clone(), request).await?.into_parts();
            let frame = res_codec.marshal(&out)?;
            Ok(UnaryReply {
                headers,
                trailers,
                frame,
            })
        })
    })
}

/// Build the innermost server-streaming func: read the single request, run the
/// handler, drain its response `Stream` to the sink.
pub(crate) fn server_stream_handler<Req, Res, S, F, Fut>(
    codecs: Arc<ProcedureCodecs<Req, Res>>,
    handler: F,
) -> StreamingHandlerFunc
where
    Req: Send + 'static,
    Res: Send + 'static,
    S: Stream<Item = ConnectResult<Res>> + Send + 'static,
    F: Fn(Ctx, Request<Req>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ConnectResult<S>> + Send + 'static,
{
    let handler = Arc::new(handler);
    Arc::new(move |ctx: Ctx, call: StreamCall| {
        let codecs = codecs.clone();
        let handler = handler.clone();
        Box::pin(async move {
            let req_codec = codecs.for_request(&call.codec_name)?;
            let res_codec = codecs.for_response(&call.codec_name)?;
            let headers = call.conn.request_headers().clone();
            let (receiver, sender) = call.conn.split();

            let mut source = MessageSource::new(receiver, req_codec, headers.clone());
            let req = source.receive().await?.ok_or_else(|| {
                ConnectError::invalid_argument(
                    "server-stream requires exactly one request message",
                )
            })?;
            let request =
                Request::with_parts(req, ctx.spec().clone(), ctx.peer().clone(), headers);

            let stream = handler(ctx.clone(), request).await?;
            let mut sink = MessageSink::new(sender, res_codec);
            let mut stream = Box::pin(stream);
            while let Some(item) = stream.next().await {
                match item {
                    Ok(msg) => sink.send(msg).await?,
                    Err(e) => return sink.close_with_error(e, SimpleHeaders::new()).await,
                }
            }
            sink.close(SimpleHeaders::new()).await
        })
    })
}

/// Build the innermost client-streaming func: hand the request `Stream` to the
/// handler, encode its single response.
pub(crate) fn client_stream_handler<Req, Res, F, Fut>(
    codecs: Arc<ProcedureCodecs<Req, Res>>,
    handler: F,
) -> StreamingHandlerFunc
where
    Req: Send + 'static,
    Res: Send + 'static,
    F: Fn(Ctx, RequestStream<Req>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ConnectResult<Response<Res>>> + Send + 'static,
{
    let handler = Arc::new(handler);
    Arc::new(move |ctx: Ctx, call: StreamCall| {
        let codecs = codecs.clone();
        let handler = handler.clone();
        Box::pin(async move {
            let req_codec = codecs.for_request(&call.codec_name)?;
            let res_codec = codecs.for_response(&call.codec_name)?;
            let headers = call.conn.request_headers().clone();
            let (receiver, sender) = call.conn.split();

            let source = MessageSource::new(receiver, req_codec, headers);
            let req_stream = source_into_stream(source);
            let (out, resp_headers, trailers) =
                handler(ctx.clone(), req_stream).await?.into_parts();

            let mut sink = MessageSink::new(sender, res_codec);
            sink.set_headers(resp_headers).await?;
            sink.send(out).await?;
            sink.close(trailers).await
        })
    })
}

/// Build the innermost bidi-streaming func: hand the request `Stream` in, drain
/// the response `Stream` out — interleaved by the executor over the split halves.
pub(crate) fn bidi_stream_handler<Req, Res, S, F, Fut>(
    codecs: Arc<ProcedureCodecs<Req, Res>>,
    handler: F,
) -> StreamingHandlerFunc
where
    Req: Send + 'static,
    Res: Send + 'static,
    S: Stream<Item = ConnectResult<Res>> + Send + 'static,
    F: Fn(Ctx, RequestStream<Req>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ConnectResult<S>> + Send + 'static,
{
    let handler = Arc::new(handler);
    Arc::new(move |ctx: Ctx, call: StreamCall| {
        let codecs = codecs.clone();
        let handler = handler.clone();
        Box::pin(async move {
            let req_codec = codecs.for_request(&call.codec_name)?;
            let res_codec = codecs.for_response(&call.codec_name)?;
            let headers = call.conn.request_headers().clone();
            let (receiver, sender) = call.conn.split();

            let source = MessageSource::new(receiver, req_codec, headers);
            let req_stream = source_into_stream(source);
            let out_stream = handler(ctx.clone(), req_stream).await?;

            let mut sink = MessageSink::new(sender, res_codec);
            let mut out_stream = Box::pin(out_stream);
            while let Some(item) = out_stream.next().await {
                match item {
                    Ok(msg) => sink.send(msg).await?,
                    Err(e) => return sink.close_with_error(e, SimpleHeaders::new()).await,
                }
            }
            sink.close(SimpleHeaders::new()).await
        })
    })
}

/// Adapt a [`MessageSource`] into the boxed [`RequestStream`] handed to a handler.
/// A terminal error is yielded once, then the stream ends.
fn source_into_stream<Req: Send + 'static>(source: MessageSource<Req>) -> RequestStream<Req> {
    Box::pin(futures::stream::unfold(
        (source, false),
        |(mut source, err_done)| async move {
            if err_done {
                return None;
            }
            match source.receive().await {
                Ok(Some(msg)) => Some((Ok(msg), (source, false))),
                Ok(None) => None,
                Err(e) => Some((Err(e), (source, true))),
            }
        },
    ))
}

/// Wrap a bare [`ProcedureMeta`] over the shared codec table.
pub(crate) fn codec_meta<Req: 'static, Res: 'static>(
    codecs: Arc<ProcedureCodecs<Req, Res>>,
) -> Arc<dyn ProcedureMeta> {
    Arc::new(CodecMeta { codecs })
}
