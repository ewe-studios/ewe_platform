use crate::valtron::{Stream, StreamSpread};

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
