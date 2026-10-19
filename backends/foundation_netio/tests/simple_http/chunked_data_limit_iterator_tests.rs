//! Tests for [`ChunkedDataLimitIterator`]: the wrapper that caps the number of
//! chunk payload bytes a chunked body may yield.
//!
//! Semantics under test:
//! - every item from the parent is passed through while the running total of
//!   `ChunkedData::Data` payload bytes stays `<= limit`;
//! - the item that would take the total past the limit is replaced by
//!   `HttpReaderError::LimitReached(limit)`, after which the iterator is finished;
//! - `DataEnded` and `Trailers` pass through and do not count toward the limit;
//! - errors from the parent are propagated, after which the iterator is finished;
//! - once the parent returns `None` the iterator stays finished.

use foundation_core::extensions::result_ext::BoxedError;
use foundation_netio::shared::http::{
    ChunkedData, ChunkedDataLimitIterator, ChunkedVecIterator, HttpReaderError,
};

type Item = Result<ChunkedData, BoxedError>;

fn data(bytes: &[u8]) -> ChunkedData {
    ChunkedData::Data(bytes.to_vec(), None)
}

fn parent_of(items: Vec<Item>) -> ChunkedVecIterator<BoxedError> {
    Box::new(items.into_iter())
}

fn limited(limit: usize, items: Vec<Item>) -> ChunkedDataLimitIterator {
    ChunkedDataLimitIterator::new(limit, parent_of(items))
}

fn expect_ok(item: Option<Item>) -> ChunkedData {
    match item {
        Some(Ok(value)) => value,
        Some(Err(err)) => panic!("expected Ok item, got error: {err}"),
        None => panic!("expected Ok item, got end of iteration"),
    }
}

fn expect_limit_reached(item: Option<Item>, limit: usize) {
    match item {
        Some(Err(err)) => match err.downcast_ref::<HttpReaderError>() {
            Some(HttpReaderError::LimitReached(reported)) => assert_eq!(*reported, limit),
            other => panic!("expected LimitReached({limit}), got {other:?} ({err})"),
        },
        Some(Ok(value)) => panic!("expected LimitReached({limit}), got Ok({value:?})"),
        None => panic!("expected LimitReached({limit}), got end of iteration"),
    }
}

/// A parent that panics if it is polled again after returning `None`, so a test
/// can prove the wrapper never re-polls a finished parent.
struct PanicAfterEnd {
    items: std::vec::IntoIter<Item>,
    ended: bool,
}

impl Iterator for PanicAfterEnd {
    type Item = Item;

    fn next(&mut self) -> Option<Item> {
        assert!(!self.ended, "parent polled again after returning None");
        let next = self.items.next();
        if next.is_none() {
            self.ended = true;
        }
        next
    }
}

#[derive(Debug)]
struct ParentFailure(&'static str);

impl std::fmt::Display for ParentFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parent failure: {}", self.0)
    }
}

impl std::error::Error for ParentFailure {}

#[test]
fn yields_all_items_under_the_limit() {
    let mut iter = limited(
        100,
        vec![Ok(data(b"hello")), Ok(data(b" ")), Ok(data(b"world"))],
    );

    assert_eq!(expect_ok(iter.next()), data(b"hello"));
    assert_eq!(expect_ok(iter.next()), data(b" "));
    assert_eq!(expect_ok(iter.next()), data(b"world"));
    assert!(iter.next().is_none());
}

#[test]
fn yields_extensions_unchanged() {
    let with_ext = ChunkedData::Data(
        b"abc".to_vec(),
        Some(vec![("name".to_string(), Some("value".to_string()))]),
    );
    let mut iter = limited(10, vec![Ok(with_ext.clone())]);

    assert_eq!(expect_ok(iter.next()), with_ext);
    assert!(iter.next().is_none());
}

#[test]
fn errors_once_limit_is_crossed_then_ends() {
    // 4 + 4 = 8 <= 10, the third chunk would make it 12 > 10.
    let mut iter = limited(
        10,
        vec![
            Ok(data(b"aaaa")),
            Ok(data(b"bbbb")),
            Ok(data(b"cccc")),
            Ok(data(b"d")),
            Ok(ChunkedData::DataEnded),
        ],
    );

    assert_eq!(expect_ok(iter.next()), data(b"aaaa"));
    assert_eq!(expect_ok(iter.next()), data(b"bbbb"));
    expect_limit_reached(iter.next(), 10);
    assert!(
        iter.next().is_none(),
        "nothing may be yielded after LimitReached"
    );
    assert!(iter.next().is_none(), "iterator must stay finished");
}

#[test]
fn single_chunk_larger_than_limit_errors_immediately() {
    let mut iter = limited(3, vec![Ok(data(b"abcd")), Ok(ChunkedData::DataEnded)]);

    expect_limit_reached(iter.next(), 3);
    assert!(iter.next().is_none());
}

#[test]
fn exact_limit_is_allowed() {
    let mut iter = limited(
        8,
        vec![
            Ok(data(b"aaaa")),
            Ok(data(b"bbbb")),
            Ok(ChunkedData::DataEnded),
        ],
    );

    assert_eq!(expect_ok(iter.next()), data(b"aaaa"));
    assert_eq!(expect_ok(iter.next()), data(b"bbbb"));
    assert_eq!(expect_ok(iter.next()), ChunkedData::DataEnded);
    assert!(iter.next().is_none());
}

#[test]
fn one_byte_over_exact_limit_errors() {
    let mut iter = limited(8, vec![Ok(data(b"aaaa")), Ok(data(b"bbbbb"))]);

    assert_eq!(expect_ok(iter.next()), data(b"aaaa"));
    expect_limit_reached(iter.next(), 8);
    assert!(iter.next().is_none());
}

#[test]
fn zero_limit_allows_empty_body_only() {
    let mut iter = limited(0, vec![Ok(ChunkedData::DataEnded)]);
    assert_eq!(expect_ok(iter.next()), ChunkedData::DataEnded);
    assert!(iter.next().is_none());

    let mut iter = limited(0, vec![Ok(data(b"x"))]);
    expect_limit_reached(iter.next(), 0);
    assert!(iter.next().is_none());
}

#[test]
fn data_ended_and_trailers_pass_through() {
    let trailers = ChunkedData::Trailers(vec![
        ("Expires".to_string(), Some("never".to_string())),
        ("X-Flag".to_string(), None),
    ]);
    // The data alone fills the limit exactly; DataEnded and Trailers must still
    // pass because they carry no body payload.
    let mut iter = limited(
        4,
        vec![
            Ok(data(b"abcd")),
            Ok(ChunkedData::DataEnded),
            Ok(trailers.clone()),
        ],
    );

    assert_eq!(expect_ok(iter.next()), data(b"abcd"));
    assert_eq!(expect_ok(iter.next()), ChunkedData::DataEnded);
    assert_eq!(expect_ok(iter.next()), trailers);
    assert!(iter.next().is_none());
}

#[test]
fn parent_errors_propagate_then_end() {
    let mut iter = limited(
        100,
        vec![
            Ok(data(b"abc")),
            Err(Box::new(ParentFailure("bad chunk size"))),
            Ok(data(b"never seen")),
        ],
    );

    assert_eq!(expect_ok(iter.next()), data(b"abc"));
    match iter.next() {
        Some(Err(err)) => {
            let failure = err
                .downcast_ref::<ParentFailure>()
                .expect("parent error must be propagated unchanged");
            assert_eq!(failure.0, "bad chunk size");
        }
        Some(Ok(value)) => panic!("expected parent error, got Ok({value:?})"),
        None => panic!("expected parent error, got end of iteration"),
    }
    assert!(
        iter.next().is_none(),
        "nothing may be yielded after an error"
    );
}

#[test]
fn stays_finished_after_parent_ends() {
    let parent = PanicAfterEnd {
        items: vec![Ok(data(b"ab"))].into_iter(),
        ended: false,
    };
    let mut iter = ChunkedDataLimitIterator::new(10, Box::new(parent));

    assert_eq!(expect_ok(iter.next()), data(b"ab"));
    assert!(iter.next().is_none());
    // Would panic inside PanicAfterEnd if the wrapper polled the parent again.
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn empty_parent_yields_nothing() {
    let mut iter = limited(10, vec![]);
    assert!(iter.next().is_none());
}
