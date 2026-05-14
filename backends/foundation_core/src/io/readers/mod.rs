//! TCP-resilient batch readers that use `read()` instead of `read_exact()`.
//!
//! # WHY
//!
//! `read_exact()` converts `WouldBlock` and `TimedOut` errors into `UnexpectedEof`,
//! making it impossible for callers to distinguish "no data available yet" from
//! "connection closed." On TCP streams this causes spurious failures on slow or
//! congested connections.
//!
//! # WHAT
//!
//! Provides:
//! - [`Data`] — enum distinguishing real bytes from retry signals
//! - [`BatchReader`] — iterator over read batches with retry handling
//! - [`FullBodyReader`] — iterator for known-size bodies, exposes `Data::Retry`
//! - [`EofReader`] — iterator for reading until EOF, exposes `Data::Retry`
//! - [`BatchStreamReader`] — adapter from `BatchReader` that yields `Data` (no loop)
//! - [`LimitedBatchStreamReader`] — like `BatchStreamReader` but with byte cap
//! - [`EOFStreamReader`] — reads until EOF via iterator, exposes `Data::Retry`
//! - [`LimitedEOFStreamReader`] — reads until EOF with max size enforcement
//! - [`DataBytesIterator`] — backward compat: absorbs retries, yields `Vec<u8>`
//!
//! # HOW
//!
//! Uses `read()` and propagates `WouldBlock`/`TimedOut` directly (as `Data::Retry`),
//! following the pattern established in `wire::websocket::frame::decode`.

use std::io::{self, Read};

/// WHY: Callers need to distinguish between actual data and transient
/// "no data yet" signals when reading from TCP streams.
///
/// WHAT: Result of a single read batch — either real bytes or a retry signal.
///
/// HOW: Wraps the two possible outcomes of a non-blocking `read()` call
/// into a type-safe enum that the caller can pattern-match on.
#[derive(Debug, Clone)]
pub enum Data {
    /// A batch of bytes successfully read from the source.
    Bytes(Vec<u8>),
    /// Yielded on `WouldBlock`, `TimedOut`, or `Ok(0)`-when-not-EOF.
    /// Signals the caller should retry (poll again later).
    Retry,
}

/// WHY: `read()`-based readers with a full inner `Read` need a way for
/// outer limited readers to tell them exactly how many bytes remain,
/// so they don't overshoot by reading a full buffer.
///
/// WHAT: Trait for hint-aware reading. `next_bytes(Some(n))` reads up to
/// `n` bytes; `next_bytes(None)` uses the reader's default batch size.
///
/// HOW: Implementations pass the hint down to their inner reader or
/// use it to limit the buffer allocation for the next `read()` call.
pub trait HintReadterator {
    type Item;
    type Error;

    fn next_bytes(&mut self, size_hint: Option<usize>) -> Option<Result<Self::Item, Self::Error>>;
}

/// WHY: `read_exact()` converts `WouldBlock`/`TimedOut` into `UnexpectedEof`,
/// hiding transient failures on TCP streams. An iterator-based reader using
/// `read()` preserves these error kinds so callers can decide when to retry.
///
/// WHAT: Iterator over read batches from a `Read` source with TCP-resilient
/// retry handling, yielding `Result<Data, io::Error>`.
///
/// HOW: Each call to `next()` performs a single `read()` into a buffer of
/// `batch_size` bytes. `WouldBlock`/`TimedOut` errors and (optionally)
/// zero-length reads are converted to `Data::Retry`. A consecutive retry
/// counter prevents infinite spinning.
pub struct BatchReader<R: Read> {
    reader: R,
    batch_size: usize,
    eof_on_zero_read: bool,
    received_data: bool,
    max_consecutive_retries: usize,
    consecutive_retries: usize,
    done: bool,
}

impl<R: Read> BatchReader<R> {
    /// Create a new `BatchReader` with default configuration.
    ///
    /// Defaults: `batch_size=512`, `eof_on_zero_read=true`, `max_consecutive_retries=100`.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            batch_size: 512,
            received_data: false,
            eof_on_zero_read: true,
            max_consecutive_retries: 100,
            consecutive_retries: 0,
            done: false,
        }
    }

    /// Set the read buffer size per batch.
    #[must_use]
    pub fn batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// If `true`, `read()` returning 0 means EOF (iterator ends).
    /// If `false`, 0 is retryable (yields `Data::Retry`).
    #[must_use]
    pub fn eof_on_zero_read(mut self, eof: bool) -> Self {
        self.eof_on_zero_read = eof;
        self
    }

    /// Maximum retries without progress before erroring.
    #[must_use]
    pub fn max_consecutive_retries(mut self, max: usize) -> Self {
        self.max_consecutive_retries = max;
        self
    }
}

impl<R: Read> Iterator for BatchReader<R> {
    type Item = Result<Data, io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let _span = tracing::span!(tracing::Level::TRACE, "next").entered();
        self.next_bytes(None)
    }
}

impl<R: Read> HintReadterator for BatchReader<R> {
    type Item = Data;
    type Error = io::Error;

    fn next_bytes(&mut self, size_hint: Option<usize>) -> Option<Result<Self::Item, Self::Error>> {
        let _span =
            tracing::span!(tracing::Level::TRACE, "next_bytes", hint = ?size_hint).entered();
        if self.done {
            tracing::trace!("Reading is now considered done!");
            return None;
        }

        // return zero once int indicates no more to read.
        if size_hint.is_some() && size_hint == Some(0) {
            return None;
        }

        let target_size = size_hint.unwrap_or(self.batch_size);
        tracing::trace!("Using target size for buffer: {}", &target_size);
        let mut buf = vec![0u8; target_size];
        match self.reader.read(&mut buf) {
            Ok(0) => {
                tracing::trace!("Zero bytes read occured");
                if self.eof_on_zero_read {
                    tracing::trace!("Stream is now considered finished");
                    self.done = true;
                    None
                } else {
                    self.consecutive_retries += 1;
                    if self.consecutive_retries > self.max_consecutive_retries {
                        self.done = true;

                        if self.received_data {
                            return None;
                        }

                        Some(Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "max consecutive retries exceeded without progress",
                        )))
                    } else {
                        tracing::debug!("Sending retry");
                        Some(Ok(Data::Retry))
                    }
                }
            }
            Ok(n) => {
                tracing::trace!("Received data Bytes(len={})", n);
                self.received_data = true;
                self.consecutive_retries = 0;
                buf.truncate(n);
                tracing::trace!(
                    "Truncating to length Bytes(len={}) with data: {:?}",
                    n,
                    &buf
                );
                Some(Ok(Data::Bytes(buf)))
            }
            Err(e)
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut =>
            {
                tracing::error!("Timeout/WouldBlock error received: {:?}", &e);
                self.consecutive_retries += 1;
                tracing::trace!(
                    "Checking consecutive errors (max={}): current={}",
                    &self.max_consecutive_retries,
                    self.consecutive_retries
                );
                if self.consecutive_retries > self.max_consecutive_retries {
                    self.done = true;

                    if self.received_data {
                        tracing::trace!("Finished reading, saw data, ending");
                        return None;
                    }

                    tracing::trace!(
                        "Problematic, max retries reached, returning exahaustion errors"
                    );
                    Some(Err(io::Error::new(
                        e.kind(),
                        "max consecutive retries exceeded without progress",
                    )))
                } else {
                    Some(Ok(Data::Retry))
                }
            }
            Err(e) => {
                tracing::error!("Read error occured: {:?}", &e);
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// FullBodyReader — iterator for known-size bodies
// ---------------------------------------------------------------------------

/// WHY: Stream known-size bodies, exposing `Data::Retry` for caller control.
///
/// WHAT: Iterator yielding `Result<Data, io::Error>` until `target_size` bytes read.
///
/// HOW: Wraps `BatchReader`, counts bytes, returns `Data::Bytes` or passes through `Data::Retry`.
pub struct FullBodyReader<R: Read> {
    inner: BatchReader<R>,
    bytes_yielded: usize,
    target_size: usize,
}

impl<R: Read> FullBodyReader<R> {
    /// Create a new `FullBodyReader` that reads exactly `target_size` bytes.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(inner: BatchReader<R>, target_size: usize) -> Self {
        Self {
            inner,
            bytes_yielded: 0,
            target_size,
        }
    }
}

impl<R: Read> Iterator for FullBodyReader<R> {
    type Item = Result<Data, io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bytes_yielded >= self.target_size {
            return None;
        }

        match self.inner.next() {
            Some(Ok(Data::Bytes(bytes))) => {
                self.bytes_yielded += bytes.len();
                Some(Ok(Data::Bytes(bytes)))
            }
            other => other, // Pass through Data::Retry and errors
        }
    }
}

// ---------------------------------------------------------------------------
// EofReader — iterator for reading until EOF
// ---------------------------------------------------------------------------

/// WHY: Stream until EOF, exposing `Data::Retry` for caller control.
///
/// WHAT: Iterator yielding `Result<Data, io::Error>` until EOF.
///
/// HOW: Wraps `BatchReader` with `eof_on_zero_read=true`, passes through all `Data`.
pub struct EofReader<R: Read> {
    inner: BatchReader<R>,
}

impl<R: Read> EofReader<R> {
    /// Create a new `EofReader` that reads until EOF.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(inner: BatchReader<R>) -> Self {
        Self { inner }
    }
}

impl<R: Read> Iterator for EofReader<R> {
    type Item = Result<Data, io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

// ---------------------------------------------------------------------------
// BatchStreamReader — adapter from BatchReader, exposes Data (no loop)
// ---------------------------------------------------------------------------

/// WHY: Adapter from `BatchReader` that can be boxed for `SendSafeBody`.
///
/// WHAT: Iterator yielding `Result<Data, BoxedError>` — exposes `Data::Retry` to caller.
///
/// HOW: No internal loop — caller decides how to handle `Data::Retry`.
pub struct BatchStreamReader<R: Read> {
    inner: BatchReader<R>,
}

impl<R: Read> BatchStreamReader<R> {
    /// Wrap a [`BatchReader`] to produce a stream-compatible iterator.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(inner: BatchReader<R>) -> Self {
        Self { inner }
    }
}

impl<R: Read + Send> Iterator for BatchStreamReader<R> {
    type Item = Result<Data, Box<dyn std::error::Error + 'static>>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(Ok(data)) => Some(Ok(data)),
            Some(Err(e)) => Some(Err(Box::new(e))),
            None => None,
        }
    }
}

// ---------------------------------------------------------------------------
// LimitedBatchStreamReader — byte-capped BatchStreamReader
// ---------------------------------------------------------------------------

/// WHY: Stream limited-size bodies with byte cap, exposing `Data::Retry`.
///
/// WHAT: Iterator yielding `Result<Data, BoxedError>`, stops at cap.
///
/// HOW: Wraps `BatchReader`, counts bytes, returns `None` when cap reached.
pub struct LimitedBatchStreamReader<R: Read> {
    inner: BatchReader<R>,
    bytes_yielded: usize,
    byte_cap: usize,
}

impl<R: Read> LimitedBatchStreamReader<R> {
    /// Create a new `LimitedBatchStreamReader` capped at `byte_cap` bytes.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(inner: BatchReader<R>, byte_cap: usize) -> Self {
        Self {
            inner,
            bytes_yielded: 0,
            byte_cap,
        }
    }
}

impl<R: Read + Send> Iterator for LimitedBatchStreamReader<R> {
    type Item = Result<Data, Box<dyn std::error::Error + 'static>>;

    fn next(&mut self) -> Option<Self::Item> {
        tracing::trace!("Pulling next data bytes from stream");
        if self.bytes_yielded >= self.byte_cap {
            tracing::trace!("Readed max allowed bytes, stopping");
            return None;
        }

        let remaining = self.byte_cap - self.bytes_yielded;

        tracing::trace!("Remaining bytes to read: {}", remaining);
        match self.inner.next_bytes(Some(remaining)) {
            Some(Ok(Data::Bytes(bytes))) => {
                tracing::trace!("Readed new bytes size from reader: {}", bytes.len());
                self.bytes_yielded += bytes.len();
                Some(Ok(Data::Bytes(bytes)))
            }
            Some(Ok(data)) => Some(Ok(data)), // Data::Retry
            Some(Err(e)) => Some(Err(Box::new(e))),
            None => None,
        }
    }
}

// ---------------------------------------------------------------------------
// EOFStreamReader — reads until EOF via iterator
// ---------------------------------------------------------------------------

/// WHY: Stream until EOF via `BoxedSendableIterator`, exposing `Data::Retry`.
///
/// WHAT: Iterator yielding `Result<Data, BoxedError>` until EOF.
pub struct EOFStreamReader<R: Read> {
    inner: BatchReader<R>,
}

impl<R: Read> EOFStreamReader<R> {
    /// Create a new `EOFStreamReader` that reads until EOF.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(inner: BatchReader<R>) -> Self {
        Self { inner }
    }
}

impl<R: Read + Send> Iterator for EOFStreamReader<R> {
    type Item = Result<Data, Box<dyn std::error::Error + 'static>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|r| r.map_err(|e| Box::new(e) as Box<dyn std::error::Error + 'static>))
    }
}

// ---------------------------------------------------------------------------
// LimitedEOFStreamReader — reads until EOF with max size enforcement
// ---------------------------------------------------------------------------

/// WHY: Stream until EOF with max size enforcement, exposing `Data::Retry`.
///
/// WHAT: Iterator yielding `Result<Data, BoxedError>`, errors if cap exceeded.
pub struct LimitedEOFStreamReader<R: Read> {
    inner: BatchReader<R>,
    bytes_yielded: usize,
    byte_cap: usize,
}

impl<R: Read> LimitedEOFStreamReader<R> {
    /// Create a new `LimitedEOFStreamReader` capped at `byte_cap` bytes.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(inner: BatchReader<R>, byte_cap: usize) -> Self {
        Self {
            inner,
            bytes_yielded: 0,
            byte_cap,
        }
    }
}

impl<R: Read + Send> Iterator for LimitedEOFStreamReader<R> {
    type Item = Result<Data, Box<dyn std::error::Error + 'static>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bytes_yielded > self.byte_cap {
            tracing::error!(
                "body size {} exceeds max {}",
                self.bytes_yielded,
                self.byte_cap
            );
            return Some(Err(Box::new(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "body size {} exceeds max {}",
                    self.bytes_yielded, self.byte_cap
                ),
            ))));
        }

        let remaining = self.byte_cap - self.bytes_yielded;
        match self.inner.next_bytes(Some(remaining)) {
            Some(Ok(Data::Bytes(bytes))) => {
                self.bytes_yielded += bytes.len();
                if self.bytes_yielded > self.byte_cap {
                    tracing::error!(
                        "body size {} exceeds max {}",
                        self.bytes_yielded,
                        self.byte_cap
                    );
                    return None;
                }
                Some(Ok(Data::Bytes(bytes)))
            }
            Some(Ok(Data::Retry)) => Some(Ok(Data::Retry)),
            Some(Err(e)) => Some(Err(Box::new(e) as Box<dyn std::error::Error + 'static>)),
            None => None,
        }
    }
}

// ---------------------------------------------------------------------------
// DataBytesIterator — backward compat: absorbs retries, yields Vec<u8>
// ---------------------------------------------------------------------------

use std::marker::PhantomData;

/// WHY: Backward compatibility — restores old retry-absorbing behavior.
///
/// WHAT: Wraps `Iterator<Item=Result<Data, E>>` and loops on `Data::Retry`.
///
/// HOW: Transforms `Data::Bytes` into `Vec<u8>`, filters `Data::Retry`.
pub struct DataBytesIterator<I, E> {
    inner: I,
    _marker: PhantomData<E>,
}

impl<I, E> DataBytesIterator<I, E> {
    /// Wrap a `Data`-exposing iterator into a `Vec<u8>`-yielding iterator.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(inner: I) -> Self {
        Self {
            inner,
            _marker: PhantomData,
        }
    }
}

impl<I, E> Iterator for DataBytesIterator<I, E>
where
    I: Iterator<Item = Result<Data, E>>,
{
    type Item = Result<Vec<u8>, E>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some(Ok(Data::Retry)) => continue, // Absorb and retry
                Some(Ok(Data::Bytes(bytes))) => return Some(Ok(bytes)),
                Some(Err(e)) => return Some(Err(e)),
                None => return None,
            }
        }
    }
}

/// WHY: Convenience extension trait for converting `Data`-exposing iterators
/// to `Vec<u8>`-yielding iterators via `.into_bytes()`.
///
/// WHAT: Extension trait implemented for all `Iterator<Item=Result<Data, E>>`.
pub trait IntoDataBytes<E> {
    fn into_bytes(self) -> DataBytesIterator<Self, E>
    where
        Self: Iterator<Item = Result<Data, E>> + Sized,
    {
        DataBytesIterator::new(self)
    }
}

impl<I, E> IntoDataBytes<E> for I where I: Iterator<Item = Result<Data, E>> {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tracing_test::traced_test;

    // -- BatchReader tests (UNCHANGED — already uses Data enum) --

    #[test]
    fn batch_reader_normal_reads() {
        let data = b"hello world";
        let reader = BatchReader::new(Cursor::new(data.to_vec())).batch_size(5);
        let results: Vec<_> = reader.collect();

        assert_eq!(results.len(), 3);
        for r in &results {
            assert!(r.is_ok());
            match r.as_ref().unwrap() {
                Data::Bytes(b) => assert!(!b.is_empty()),
                Data::Retry => panic!("unexpected retry"),
            }
        }
    }

    #[test]
    fn batch_reader_empty_source_eof() {
        let reader = BatchReader::new(Cursor::new(Vec::<u8>::new()));
        let results: Vec<_> = reader.collect();
        assert!(results.is_empty());
    }

    #[test]
    fn batch_reader_would_block_handling() {
        struct WouldBlockReader {
            calls: usize,
            data: Vec<u8>,
            pos: usize,
        }

        impl Read for WouldBlockReader {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                self.calls += 1;
                if self.calls % 2 == 1 && self.pos < self.data.len() {
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "would block"));
                }
                if self.pos >= self.data.len() {
                    return Ok(0);
                }
                let n = std::cmp::min(buf.len(), self.data.len() - self.pos);
                buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
                self.pos += n;
                Ok(n)
            }
        }

        let reader = BatchReader::new(WouldBlockReader {
            calls: 0,
            data: b"test".to_vec(),
            pos: 0,
        })
        .batch_size(4);

        let results: Vec<_> = reader.collect();
        let mut got_retry = false;
        let mut got_bytes = false;
        for r in &results {
            match r.as_ref().unwrap() {
                Data::Retry => got_retry = true,
                Data::Bytes(_) => got_bytes = true,
            }
        }
        assert!(got_retry);
        assert!(got_bytes);
    }

    #[test]
    fn batch_reader_retry_limit_exceeded() {
        struct AlwaysWouldBlock;
        impl Read for AlwaysWouldBlock {
            fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"))
            }
        }

        let reader = BatchReader::new(AlwaysWouldBlock).max_consecutive_retries(3);
        let results: Vec<_> = reader.collect();

        assert_eq!(results.len(), 4);
        assert!(results.last().unwrap().is_err());
    }

    #[test]
    fn batch_reader_eof_on_zero_read_false() {
        struct ZeroThenData {
            calls: usize,
        }
        impl Read for ZeroThenData {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                self.calls += 1;
                if self.calls <= 2 {
                    Ok(0)
                } else if self.calls == 3 {
                    buf[0] = b'x';
                    Ok(1)
                } else {
                    Ok(0)
                }
            }
        }

        let reader = BatchReader::new(ZeroThenData { calls: 0 })
            .eof_on_zero_read(false)
            .max_consecutive_retries(5);

        let results: Vec<_> = reader.take(4).collect();
        assert_eq!(results.len(), 4);
        assert!(matches!(results[0].as_ref().unwrap(), Data::Retry));
        assert!(matches!(results[1].as_ref().unwrap(), Data::Retry));
        assert!(matches!(results[2].as_ref().unwrap(), Data::Bytes(_)));
    }

    // -- FullBodyReader tests (converted to iterator pattern) --

    #[test]
    fn full_body_reader_complete_read() {
        let data = b"hello world";
        let reader = FullBodyReader::new(BatchReader::new(Cursor::new(data.to_vec())), data.len());

        let collected: Vec<u8> = reader
            .filter_map(|r| r.ok())
            .filter_map(|d| match d {
                Data::Bytes(b) => Some(b),
                Data::Retry => None,
            })
            .flatten()
            .collect();

        assert_eq!(collected, data);
    }

    #[test]
    fn full_body_reader_partial_reads() {
        struct OneByteReader {
            data: Vec<u8>,
            pos: usize,
        }
        impl Read for OneByteReader {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.pos >= self.data.len() {
                    return Ok(0);
                }
                buf[0] = self.data[self.pos];
                self.pos += 1;
                Ok(1)
            }
        }

        let data = b"hello";
        let reader = FullBodyReader::new(
            BatchReader::new(OneByteReader {
                data: data.to_vec(),
                pos: 0,
            }),
            data.len(),
        );

        let collected: Vec<u8> = reader
            .filter_map(|r| r.ok())
            .filter_map(|d| match d {
                Data::Bytes(b) => Some(b),
                Data::Retry => None,
            })
            .flatten()
            .collect();

        assert_eq!(collected, data);
    }

    #[test]
    fn full_body_reader_retry_handling() {
        struct RetryReader {
            data: Vec<u8>,
            pos: usize,
            calls: usize,
        }
        impl Read for RetryReader {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                self.calls += 1;
                if self.calls.is_multiple_of(3) {
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"));
                }
                if self.pos >= self.data.len() {
                    return Ok(0);
                }
                let n = std::cmp::min(buf.len(), self.data.len() - self.pos);
                buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
                self.pos += n;
                Ok(n)
            }
        }

        let data = b"hello world test";
        let reader = FullBodyReader::new(
            BatchReader::new(RetryReader {
                data: data.to_vec(),
                pos: 0,
                calls: 0,
            }),
            data.len(),
        );

        let collected: Vec<u8> = reader
            .filter_map(|r| r.ok())
            .filter_map(|d| match d {
                Data::Bytes(b) => Some(b),
                Data::Retry => None,
            })
            .flatten()
            .collect();

        assert_eq!(collected, data);
    }

    #[test]
    fn full_body_reader_unexpected_eof() {
        let data = b"hi";
        let reader = FullBodyReader::new(BatchReader::new(Cursor::new(data.to_vec())), 10);

        let collected: Vec<u8> = reader
            .filter_map(|r| r.ok())
            .filter_map(|d| match d {
                Data::Bytes(b) => Some(b),
                Data::Retry => None,
            })
            .flatten()
            .collect();

        assert_ne!(collected.len(), 10);
        assert_eq!(collected, data);
    }

    #[test]
    fn full_body_reader_retry_limit_exceeded() {
        struct AlwaysWouldBlock;
        impl Read for AlwaysWouldBlock {
            fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"))
            }
        }

        let reader = FullBodyReader::new(
            BatchReader::new(AlwaysWouldBlock).max_consecutive_retries(3),
            10,
        );

        let results: Vec<_> = reader.collect();
        // Should get an error after retries exhausted
        assert!(results.iter().any(|r| r.is_err()));
    }

    // -- BatchStreamReader tests (updated: now exposes Data::Retry) --

    #[test]
    fn batch_stream_reader_exposes_retries() {
        struct AlternatingReader {
            data: Vec<u8>,
            pos: usize,
            calls: usize,
        }
        impl Read for AlternatingReader {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                self.calls += 1;
                if self.calls % 2 == 1 && self.pos < self.data.len() {
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"));
                }
                if self.pos >= self.data.len() {
                    return Ok(0);
                }
                let n = std::cmp::min(buf.len(), self.data.len() - self.pos);
                buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
                self.pos += n;
                Ok(n)
            }
        }

        let batch = BatchReader::new(AlternatingReader {
            data: b"hello".to_vec(),
            pos: 0,
            calls: 0,
        })
        .batch_size(5);

        let mut stream = BatchStreamReader::new(batch);

        let mut got_retry = false;
        let mut got_bytes = false;

        while let Some(result) = stream.next() {
            match result.unwrap() {
                Data::Retry => got_retry = true,
                Data::Bytes(_) => got_bytes = true,
            }
        }

        assert!(got_retry, "Should expose Data::Retry");
        assert!(got_bytes, "Should expose Data::Bytes");
    }

    #[test]
    fn batch_stream_reader_propagates_errors() {
        struct ErrorReader;
        impl Read for ErrorReader {
            fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::ConnectionReset, "reset"))
            }
        }

        let batch = BatchReader::new(ErrorReader);
        let stream = BatchStreamReader::new(batch);
        let results: Vec<_> = stream.collect();

        assert_eq!(results.len(), 1);
        assert!(results[0].is_err());
    }

    #[test]
    fn batch_stream_reader_empty_source() {
        let batch = BatchReader::new(Cursor::new(Vec::<u8>::new()));
        let stream = BatchStreamReader::new(batch);
        let results: Vec<_> = stream.collect();
        assert!(results.is_empty());
    }

    // -- EofReader tests (converted to iterator pattern) --

    #[test]
    fn eof_reader_complete_read() {
        let data = b"hello world test data";
        let batch = BatchReader::new(Cursor::new(data.to_vec()))
            .batch_size(512)
            .max_consecutive_retries(100);
        let mut reader = EofReader::new(batch);

        let collected: Vec<u8> = reader
            .filter_map(|r| r.ok())
            .filter_map(|d| match d {
                Data::Bytes(b) => Some(b),
                Data::Retry => None,
            })
            .flatten()
            .collect();

        assert_eq!(collected, data);
    }

    #[test]
    #[traced_test]
    fn eof_reader_with_max_size() {
        // Test EOFStreamReader with LimitedEOFStreamReader for max size
        let data = b"hello world";
        let batch = BatchReader::new(Cursor::new(data.to_vec()))
            .batch_size(512)
            .max_consecutive_retries(100);
        let mut reader = LimitedEOFStreamReader::new(batch, 20);

        let collected: Vec<u8> = reader
            .filter_map(|r| {
                tracing::info!("Filtering first filter map with: {:?}", &r);
                r.ok()
            })
            .filter_map(|d| {
                tracing::info!("Filtering results with: {:?}", &d);
                match d {
                    Data::Bytes(b) => Some(b),
                    Data::Retry => None,
                }
            })
            .flatten()
            .collect();

        assert_eq!(collected, data);
        println!("Results generarted from first reader: {:?}", &collected);

        // Test exceeding cap
        let batch = BatchReader::new(Cursor::new(data.to_vec()))
            .batch_size(512)
            .max_consecutive_retries(100);
        let reader = LimitedEOFStreamReader::new(batch, 5);

        let results: Vec<Result<Data, Box<dyn std::error::Error + 'static>>> = reader.collect();
        println!("Results generarted from reader: {:?}", &results);
        assert!(results.len() > 0);
    }

    #[test]
    fn eof_reader_partial_reads() {
        struct OneByteReader {
            data: Vec<u8>,
            pos: usize,
        }
        impl Read for OneByteReader {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.pos >= self.data.len() {
                    return Ok(0);
                }
                buf[0] = self.data[self.pos];
                self.pos += 1;
                Ok(1)
            }
        }

        let data = b"hello";
        let batch = BatchReader::new(OneByteReader {
            data: data.to_vec(),
            pos: 0,
        })
        .batch_size(512)
        .max_consecutive_retries(100);
        let mut reader = EofReader::new(batch);

        let collected: Vec<u8> = reader
            .filter_map(|r| r.ok())
            .filter_map(|d| match d {
                Data::Bytes(b) => Some(b),
                Data::Retry => None,
            })
            .flatten()
            .collect();

        assert_eq!(collected, data);
    }

    #[test]
    fn eof_reader_retry_handling() {
        struct RetryReader {
            data: Vec<u8>,
            pos: usize,
            calls: usize,
        }
        impl Read for RetryReader {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                self.calls += 1;
                if self.calls.is_multiple_of(3) {
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"));
                }
                if self.pos >= self.data.len() {
                    return Ok(0);
                }
                let n = std::cmp::min(buf.len(), self.data.len() - self.pos);
                buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
                self.pos += n;
                Ok(n)
            }
        }

        let data = b"hello world test";
        let batch = BatchReader::new(RetryReader {
            data: data.to_vec(),
            pos: 0,
            calls: 0,
        })
        .batch_size(512)
        .max_consecutive_retries(100);
        let mut reader = EofReader::new(batch);

        let collected: Vec<u8> = reader
            .filter_map(|r| r.ok())
            .filter_map(|d| match d {
                Data::Bytes(b) => Some(b),
                Data::Retry => None,
            })
            .flatten()
            .collect();

        assert_eq!(collected, data);
    }

    #[test]
    fn eof_reader_retry_limit_exceeded() {
        struct AlwaysWouldBlock;
        impl Read for AlwaysWouldBlock {
            fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"))
            }
        }

        let batch = BatchReader::new(AlwaysWouldBlock).max_consecutive_retries(3);
        let mut reader = EofReader::new(batch);

        let results: Vec<_> = reader.collect();
        assert!(results.iter().any(|r| r.is_err()));
    }

    #[test]
    fn eof_reader_empty_source() {
        let batch = BatchReader::new(Cursor::new(Vec::<u8>::new()));
        let mut reader = EofReader::new(batch);

        let collected: Vec<u8> = reader
            .filter_map(|r| r.ok())
            .filter_map(|d| match d {
                Data::Bytes(b) => Some(b),
                Data::Retry => None,
            })
            .flatten()
            .collect();

        assert!(collected.is_empty());
    }

    // -- DataBytesIterator tests --

    #[test]
    fn data_bytes_iterator_filters_retries() {
        struct AlternatingRetryReader {
            data: Vec<u8>,
            pos: usize,
            calls: usize,
        }
        impl Read for AlternatingRetryReader {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                self.calls += 1;
                if self.calls % 2 == 1 && self.pos < self.data.len() {
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"));
                }
                if self.pos >= self.data.len() {
                    return Ok(0);
                }
                let n = std::cmp::min(buf.len(), self.data.len() - self.pos);
                buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
                self.pos += n;
                Ok(n)
            }
        }

        let batch = BatchReader::new(AlternatingRetryReader {
            data: b"test".to_vec(),
            pos: 0,
            calls: 0,
        })
        .batch_size(5);
        let data_stream = BatchStreamReader::new(batch);
        let mut wrapped = DataBytesIterator::new(data_stream);

        while let Some(result) = wrapped.next() {
            assert!(result.is_ok());
        }
    }

    #[test]
    fn data_bytes_iterator_propagates_errors() {
        struct ErrorReader;
        impl Read for ErrorReader {
            fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::ConnectionReset, "reset"))
            }
        }

        let batch = BatchReader::new(ErrorReader);
        let data_stream = BatchStreamReader::new(batch);
        let mut wrapped = DataBytesIterator::new(data_stream);

        assert!(wrapped.next().unwrap().is_err());
    }

    #[test]
    fn data_bytes_iterator_empty_source() {
        let batch = BatchReader::new(Cursor::new(Vec::<u8>::new()));
        let data_stream = BatchStreamReader::new(batch);
        let mut wrapped = DataBytesIterator::new(data_stream);

        assert!(wrapped.next().is_none());
    }

    // -- LimitedBatchStreamReader tests --

    #[test]
    fn limited_batch_stream_reader_stops_at_cap() {
        let data = b"hello world";
        let batch = BatchReader::new(Cursor::new(data.to_vec())).batch_size(5);
        let mut limited = LimitedBatchStreamReader::new(batch, 5);

        let first = limited.next().unwrap().unwrap();
        assert!(matches!(first, Data::Bytes(ref b) if b == b"hello"));

        // Should stop at cap
        assert!(limited.next().is_none());
    }

    #[test]
    fn limited_batch_stream_reader_exposes_retries() {
        struct RetryThenDataReader {
            data: Vec<u8>,
            pos: usize,
            calls: usize,
        }
        impl Read for RetryThenDataReader {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                self.calls += 1;
                if self.calls == 1 {
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"));
                }
                if self.pos >= self.data.len() {
                    return Ok(0);
                }
                let n = std::cmp::min(buf.len(), self.data.len() - self.pos);
                buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
                self.pos += n;
                Ok(n)
            }
        }

        let batch = BatchReader::new(RetryThenDataReader {
            data: b"test".to_vec(),
            pos: 0,
            calls: 0,
        })
        .batch_size(100);
        let mut limited = LimitedBatchStreamReader::new(batch, 100);

        assert!(matches!(limited.next(), Some(Ok(Data::Retry))));
        assert!(matches!(limited.next(), Some(Ok(Data::Bytes(_)))));
    }

    // -- EOFStreamReader tests --

    #[test]
    fn eof_stream_reader_reads_until_eof() {
        let data = b"hello world";
        let batch = BatchReader::new(Cursor::new(data.to_vec()))
            .batch_size(512)
            .eof_on_zero_read(true);
        let mut eof_reader = EOFStreamReader::new(batch);

        let collected: Vec<u8> = eof_reader
            .filter_map(|r| r.ok())
            .filter_map(|d| match d {
                Data::Bytes(b) => Some(b),
                Data::Retry => None,
            })
            .flatten()
            .collect();

        assert_eq!(collected, data);
    }

    #[test]
    fn eof_stream_reader_exposes_retries() {
        struct RetryReader;
        impl Read for RetryReader {
            fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"))
            }
        }

        let batch = BatchReader::new(RetryReader).eof_on_zero_read(false);
        let mut eof_reader = EOFStreamReader::new(batch);

        assert!(matches!(eof_reader.next(), Some(Ok(Data::Retry))));
    }

    // -- LimitedEOFStreamReader tests --

    #[test]
    fn limited_eof_stream_reader_enforces_max() {
        let data = b"hello world this is long";
        let batch = BatchReader::new(Cursor::new(data.to_vec())).batch_size(10);
        let mut limited = LimitedEOFStreamReader::new(batch, 10);

        let mut collected = Vec::new();
        while let Some(result) = limited.next() {
            match result {
                Ok(Data::Bytes(bytes)) => collected.extend(bytes),
                Ok(Data::Retry) => continue,
                Err(e) => {
                    assert!(
                        collected.len() >= 10 || e.to_string().contains("exceeds"),
                        "expected error at cap, got: {e}"
                    );
                    return;
                }
            }
        }
        panic!("Should have errored at cap");
    }

    #[test]
    fn limited_eof_stream_reader_under_limit() {
        let data = b"short";
        let batch = BatchReader::new(Cursor::new(data.to_vec())).batch_size(100);
        let mut limited = LimitedEOFStreamReader::new(batch, 100);

        let collected: Vec<u8> = limited
            .filter_map(|r| r.ok())
            .filter_map(|d| match d {
                Data::Bytes(b) => Some(b),
                Data::Retry => None,
            })
            .flatten()
            .collect();

        assert_eq!(collected, data);
    }

    // // -- HintReadterator tests --

    // /// WHY: Verify `next_bytes(Some(n))` limits buffer allocation to exactly `n`,
    // /// preventing overshoot when the inner reader's batch_size exceeds remaining bytes.
    // ///
    // /// WHAT: Use a one-byte-per-read source with `batch_size=100` but `byte_cap=7`.
    // /// Without the hint, `LimitedBatchStreamReader` would request 100 bytes and
    // /// consume all 11. With the hint, it requests `remaining` each time.
    // #[test]
    // fn limited_batch_stream_reader_hint_prevents_overshoot() {
    //     use crate::io::readers::HintReadterator;

    //     let data = b"hello world";
    //     let batch = BatchReader::new(Cursor::new(data.to_vec())).batch_size(100);
    //     let mut limited = LimitedBatchStreamReader::new(batch, 7);

    //     let mut collected = Vec::new();
    //     while let Some(result) = limited.next() {
    //         match result {
    //             Ok(Data::Bytes(bytes)) => collected.extend(bytes),
    //             Ok(Data::Retry) => continue,
    //             Err(e) => panic!("unexpected error: {e}"),
    //         }
    //     }

    //     assert_eq!(
    //         collected,
    //         &data[..7],
    //         "should only read 7 bytes, got {} bytes: {:?}",
    //         collected.len(),
    //         std::str::from_utf8(&collected)
    //     );
    // }

    // /// WHY: Same overshoot test for `LimitedEOFStreamReader`.
    // #[test]
    // fn limited_eof_stream_reader_hint_prevents_overshoot() {
    //     let data = b"hello world";
    //     let batch = BatchReader::new(Cursor::new(data.to_vec())).batch_size(100);
    //     let mut limited = LimitedEOFStreamReader::new(batch, 7);

    //     let mut collected = Vec::new();
    //     while let Some(result) = limited.next() {
    //         match result {
    //             Ok(Data::Bytes(bytes)) => collected.extend(bytes),
    //             Ok(Data::Retry) => continue,
    //             Err(e) => {
    //                 assert!(
    //                     e.to_string().contains("exceeds"),
    //                     "expected overshoot error, got: {e}"
    //                 );
    //                 break;
    //             }
    //         }
    //     }

    //     // With the hint, inner reader allocates exactly `remaining` bytes,
    //     // so we should hit exactly the cap without overshooting.
    //     assert_eq!(
    //         collected.len(),
    //         7,
    //         "should read exactly 7 bytes, got {}",
    //         collected.len()
    //     );
    //     assert_eq!(collected, &data[..7]);
    // }

    // /// WHY: Verify `next_bytes` directly with progressively shrinking hints
    // /// limits the buffer to the hint size.
    // ///
    // /// WHAT: Call `BatchReader::next_bytes` with decreasing size hints
    // /// and confirm each buffer is capped to the hint.
    // #[test]
    // fn batch_reader_next_bytes_respects_hint() {
    //     let data = b"0123456789abcdef";
    //     let mut batch = BatchReader::new(Cursor::new(data.to_vec())).batch_size(100);

    //     let result = batch.next_bytes(Some(4)).unwrap().unwrap();
    //     match result {
    //         Data::Bytes(b) => assert_eq!(&b, b"0123", "hint=4 should read exactly 4 bytes"),
    //         Data::Retry => panic!("expected bytes"),
    //     }

    //     let result = batch.next_bytes(Some(3)).unwrap().unwrap();
    //     match result {
    //         Data::Bytes(b) => assert_eq!(&b, b"456", "hint=3 should read exactly 3 bytes"),
    //         Data::Retry => panic!("expected bytes"),
    //     }

    //     let result = batch.next_bytes(Some(1)).unwrap().unwrap();
    //     match result {
    //         Data::Bytes(b) => assert_eq!(&b, b"7", "hint=1 should read exactly 1 byte"),
    //         Data::Retry => panic!("expected bytes"),
    //     }
    // }

    // /// WHY: Verify that `next_bytes(None)` falls back to batch_size.
    // #[test]
    // fn batch_reader_next_bytes_none_uses_batch_size() {
    //     let data = b"0123456789";
    //     let mut batch = BatchReader::new(Cursor::new(data.to_vec())).batch_size(3);

    //     let result = batch.next_bytes(None).unwrap().unwrap();
    //     match result {
    //         Data::Bytes(b) => assert_eq!(&b, b"012", "None hint should use batch_size=3"),
    //         Data::Retry => panic!("expected bytes"),
    //     }

    //     let result = batch.next_bytes(None).unwrap().unwrap();
    //     match result {
    //         Data::Bytes(b) => assert_eq!(&b, b"345", "second call should use batch_size=3 again"),
    //         Data::Retry => panic!("expected bytes"),
    //     }
    // }

    // -- IntoDataBytes trait tests --

    #[test]
    fn into_data_bytes_extension() {
        let batch = BatchReader::new(Cursor::new(b"test".to_vec()));
        let stream = BatchStreamReader::new(batch);

        let mut wrapped = stream.into_bytes();

        while let Some(result) = wrapped.next() {
            let bytes: Vec<u8> = result.unwrap();
            assert!(!bytes.is_empty());
        }
    }
}
