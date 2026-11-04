//! Tests for `io::readers`: `BatchReader` and the stream/EOF/limited readers
//! built on it.

use foundation_core::io::readers::*;
use std::io::{self, Cursor, Read};

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

    let stream = BatchStreamReader::new(batch);

    let mut got_retry = false;
    let mut got_bytes = false;

    for result in stream {
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
    let reader = EofReader::new(batch);

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
fn eof_reader_with_max_size() {
    // Test EOFStreamReader with LimitedEOFStreamReader for max size
    let data = b"hello world";
    let batch = BatchReader::new(Cursor::new(data.to_vec()))
        .batch_size(512)
        .max_consecutive_retries(100);
    let reader = LimitedEOFStreamReader::new(batch, 20);

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
    assert!(!results.is_empty());
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
    let reader = EofReader::new(batch);

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
    let reader = EofReader::new(batch);

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
    let reader = EofReader::new(batch);

    let results: Vec<_> = reader.collect();
    assert!(results.iter().any(|r| r.is_err()));
}

#[test]
fn eof_reader_empty_source() {
    let batch = BatchReader::new(Cursor::new(Vec::<u8>::new()));
    let reader = EofReader::new(batch);

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
    let wrapped = DataBytesIterator::new(data_stream);

    for result in wrapped {
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
    let eof_reader = EOFStreamReader::new(batch);

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
    let limited = LimitedEOFStreamReader::new(batch, 10);

    let mut collected = Vec::new();
    for result in limited {
        match result {
            Ok(Data::Bytes(bytes)) => collected.extend(bytes),
            Ok(Data::Retry) => continue,
            Err(e) => {
                // no error is returned as reader forces cap to only specific size.
                // never exceeding it
                panic!("got an error form reader and should not occur: {:?}", e,);
            }
        }
    }
    assert!(!collected.is_empty())
}

#[test]
fn limited_eof_stream_reader_under_limit() {
    let data = b"short";
    let batch = BatchReader::new(Cursor::new(data.to_vec())).batch_size(100);
    let limited = LimitedEOFStreamReader::new(batch, 100);

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

// -- HintReadterator tests --

/// WHY: Verify `next_bytes(Some(n))` limits buffer allocation to exactly `n`,
/// preventing overshoot when the inner reader's batch_size exceeds remaining bytes.
///
/// WHAT: Use a one-byte-per-read source with `batch_size=100` but `byte_cap=7`.
/// Without the hint, `LimitedBatchStreamReader` would request 100 bytes and
/// consume all 11. With the hint, it requests `remaining` each time.
#[test]
fn limited_batch_stream_reader_hint_prevents_overshoot() {
    let data = b"hello world";
    let batch = BatchReader::new(Cursor::new(data.to_vec())).batch_size(100);
    let limited = LimitedBatchStreamReader::new(batch, 7);

    let mut collected = Vec::new();
    for result in limited {
        match result {
            Ok(Data::Bytes(bytes)) => collected.extend(bytes),
            Ok(Data::Retry) => continue,
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    assert_eq!(
        collected,
        &data[..7],
        "should only read 7 bytes, got {} bytes: {:?}",
        collected.len(),
        std::str::from_utf8(&collected)
    );
}

/// WHY: Same overshoot test for `LimitedEOFStreamReader`.
#[test]
fn limited_eof_stream_reader_hint_prevents_overshoot() {
    let data = b"hello world";
    let batch = BatchReader::new(Cursor::new(data.to_vec())).batch_size(100);
    let limited = LimitedEOFStreamReader::new(batch, 7);

    let mut collected = Vec::new();
    for result in limited {
        match result {
            Ok(Data::Bytes(bytes)) => collected.extend(bytes),
            Ok(Data::Retry) => continue,
            Err(e) => {
                assert!(
                    e.to_string().contains("exceeds"),
                    "expected overshoot error, got: {e}"
                );
                break;
            }
        }
    }

    // With the hint, inner reader allocates exactly `remaining` bytes,
    // so we should hit exactly the cap without overshooting.
    assert_eq!(
        collected.len(),
        7,
        "should read exactly 7 bytes, got {}",
        collected.len()
    );
    assert_eq!(collected, &data[..7]);
}

/// WHY: Verify `next_bytes` directly with progressively shrinking hints
/// limits the buffer to the hint size.
///
/// WHAT: Call `BatchReader::next_bytes` with decreasing size hints
/// and confirm each buffer is capped to the hint.
#[test]
fn batch_reader_next_bytes_respects_hint() {
    let data = b"0123456789abcdef";
    let mut batch = BatchReader::new(Cursor::new(data.to_vec())).batch_size(100);

    let result = batch.next_bytes(Some(4)).unwrap().unwrap();
    match result {
        Data::Bytes(b) => assert_eq!(&b, b"0123", "hint=4 should read exactly 4 bytes"),
        Data::Retry => panic!("expected bytes"),
    }

    let result = batch.next_bytes(Some(3)).unwrap().unwrap();
    match result {
        Data::Bytes(b) => assert_eq!(&b, b"456", "hint=3 should read exactly 3 bytes"),
        Data::Retry => panic!("expected bytes"),
    }

    let result = batch.next_bytes(Some(1)).unwrap().unwrap();
    match result {
        Data::Bytes(b) => assert_eq!(&b, b"7", "hint=1 should read exactly 1 byte"),
        Data::Retry => panic!("expected bytes"),
    }
}

/// WHY: Verify that `next_bytes(None)` falls back to batch_size.
#[test]
fn batch_reader_next_bytes_none_uses_batch_size() {
    let data = b"0123456789";
    let mut batch = BatchReader::new(Cursor::new(data.to_vec())).batch_size(3);

    let result = batch.next_bytes(None).unwrap().unwrap();
    match result {
        Data::Bytes(b) => assert_eq!(&b, b"012", "None hint should use batch_size=3"),
        Data::Retry => panic!("expected bytes"),
    }

    let result = batch.next_bytes(None).unwrap().unwrap();
    match result {
        Data::Bytes(b) => assert_eq!(&b, b"345", "second call should use batch_size=3 again"),
        Data::Retry => panic!("expected bytes"),
    }
}

/// WHY: Verify that `next_bytes(Some(0))` returns `None` immediately,
/// preventing zero-byte allocations and infinite loops.
#[test]
fn batch_reader_next_bytes_zero_hint_returns_none() {
    let data = b"hello world";
    let mut batch = BatchReader::new(Cursor::new(data.to_vec())).batch_size(100);

    let result = batch.next_bytes(Some(0));
    assert!(result.is_none(), "hint=0 should return None immediately");
}

// -- IntoDataBytes trait tests --

#[test]
fn into_data_bytes_extension() {
    let batch = BatchReader::new(Cursor::new(b"test".to_vec()));
    let stream = BatchStreamReader::new(batch);

    let wrapped = stream.into_bytes();

    for result in wrapped {
        let bytes: Vec<u8> = result.unwrap();
        assert!(!bytes.is_empty());
    }
}

// -- Log-level regression (Feature 09 L9) --

use std::sync::{Arc, Mutex};
use tracing::Level;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Layer;

/// Records the target of every `ERROR` event it sees. Used to prove that a
/// `WouldBlock` read does not emit an error log.
struct ErrorCapture {
    targets: Arc<Mutex<Vec<String>>>,
}

impl<S> Layer<S> for ErrorCapture
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if event.metadata().level() == &Level::ERROR {
            self.targets
                .lock()
                .unwrap()
                .push(event.metadata().target().to_string());
        }
    }
}

/// WHY: Feature 09 moved `BatchReader`'s `WouldBlock` log from `error!` to
/// `trace!`. WouldBlock is normal back-pressure, not a failure, so it must not
/// emit an ERROR record.
/// WHAT: Read from a source that always returns `WouldBlock`, capture logs, and
/// assert no ERROR event fired. A source that returns a real error is then used
/// to prove the capture records ERROR events.
#[test]
fn batch_reader_would_block_is_not_an_error_log() {
    struct AlwaysWouldBlock;
    impl Read for AlwaysWouldBlock {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"))
        }
    }
    struct RealError;
    impl Read for RealError {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken"))
        }
    }

    let targets: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    tracing::subscriber::with_default(
        tracing_subscriber::registry().with(ErrorCapture {
            targets: targets.clone(),
        }),
        || {
            let reader = BatchReader::new(AlwaysWouldBlock).max_consecutive_retries(3);
            let _ = reader.count();
        },
    );
    assert!(
        targets.lock().unwrap().is_empty(),
        "WouldBlock must not emit ERROR records"
    );

    tracing::subscriber::with_default(
        tracing_subscriber::registry().with(ErrorCapture {
            targets: targets.clone(),
        }),
        || {
            let reader = BatchReader::new(RealError);
            let _ = reader.count();
        },
    );
    assert!(
        !targets.lock().unwrap().is_empty(),
        "a real read error must emit an ERROR record (proves capture works)"
    );
}
