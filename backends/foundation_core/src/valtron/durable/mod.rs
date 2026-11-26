//! The valtron layer for Durable Streams.
//!
//! It wraps the protocol core in [`foundation_core::durable`]. The core stays free of valtron.
//! This layer adds `StreamIterator`, `Stream::Next`, `Stream::Spread`, `Stream::Wait` and
//! `WakeableStream` semantics. Feature 02 starts with catch-up reads. The live tail and wake
//! wiring land in the same feature.

use crate::durable::{
    Chunk, DurableStream, DurableStreamStore, ReadFrom, ReadLimit, ReadRequest, ResumeCursor,
    StreamName, StreamOffset,
};
use crate::valtron::{
    drive_iterator, task, Ctx, DrivenTaskIterator, Stream, TaskSpread, TaskStatus, WakeableStream,
};

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

    if page.cursor.closed || (page.cursor.up_to_date && !state.live) {
        return None;
    }

    // The live tail waits on the notifier in the next task. End here for now.
    None
}

/// A valtron `StreamIterator` over the protocol core.
///
/// Feature 02 starts with catch-up. The live tail, `Stream::Wait` and wake forwarding land in
/// the same feature before it closes.
pub struct DurableStreamReader {
    inner: DrivenTaskIterator<read_step>,
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
            stream,
            name,
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
        }
    }
}

impl Iterator for DurableStreamReader {
    type Item = Stream<Chunk, ReaderProgress>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(Stream::from)
    }
}

impl WakeableStream for DurableStreamReader {}
