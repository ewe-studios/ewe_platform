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
                // NOTE: never trace `buf` contents here — this is the raw body
                // stream (e.g. multi-GB model downloads), so dumping it explodes
                // the logs and leaks payload bytes. The length above is enough.
                Some(Ok(Data::Bytes(buf)))
            }
            Err(e)
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut =>
            {
                // WouldBlock is normal back-pressure, not an error; TimedOut is a
                // stall worth a debug line but still not an error.
                if e.kind() == io::ErrorKind::WouldBlock {
                    tracing::trace!("WouldBlock received: {:?}", &e);
                } else {
                    tracing::debug!("TimedOut received: {:?}", &e);
                }
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
        tracing::trace!(remaining = %remaining, "Remaining bytes to read: {}", &remaining);
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
