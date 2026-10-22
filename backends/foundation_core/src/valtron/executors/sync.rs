use std::collections::VecDeque;

use crate::valtron::{Stream, StreamSpread};

/// WHY: Sync code at a boundary wants to `for`-loop over a valtron stream's
/// values as they arrive, not wait for all of them ([`collect_result`]) or
/// only the first ([`collect_one`]).
///
/// WHAT: An `Iterator<Item = D>` over a stream's `Next` values (and each
/// `Done` inside a `Spread`), in order. Control states (`Pending`, `Wait`,
/// `Delayed`, `Init`, `Ignore`) are consumed, not yielded.
///
/// HOW: Pulls the underlying iterator, which blocks or drives the executor
/// between items exactly as [`collect_result`] relies on. Use it on a stream
/// from `execute(..)` (or via `run_stream_iter` for an async stream), at
/// **sync boundaries only** — on single-threaded wasm a synchronous loop can
/// never let the event loop deliver the I/O it waits on.
///
/// ```ignore
/// for value in next_values(execute(task, None)?) {
///     println!("{value:?}");
/// }
/// ```
pub fn next_values<D, P, I>(stream: I) -> NextValues<D, I>
where
    I: Iterator<Item = Stream<D, P>>,
{
    NextValues {
        stream,
        spread: VecDeque::new(),
    }
}

/// The iterator `run_stream_iter` returns: an async stream's items, ending
/// with the stream. (Its task reports each item as `Some(item)` and the end
/// as `None`.)
pub struct StreamValues<T, I> {
    values: NextValues<Option<T>, I>,
}

impl<T, I> StreamValues<T, I> {
    /// Wrap the values of a `from_stream` task.
    pub fn new(values: NextValues<Option<T>, I>) -> Self {
        Self { values }
    }
}

impl<T, P, I> Iterator for StreamValues<T, I>
where
    I: Iterator<Item = Stream<Option<T>, P>>,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.values.next().flatten()
    }
}

/// See [`next_values`].
pub struct NextValues<D, I> {
    stream: I,
    spread: VecDeque<D>,
}

impl<D, P, I> Iterator for NextValues<D, I>
where
    I: Iterator<Item = Stream<D, P>>,
{
    type Item = D;

    fn next(&mut self) -> Option<D> {
        loop {
            if let Some(value) = self.spread.pop_front() {
                return Some(value);
            }
            match self.stream.next()? {
                Stream::Next(value) => return Some(value),
                Stream::Spread(items) => {
                    self.spread.extend(items.into_iter().filter_map(|item| match item {
                        StreamSpread::Done(value) => Some(value),
                        StreamSpread::Pending(_) => None,
                    }));
                }
                Stream::Pending(_)
                | Stream::Wait
                | Stream::Delayed(_)
                | Stream::Init
                | Stream::Ignore => {}
            }
        }
    }
}

/// WHY: Callers need a way to drain a Valtron stream at sync boundaries
/// without losing any `Next` values.
///
/// WHAT: Blocks the calling thread until the stream is exhausted, collecting
/// every `Stream::Next(value)` into a `Vec<D>`. Non-`Next` items
/// (`Pending`, `Delayed`, `Init`, `Ignore`) are consumed and discarded.
/// A `Stream::Spread` is drained: every `Done(value)` is collected, every
/// `Pending` payload is discarded.
///
/// HOW: Uses a manual loop over the stream.
///
/// Use this at **sync boundaries only** — never between composable stream
/// operations where `StreamIteratorExt` combinators should be used instead.
///
/// # Examples
///
/// ```ignore
/// // Launch work, collect at boundary
/// let stream = execute(task, None)?;
/// let results: Vec<MyValue> = collect_result(stream);
/// ```
pub fn collect_result<D, P>(stream: impl Iterator<Item = Stream<D, P>>) -> Vec<D> {
    let mut results = Vec::new();
    for item in stream {
        match item {
            Stream::Next(v) => results.push(v),
            Stream::Spread(items) => {
                for spread in items {
                    if let StreamSpread::Done(v) = spread {
                        results.push(v);
                    }
                }
            }
            // Pending, Delayed, Init, Ignore, Wait are consumed and discarded.
            _ => {}
        }
    }
    results
}

/// WHY: Single-value operations (like `get`) produce one `Next` value. Callers
/// want `Option<D>`, not `Vec<D>`.
///
/// WHAT: Blocks the calling thread until the first `Stream::Next(value)` is
/// found, then returns it. Skips `Pending`, `Delayed`, `Init`, `Ignore`.
/// A `Stream::Spread` is searched in order: the first `Done(value)` is
/// returned, `Pending` payloads are skipped. Returns `None` if the stream
/// exhausts without producing a `Next`.
///
/// HOW: Uses a manual loop over the stream.
///
/// Use this at **sync boundaries only** for streams known to produce exactly
/// one value. For multi-value streams, use `collect_result` instead.
///
/// # Examples
///
/// ```ignore
/// let stream = execute(task, None)?;
/// let value: Option<MyValue> = collect_one(stream);
/// ```
pub fn collect_one<D, P>(stream: impl Iterator<Item = Stream<D, P>>) -> Option<D> {
    for item in stream {
        match item {
            Stream::Next(v) => return Some(v),
            Stream::Spread(items) => {
                for spread in items {
                    if let StreamSpread::Done(v) = spread {
                        return Some(v);
                    }
                }
            }
            _ => {}
        }
    }
    None
}
