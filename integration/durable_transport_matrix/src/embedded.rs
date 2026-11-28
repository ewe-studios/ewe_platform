//! The embedded driver.
//!
//! The driver calls [`DurableService`] over a [`DurableStream`] on a
//! [`MemoryVfs`] in the test process. It uses no socket and no transport
//! encoding, so it is the reference for the protocol outcome.

use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::durable::{
    wire, AppendKind, AppendOutcome, AppendRequest, BadRequestKind, Chunk, CloseRequest,
    ContentType, CreateOutcome, CreateSpec, DurableService, DurableStream, Expiry, ForkSpec,
    MemoryVfs, Producer as StoreProducer, ProducerId, ReadFrom, ReadLimit, ReadPage, ReadRequest,
    StoreError, StreamName, StreamOffset, StreamSeq,
};
use foundation_core::valtron::timer;

use crate::driver::{Driver, ErrorKind, Observation, Producer, StepResult};

/// The Durable Streams protocol service over an in-memory VFS.
pub struct EmbeddedDriver {
    service: DurableService<DurableStream>,
    ctx: Ctx,
}

impl EmbeddedDriver {
    /// Build a driver over a fresh in-memory store.
    #[must_use]
    pub fn new(ctx: Ctx) -> Self {
        let stream = DurableStream::new(Arc::new(MemoryVfs::new()), &ctx);
        Self {
            service: DurableService::new(stream),
            ctx,
        }
    }

    /// Create a stream.
    ///
    /// # Errors
    /// Returns the store refusal.
    async fn create_result(
        &self,
        stream: &str,
        content_type: &str,
        ttl_ms: Option<u64>,
    ) -> Result<StepResult, StoreError> {
        let name = StreamName::parse(stream)?;
        let content_type = ContentType::parse(content_type)?;
        let outcome = self
            .service
            .create(
                name,
                CreateSpec {
                    content_type: Some(content_type),
                    expires: ttl_ms.map(|value| Expiry::Ttl(Duration::from_millis(value))),
                    closed: false,
                    initial: None,
                    fork: None,
                },
            )
            .await?;
        Ok(create_step(&outcome))
    }

    /// Append one chunk.
    ///
    /// # Errors
    /// Returns the store refusal.
    async fn append_result(
        &self,
        stream: &str,
        data: &[u8],
        content_type: &str,
        producer: Option<&Producer>,
        stream_seq: Option<&str>,
        close: bool,
    ) -> Result<StepResult, StoreError> {
        let name = StreamName::parse(stream)?;
        let content_type = ContentType::parse(content_type)?;
        let stream_seq = stream_seq.map(|value| StreamSeq::from_bytes(value.as_bytes().to_vec()));
        let producer = producer
            .map(|value| {
                Ok::<_, StoreError>(StoreProducer {
                    id: ProducerId::parse(&value.id)?,
                    epoch: value.epoch,
                    seq: value.seq,
                })
            })
            .transpose()?;
        let outcome = self
            .service
            .append(
                &name,
                AppendRequest {
                    chunk: Chunk::new(data.to_vec()),
                    content_type: Some(content_type),
                    stream_seq,
                    producer,
                    close,
                },
            )
            .await?;
        Ok(append_step(&outcome))
    }

    /// Close a stream.
    ///
    /// # Errors
    /// Returns the store refusal.
    async fn close_result(&self, stream: &str, data: &[u8]) -> Result<StepResult, StoreError> {
        let name = StreamName::parse(stream)?;
        let outcome = self
            .service
            .close(
                &name,
                CloseRequest {
                    final_chunk: (!data.is_empty()).then(|| Chunk::new(data.to_vec())),
                    producer: None,
                },
            )
            .await?;
        Ok(append_step(&outcome))
    }

    /// Read a bounded page.
    ///
    /// # Errors
    /// Returns the store refusal.
    async fn read_result(
        &self,
        stream: &str,
        offset: Option<&str>,
    ) -> Result<StepResult, StoreError> {
        let name = StreamName::parse(stream)?;
        let from = read_from(offset)?;
        let page = self
            .service
            .read(
                &name,
                ReadRequest {
                    from,
                    limit: ReadLimit::new(None, None),
                },
            )
            .await?;
        Ok(read_step(&page))
    }

    /// Create a fork.
    ///
    /// # Errors
    /// Returns the store refusal.
    async fn fork_result(
        &self,
        child: &str,
        source: &str,
        at: Option<&str>,
    ) -> Result<StepResult, StoreError> {
        let name = StreamName::parse(child)?;
        let source = StreamName::parse(source)?;
        let at = at.map(parse_offset).transpose()?;
        let outcome = self
            .service
            .fork(
                name,
                ForkSpec {
                    source,
                    at,
                    sub_offset: 0,
                },
            )
            .await?;
        Ok(create_step(&outcome))
    }
}

/// Parse a read start. `None` starts at the beginning.
fn read_from(offset: Option<&str>) -> Result<ReadFrom, StoreError> {
    match offset {
        None => Ok(ReadFrom::Start),
        Some(value) => Ok(ReadFrom::Offset(parse_offset(value)?)),
    }
}

/// Parse one wire offset.
fn parse_offset(text: &str) -> Result<StreamOffset, StoreError> {
    wire::parse_offset(text).ok_or(StoreError::BadRequest(BadRequestKind::Offset))
}

/// Build the observation of a create or fork.
fn create_step(outcome: &CreateOutcome) -> StepResult {
    let offset = wire::offset_text(&outcome.head.tail);
    let observation = Observation::status(if outcome.created { 201 } else { 200 })
        .with_next_offset(true)
        .with_closed(outcome.head.closed);
    StepResult::new(observation, Some(offset))
}

/// Build the observation of an append or close.
fn append_step(outcome: &AppendOutcome) -> StepResult {
    let offset = wire::offset_text(&outcome.cursor.next);
    let observation = Observation::status(204)
        .with_next_offset(true)
        .with_closed(outcome.closed)
        .with_duplicate(matches!(outcome.kind, AppendKind::Duplicate));
    StepResult::new(observation, Some(offset))
}

/// Build the observation of a read page.
fn read_step(page: &ReadPage) -> StepResult {
    let body: Vec<u8> = page
        .chunks
        .iter()
        .flat_map(|chunk| chunk.as_bytes().iter().copied())
        .collect();
    let offset = wire::offset_text(&page.cursor.next);
    let observation = Observation::status(200)
        .with_body(body)
        .with_next_offset(true)
        .with_up_to_date(page.cursor.up_to_date)
        .with_closed(page.cursor.closed);
    StepResult::new(observation, Some(offset))
}

/// Map one store refusal to a transport-neutral kind.
fn error_step(error: StoreError) -> StepResult {
    let kind = match error {
        StoreError::NotFound => ErrorKind::NotFound,
        StoreError::Gone => ErrorKind::Gone,
        StoreError::Closed { .. } => ErrorKind::Closed,
        StoreError::StreamSeqConflict { .. }
        | StoreError::ContentTypeMismatch { .. }
        | StoreError::ConfigConflict => ErrorKind::Conflict,
        StoreError::BadRequest(_) => ErrorKind::BadRequest,
        StoreError::Unsupported => ErrorKind::Unsupported,
        other => ErrorKind::Other(other.to_string()),
    };
    StepResult::new(Observation::error(kind), None)
}

impl Driver for EmbeddedDriver {
    async fn create(
        &mut self,
        stream: &str,
        content_type: &str,
        ttl_ms: Option<u64>,
    ) -> StepResult {
        match self.create_result(stream, content_type, ttl_ms).await {
            Ok(step) => step,
            Err(error) => error_step(error),
        }
    }

    async fn append(
        &mut self,
        stream: &str,
        data: &[u8],
        content_type: &str,
        producer: Option<&Producer>,
        stream_seq: Option<&str>,
        close: bool,
    ) -> StepResult {
        match self
            .append_result(stream, data, content_type, producer, stream_seq, close)
            .await
        {
            Ok(step) => step,
            Err(error) => error_step(error),
        }
    }

    async fn close(&mut self, stream: &str, data: &[u8], _content_type: &str) -> StepResult {
        match self.close_result(stream, data).await {
            Ok(step) => step,
            Err(error) => error_step(error),
        }
    }

    async fn read(&mut self, stream: &str, offset: Option<&str>) -> StepResult {
        match self.read_result(stream, offset).await {
            Ok(step) => step,
            Err(error) => error_step(error),
        }
    }

    async fn fork(&mut self, child: &str, source: &str, at: Option<&str>) -> StepResult {
        match self.fork_result(child, source, at).await {
            Ok(step) => step,
            Err(error) => error_step(error),
        }
    }

    async fn wait(&mut self, duration: Duration) {
        timer::sleep_for(duration, &self.ctx).await;
    }
}
