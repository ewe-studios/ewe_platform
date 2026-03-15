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
//! - [`FullBodyReader`] — reads a known-size body with retry resilience
//! - [`BatchStreamReader`] — adapter that absorbs retries, yielding only bytes or errors
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
pub enum Data {
    /// A batch of bytes successfully read from the source.
    Bytes(Vec<u8>),
    /// Yielded on `WouldBlock`, `TimedOut`, or `Ok(0)`-when-not-EOF.
    /// Signals the caller should retry (poll again later).
    Retry,
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
        if self.done {
            return None;
        }

        let mut buf = vec![0u8; self.batch_size];
        match self.reader.read(&mut buf) {
            Ok(0) => {
                tracing::debug!("Zero bytes read occured");
                if self.eof_on_zero_read {
                    tracing::debug!("Stream is now considered finished");
                    self.done = true;
                    None
                } else {
                    self.consecutive_retries += 1;
                    if self.consecutive_retries > self.max_consecutive_retries {
                        self.done = true;

                        // if data was received then we know maybe its
                        // just really EOF. Let the data interpreter decides.
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
                tracing::debug!("Received data Bytes(len={})", n);
                self.received_data = true;
                self.consecutive_retries = 0;
                buf.truncate(n);
                Some(Ok(Data::Bytes(buf)))
            }
            Err(e)
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut =>
            {
                tracing::error!("Timeout/WouldBlock error received: {:?}", &e);
                self.consecutive_retries += 1;
                if self.consecutive_retries > self.max_consecutive_retries {
                    self.done = true;

                    // if data was received then we know maybe its
                    // just really EOF. Let the data interpreter decides.
                    if self.received_data {
                        return None;
                    }

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

/// WHY: When the body size is known (via Content-Length), we need to read
/// exactly that many bytes without `read_exact()` swallowing transient errors.
///
/// WHAT: Reads a body of known total size using `read()` with retry handling.
///
/// HOW: Uses [`BatchReader`] internally with `eof_on_zero_read=true`, collecting
/// all bytes until EOF or the expected size is reached. Respects `max_consecutive_retries`
/// for transient failures.
pub struct FullBodyReader(usize);

impl FullBodyReader {
    #[must_use]
    pub fn new(batch_size: usize) -> Self {
        Self(batch_size)
    }
}

impl Default for FullBodyReader {
    fn default() -> Self {
        Self(8192)
    }
}

impl FullBodyReader {
    /// Read exactly `total_size` bytes from `reader` with TCP-resilient retry handling.
    ///
    /// Unlike `read_exact()`, this propagates meaningful errors on `WouldBlock`/`TimedOut`
    /// rather than converting them to `UnexpectedEof`.
    ///
    /// # Errors
    /// - `UnexpectedEof` — stream returned 0 bytes before `total_size` was reached.
    /// - `WouldBlock`/`TimedOut` — retry limit exceeded without progress.
    /// - Any other `io::Error` from the underlying reader.
    ///
    /// # Panics
    /// Never panics.
    pub fn read_full<R: Read>(
        &self,
        reader: &mut R,
        total_size: usize,
        max_retries: usize,
    ) -> Result<Vec<u8>, io::Error> {
        let batch_reader = BatchReader::new(reader)
            .batch_size(self.0)
            .eof_on_zero_read(true)
            .max_consecutive_retries(max_retries);

        let mut result = Vec::with_capacity(total_size);
        for batch_result in batch_reader {
            match batch_result {
                Ok(Data::Bytes(bytes)) => {
                    tracing::debug!("Received Data::Bytes(len={})", bytes.len());
                    result.extend(bytes);
                }
                Ok(Data::Retry) => {
                    // This shouldn't happen with eof_on_zero_read=true unless
                    // we get WouldBlock/TimedOut - just continue retrying
                    tracing::debug!("Received Data::Retry - will retry read");
                }
                Err(e) => {
                    tracing::error!("Read error occured: {:?}", &e);
                    return Err(e);
                }
            }
        }

        // BatchReader returned None (EOF)
        if result.len() != total_size {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "unexpected EOF: read {} of {} bytes",
                    result.len(),
                    total_size
                ),
            ));
        }

        Ok(result)
    }
}

/// WHY: `SendSafeBody::Stream` needs an iterator of `Result<Vec<u8>, BoxedError>`.
/// `BatchReader` yields `Result<Data, io::Error>` where `Data` includes `Retry`
/// variants — stream consumers shouldn't need to handle retry logic.
///
/// WHAT: Adapter from [`BatchReader`] to `Iterator<Item = Result<Vec<u8>, BoxedError>>`.
///
/// HOW: Internally spins on `Data::Retry` results, only yielding when it gets
/// actual bytes or an error. The retry budget is enforced by the inner `BatchReader`.
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
    type Item = Result<Vec<u8>, Box<dyn std::error::Error + 'static>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some(Ok(Data::Retry)) => {}
                Some(Ok(Data::Bytes(bytes))) => return Some(Ok(bytes)),
                Some(Err(e)) => return Some(Err(Box::new(e))),
                None => return None,
            }
        }
    }
}

/// WHY: Reading until EOF on TCP streams requires handling `WouldBlock`/`TimedOut`
/// errors gracefully rather than treating them as fatal. Using `read()` directly
/// loses retry state and buffer management.
///
/// WHAT: Reads an unknown-size body (until EOF) with TCP-resilient retry handling.
///
/// HOW: Uses [`BatchReader`] internally with `eof_on_zero_read=true`, accumulating
/// all bytes into a `Vec<u8>`. Respects `max_consecutive_retries` for transient
/// failures and supports an optional maximum size limit.
pub struct EofReader;

impl EofReader {
    /// Read until EOF from `reader` with TCP-resilient retry handling.
    ///
    /// Unlike `read_to_end()`, this propagates meaningful errors on `WouldBlock`/`TimedOut`
    /// rather than treating them as fatal.
    ///
    /// # Arguments
    /// - `reader` - The `Read` source to read from
    /// - `batch_size` - Size of each read batch (default: 8192 if not specified)
    /// - `max_retries` - Maximum consecutive retries for WouldBlock/TimedOut
    /// - `max_size` - Optional maximum size limit. Returns error if body exceeds this.
    ///
    /// # Errors
    /// - `WouldBlock`/`TimedOut` — retry limit exceeded without progress.
    /// - `InvalidInput` — body exceeds `max_size` if provided.
    /// - Any other `io::Error` from the underlying reader.
    ///
    /// # Panics
    /// Never panics.
    pub fn read_to_end<R: Read>(
        reader: &mut R,
        batch_size: usize,
        max_retries: usize,
        max_size: Option<usize>,
    ) -> Result<Vec<u8>, io::Error> {
        let mut result = Vec::with_capacity(1024);
        let batch_reader = BatchReader::new(reader)
            .batch_size(batch_size)
            .eof_on_zero_read(true)
            .max_consecutive_retries(max_retries);

        for batch_result in batch_reader {
            match batch_result {
                Ok(Data::Bytes(bytes)) => {
                    tracing::debug!("Received Data::Bytes(len={})", bytes.len());
                    if let Some(max) = max_size {
                        if result.len() + bytes.len() > max {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidInput,
                                format!(
                                    "body size {} exceeds max {}",
                                    result.len() + bytes.len(),
                                    max
                                ),
                            ));
                        }
                    }
                    result.extend(bytes);
                }
                Ok(Data::Retry) => {
                    tracing::debug!("Received Data::Retry - will retry read");
                }
                Err(e) => {
                    tracing::error!("Read error occured: {:?}", &e);
                    return Err(e);
                }
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // -- BatchReader tests --

    #[test]
    fn batch_reader_normal_reads() {
        let data = b"hello world";
        let reader = BatchReader::new(Cursor::new(data.to_vec())).batch_size(5);
        let results: Vec<_> = reader.collect();

        // Should get "hello", " worl", "d", then EOF
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
        // Custom reader that returns WouldBlock then data
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

        // 3 retries + 1 error
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
                    Ok(0) // now we want real EOF but eof_on_zero_read is false
                }
            }
        }

        let reader = BatchReader::new(ZeroThenData { calls: 0 })
            .eof_on_zero_read(false)
            .max_consecutive_retries(5);

        // Take first 4 items
        let results: Vec<_> = reader.take(4).collect();
        assert_eq!(results.len(), 4);
        // First two should be Retry, third should be Bytes
        assert!(matches!(results[0].as_ref().unwrap(), Data::Retry));
        assert!(matches!(results[1].as_ref().unwrap(), Data::Retry));
        assert!(matches!(results[2].as_ref().unwrap(), Data::Bytes(_)));
    }

    // -- FullBodyReader tests --

    #[test]
    fn full_body_reader_complete_read() {
        let data = b"hello world";
        let mut cursor = Cursor::new(data.to_vec());
        let result = FullBodyReader::default().read_full(&mut cursor, data.len(), 10);
        assert_eq!(result.unwrap(), data);
    }

    #[test]
    fn full_body_reader_partial_reads() {
        // Reader that gives 1 byte at a time
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
        let mut reader = OneByteReader {
            data: data.to_vec(),
            pos: 0,
        };
        let result = FullBodyReader::default().read_full(&mut reader, data.len(), 10);
        assert_eq!(result.unwrap(), data);
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
                if self.calls % 3 == 0 {
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
        let mut reader = RetryReader {
            data: data.to_vec(),
            pos: 0,
            calls: 0,
        };
        let result = FullBodyReader::default().read_full(&mut reader, data.len(), 10);
        assert_eq!(result.unwrap(), data);
    }

    #[test]
    fn full_body_reader_unexpected_eof() {
        let data = b"hi";
        let mut cursor = Cursor::new(data.to_vec());
        let result = FullBodyReader::default().read_full(&mut cursor, 10, 5);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn full_body_reader_retry_limit_exceeded() {
        struct AlwaysWouldBlock;
        impl Read for AlwaysWouldBlock {
            fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"))
            }
        }

        let result = FullBodyReader::default().read_full(&mut AlwaysWouldBlock, 10, 3);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);
    }

    // -- BatchStreamReader tests --

    #[test]
    fn batch_stream_reader_absorbs_retries() {
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

        let stream = BatchStreamReader::new(batch);
        let results: Vec<_> = stream.collect();

        // Should only get bytes, no retries exposed
        for r in &results {
            assert!(r.is_ok());
        }
        let all_bytes: Vec<u8> = results.into_iter().flat_map(|r| r.unwrap()).collect();
        assert_eq!(all_bytes, b"hello");
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

    // -- EofReader tests --

    #[test]
    fn eof_reader_complete_read() {
        let data = b"hello world test data";
        let result = EofReader::read_to_end(&mut Cursor::new(data.to_vec()), 512, 100, None);
        assert_eq!(result.unwrap(), data);
    }

    #[test]
    fn eof_reader_with_max_size() {
        let data = b"hello world";
        let result = EofReader::read_to_end(&mut Cursor::new(data.to_vec()), 512, 100, Some(20));
        assert_eq!(result.unwrap(), data);

        let result = EofReader::read_to_end(&mut Cursor::new(data.to_vec()), 512, 100, Some(5));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidInput);
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
        let result = EofReader::read_to_end(
            &mut OneByteReader {
                data: data.to_vec(),
                pos: 0,
            },
            512,
            100,
            None,
        );
        assert_eq!(result.unwrap(), data);
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
                if self.calls % 3 == 0 {
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
        let result = EofReader::read_to_end(
            &mut RetryReader {
                data: data.to_vec(),
                pos: 0,
                calls: 0,
            },
            512,
            100,
            None,
        );
        assert_eq!(result.unwrap(), data);
    }

    #[test]
    fn eof_reader_retry_limit_exceeded() {
        struct AlwaysWouldBlock;
        impl Read for AlwaysWouldBlock {
            fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"))
            }
        }

        let result = EofReader::read_to_end(&mut AlwaysWouldBlock, 512, 3, None);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);
    }

    #[test]
    fn eof_reader_empty_source() {
        let result = EofReader::read_to_end(&mut Cursor::new(Vec::<u8>::new()), 512, 100, None);
        assert_eq!(result.unwrap(), Vec::<u8>::new());
    }
}
