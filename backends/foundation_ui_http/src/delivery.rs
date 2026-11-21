//! Connect a synchronous serializer to the asynchronous HTTP writer.
use crate::{http, Response, ViewBody, ViewError, ViewRequest};
use http::native::writer::{render_message, send_text, ResponseWriter, WriterConfig};
use http::shared::serve::ServeError;
use http::{ConnectionResult, ServeOutcome, SimpleHeader};
use std::future::Future;
use std::io::{self, BufWriter, Write};
use std::num::NonZeroUsize;

/// Maximum bytes in each pending chunk. One chunk can wait for the socket.
#[derive(Clone, Copy)]
pub struct BodyWriterConfig {
    chunk_bytes: NonZeroUsize,
}

impl Default for BodyWriterConfig {
    fn default() -> Self {
        // Use the standard buffered writer size (8 KiB).
        Self {
            chunk_bytes: NonZeroUsize::new(8192).expect("nonzero size"),
        }
    }
}
impl BodyWriterConfig {
    #[must_use]
    pub fn with_chunk_bytes(mut self, size: NonZeroUsize) -> Self {
        self.chunk_bytes = size;
        self
    }
}

struct BodyPipe {
    output: async_channel::Sender<Vec<u8>>,
    chunk_bytes: usize,
}

impl Write for BodyPipe {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let count = bytes.len().min(self.chunk_bytes);
        self.output
            .send_blocking(bytes[..count].to_vec())
            .map_err(|_| io::ErrorKind::BrokenPipe)?;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn start_body(
    body: ViewBody,
    size: usize,
) -> (
    async_channel::Receiver<Vec<u8>>,
    blocking::Task<Result<(), ViewError>>,
) {
    let (output, input) = async_channel::bounded(1);
    let producer = blocking::unblock(move || {
        let mut pipe = BufWriter::with_capacity(
            size,
            BodyPipe {
                output,
                chunk_bytes: size,
            },
        );
        let result = body
            .write_body(&mut pipe)
            .and_then(|()| pipe.flush().map_err(ViewError::Write));
        // Discard unsent bytes on failure. A drop must never retry a failed write.
        drop(pipe.into_parts());
        result
    });
    (input, producer)
}

fn response_head(media_type: &str) -> Result<Vec<u8>, ViewError> {
    let response = http::SimpleOutgoingResponse::builder()
        .with_status(http::Status::OK)
        .add_header(SimpleHeader::CONTENT_TYPE, media_type.to_owned())
        .add_header(SimpleHeader::TRANSFER_ENCODING, "chunked")
        .with_body(http::SendSafeBody::None)
        .build()
        .map_err(|e| ViewError::Render(e.to_string()))?;
    render_message(response).map_err(ViewError::Render)
}

fn close(error: &ViewError) -> ServeOutcome {
    ServeOutcome::Close(Some(
        ServeError::InternalError {
            status: 500,
            reason: error.to_string(),
        }
        .into(),
    ))
}

pub(crate) async fn error_response(
    ctx: &http::Ctx,
    conn: http::SharedByteBufferStream<http::RawStream>,
    error: ViewError,
) -> ServeOutcome {
    let (status, message) = match error {
        ViewError::PayloadTooLarge => (413, "Action request exceeds the configured limit"),
        ViewError::BadRequest(_) => (400, "Invalid view request"),
        ViewError::UnsupportedVocabulary => (406, "Unsupported view vocabulary"),
        _ => {
            tracing::error!(%error, "view handler failed");
            (500, "Cannot render the view")
        }
    };
    match send_text(conn, status, message, ctx).split() {
        Ok(future) => future.await,
        Err(outcome) => outcome,
    }
}

pub(crate) async fn send_body(
    ctx: http::Ctx,
    conn: http::SharedByteBufferStream<http::RawStream>,
    body: ViewBody,
) -> ServeOutcome {
    let size = ctx
        .get_cloned::<BodyWriterConfig>()
        .unwrap_or_default()
        .chunk_bytes
        .get();
    let head = match response_head(body.response().media_type()) {
        Ok(head) => head,
        Err(error) => return error_response(&ctx, conn, error).await,
    };
    let (chunks, producer) = start_body(body, size);
    let mut next = chunks.recv().await.ok();
    let mut producer = Some(producer);
    if next.is_none() {
        if let Err(error) = producer.take().expect("producer available").await {
            return error_response(&ctx, conn, error).await;
        }
    }
    let config = ctx.get_cloned::<WriterConfig>().unwrap_or_default();
    let mut writer = ResponseWriter::new(conn, config, ctx);
    let result = async {
        writer.write_all(&head).await?;
        while let Some(bytes) = next.take() {
            writer
                .write_all(&http::Http11Chunk::Chunked(bytes).render())
                .await?;
            next = chunks.recv().await.ok();
        }
        if let Some(producer) = producer {
            producer.await?;
        }
        writer
            .write_all(&http::Http11Chunk::Chunked(Vec::new()).render())
            .await?;
        writer.flush().await?;
        Ok(())
    }
    .await;
    match result {
        Ok(()) => ServeOutcome::Keep,
        Err(error) => close(&error),
    }
}

impl From<io::Error> for ViewError {
    fn from(error: io::Error) -> Self {
        Self::Write(error)
    }
}

/// Runs a view function and writes its selected response on this connection.
#[must_use]
pub fn serve_view<F, Fut>(
    ctx: http::Ctx,
    req: http::SimpleIncomingRequest,
    conn: http::SharedByteBufferStream<http::RawStream>,
    handler: F,
) -> ConnectionResult
where
    F: FnOnce(ViewRequest) -> Fut + Send + 'static,
    Fut: Future<Output = Result<Response, ViewError>> + Send + 'static,
{
    ConnectionResult::Async(Box::pin(async move {
        let result = async {
            let request = ViewRequest::new(&ctx, req)?;
            let session = request.session();
            Ok(session.body(handler(request).await?))
        }
        .await;
        match result {
            Ok(body) => send_body(ctx, conn, body).await,
            Err(error) => error_response(&ctx, conn, error).await,
        }
    }))
}
