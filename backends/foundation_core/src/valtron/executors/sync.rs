use crate::valtron::Stream;

/// WHY: Callers need a way to drain a Valtron stream at sync boundaries
/// without losing any `Next` values.
///
/// WHAT: Blocks the calling thread until the stream is exhausted, collecting
/// every `Stream::Next(value)` into a `Vec<D>`. Non-`Next` items
/// (`Pending`, `Delayed`, `Init`, `Ignore`) are consumed and discarded.
///
/// HOW: Uses `Iterator::filter_map` + `collect` on the stream.
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
    stream
        .filter_map(|s| match s {
            Stream::Next(v) => Some(v),
            _ => None,
        })
        .collect()
}

/// WHY: Single-value operations (like `get`) produce one `Next` value. Callers
/// want `Option<D>`, not `Vec<D>`.
///
/// WHAT: Blocks the calling thread until the first `Stream::Next(value)` is
/// found, then returns it. Skips `Pending`, `Delayed`, `Init`, `Ignore`.
/// Returns `None` if the stream exhausts without producing a `Next`.
///
/// HOW: Uses `Iterator::find_map` on the stream.
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
pub fn collect_one<D, P>(mut stream: impl Iterator<Item = Stream<D, P>>) -> Option<D> {
    stream.find_map(|s| match s {
        Stream::Next(v) => Some(v),
        _ => None,
    })
}
