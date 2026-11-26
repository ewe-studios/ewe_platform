//! The valtron layer for Durable Streams.
//!
//! It wraps the protocol core in [`foundation_core::durable`]. The core stays free of valtron.
//! This layer adds `StreamIterator`, `Stream::Next`, `Stream::Spread`, `Stream::Wait` and
//! `WakeableStream` semantics. Feature 02 starts with catch-up reads. The live tail and wake
//! wiring land in the same feature.

use crate::durable::{
    AppendKind, AppendOutcome, AppendRequest, Chunk, DurableStream, DurableStreamStore, Producer,
    ProducerId, ReadFrom, ReadLimit, ReadRequest, ResumeCursor, StoreError, StreamName,
    StreamOffset,
};
use crate::valtron::{
    drive_iterator, task, Ctx, DrivenTaskIterator, Stream, TaskSpread, TaskStatus, WakeCallback,
    WakeResult, WakeableStream,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The pending context a valtron reader reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaderProgress {
    /// The reader is opening the stream.
    Connecting,
    /// The reader is refusing a trimmed offset.
    Trimming,
    /// The reader is parked at the live tail.
    Live,
}

/// The state of one catch-up reader task.
#[derive(Clone)]
struct ReaderState {
    stream: DurableStream,
    name: StreamName,
    first: bool,
    from: ReadFrom,
    cursor: ResumeCursor,
    live: bool,
}

#[allow(clippy::needless_pass_by_value)] // The task macro takes the state by value.
#[task]
async fn read_step(
    ctx: Ctx,
    state: ReaderState,
) -> Option<(ReaderState, TaskStatus<Chunk, ReaderProgress>)> {
    let _ = ctx;
    let from = if state.first {
        state.from.clone()
    } else {
        ReadFrom::Offset(state.cursor.next.clone())
    };

    let page = state
        .stream
        .read(
            &state.name,
            ReadRequest {
                from,
                limit: ReadLimit::default(),
            },
        )
        .await
        .ok()?;

    let mut next = state.clone();
    next.first = false;
    next.cursor = page.cursor.clone();

    if !page.chunks.is_empty() {
        let items = page.chunks.into_iter().map(TaskSpread::Ready).collect();
        return Some((next, TaskStatus::Spread(items)));
    }

    if page.cursor.closed {
        return None;
    }

    if page.cursor.up_to_date && !state.live {
        return None;
    }

    Some((next, TaskStatus::Wait))
}

/// A valtron `StreamIterator` over the protocol core.
///
/// Feature 02 starts with catch-up. The live tail, `Stream::Wait` and wake forwarding land in
/// the same feature before it closes.
pub struct DurableStreamReader {
    inner: DrivenTaskIterator<read_step>,
    stream: DurableStream,
    name: StreamName,
}

impl DurableStreamReader {
    /// Build a reader for a stream.
    #[must_use]
    pub fn new(
        stream: DurableStream,
        name: StreamName,
        from: ReadFrom,
        live: bool,
        ctx: Ctx,
    ) -> Self {
        let state = ReaderState {
            stream: stream.clone(),
            name: name.clone(),
            first: true,
            cursor: ResumeCursor {
                next: StreamOffset::from_bytes(Vec::new()),
                up_to_date: false,
                closed: false,
            },
            from,
            live,
        };
        Self {
            inner: drive_iterator(read_step::new(state), ctx),
            stream,
            name,
        }
    }
}

impl Iterator for DurableStreamReader {
    type Item = Stream<Chunk, ReaderProgress>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(Stream::from)
    }
}

/// The pending context a producer task reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProducerProgress {
    /// The producer is sending a batch.
    Sending,
    /// The producer was fenced and stopped.
    Fenced,
}

/// A producer over the core append ladder.
#[derive(Clone)]
pub struct DurableStreamWriter {
    stream: DurableStream,
    name: StreamName,
    producer: ProducerId,
    epoch: u64,
    next_seq: Arc<AtomicU64>,
}

impl DurableStreamWriter {
    /// Build a producer at `epoch`.
    #[must_use]
    pub fn new(stream: DurableStream, name: StreamName, producer: ProducerId, epoch: u64) -> Self {
        Self {
            stream,
            name,
            producer,
            epoch,
            next_seq: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Return the next sequence number.
    #[must_use]
    pub fn next_seq(&self) -> u64 {
        self.next_seq.load(Ordering::SeqCst)
    }

    /// Append one chunk through the producer.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the core producer ladder.
    pub async fn append(&self, chunk: Chunk) -> Result<AppendOutcome, StoreError> {
        let seq = self.next_seq.load(Ordering::SeqCst);
        let outcome = self
            .stream
            .append(
                &self.name,
                AppendRequest {
                    chunk,
                    content_type: None,
                    stream_seq: None,
                    producer: Some(Producer {
                        id: self.producer.clone(),
                        epoch: self.epoch,
                        seq,
                    }),
                    close: false,
                },
            )
            .await?;

        if matches!(outcome.kind, AppendKind::Created | AppendKind::Duplicate) {
            let _ = self.next_seq.fetch_add(1, Ordering::SeqCst);
        }
        Ok(outcome)
    }
}

#[derive(Clone)]
struct ProducerState {
    writer: DurableStreamWriter,
    queue: Arc<Mutex<VecDeque<Chunk>>>,
    fenced: Arc<AtomicBool>,
}

#[allow(clippy::needless_pass_by_value)] // The task macro takes the state by value.
#[task]
async fn producer_step(
    ctx: Ctx,
    state: ProducerState,
) -> Option<(
    ProducerState,
    TaskStatus<Result<AppendOutcome, StoreError>, ProducerProgress>,
)> {
    let _ = ctx;
    if state.fenced.load(Ordering::SeqCst) {
        return None;
    }

    let chunk = {
        state
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()
    };
    let chunk = chunk?;

    let result = state.writer.append(chunk).await;
    if matches!(&result, Err(StoreError::Fenced { .. })) {
        state.fenced.store(true, Ordering::SeqCst);
    }

    let next = state.clone();
    Some((next, TaskStatus::Ready(result)))
}

/// Build a producer task over a fixed batch of chunks.
///
/// # Errors
///
/// Returns an error when the valtron engine is not live.
pub fn producer_task(
    writer: DurableStreamWriter,
    chunks: Vec<Chunk>,
) -> impl Iterator<Item = TaskStatus<Result<AppendOutcome, StoreError>, ProducerProgress>> {
    let state = ProducerState {
        writer,
        queue: Arc::new(Mutex::new(chunks.into())),
        fenced: Arc::new(AtomicBool::new(false)),
    };
    drive_iterator(producer_step::new(state), Ctx::host())
}

impl WakeableStream for DurableStreamReader {
    fn wake(&mut self, cb: WakeCallback) -> WakeResult {
        #[cfg(feature = "multi")]
        {
            if let Some(notifier) = self.stream.notifier(&self.name) {
                notifier.register(cb);
                return WakeResult::Successful;
            }
        }
        #[cfg(not(feature = "multi"))]
        {
            let _ = self.stream.notifier(&self.name);
            let _ = cb;
        }
        WakeResult::NotSupported
    }
}
