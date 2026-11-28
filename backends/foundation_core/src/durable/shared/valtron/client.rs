//! The valtron wrapper for the Durable Streams client reader.
//!
//! The wrapper sits with the client core. A host supplies the transport.

use crate::durable::shared::client::{ReadMode, StreamHandle};
use crate::valtron::{
    drive_iterator, task, Ctx, DrivenTaskIterator, Stream, TaskSpread, TaskStatus,
};

/// The pending context a client reader reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientReadProgress {
    /// The reader is fetching the next page.
    Reading,
}

#[derive(Clone)]
struct ReaderState {
    handle: StreamHandle,
    from: Option<String>,
}

#[task]
async fn read_step(
    ctx: Ctx,
    state: ReaderState,
) -> Option<(ReaderState, TaskStatus<Vec<u8>, ClientReadProgress>)> {
    let _ = ctx;
    let mut builder = state.handle.read().mode(ReadMode::CatchUp);
    if let Some(from) = &state.from {
        builder = builder.offset(from.clone());
    }
    let page = builder.send().await.ok()?;

    let mut next = state;
    next.from.clone_from(&page.next_offset);

    if !page.chunks.is_empty() {
        let items = page
            .chunks
            .into_iter()
            .map(|chunk| TaskSpread::Ready(chunk.data))
            .collect();
        return Some((next, TaskStatus::Spread(items)));
    }

    if page.up_to_date || page.closed {
        return None;
    }

    Some((next, TaskStatus::Wait))
}

/// A valtron `StreamIterator` over the client read path.
pub struct ClientReader {
    inner: DrivenTaskIterator<read_step>,
}

impl ClientReader {
    /// Build a reader for a stream from the start.
    #[must_use]
    pub fn new(handle: StreamHandle, ctx: Ctx) -> Self {
        let state = ReaderState { handle, from: None };
        Self {
            inner: drive_iterator(read_step::new(state), ctx),
        }
    }
}

impl Iterator for ClientReader {
    type Item = Stream<Vec<u8>, ClientReadProgress>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(Stream::from)
    }
}
