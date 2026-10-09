use std::io::{Cursor, Read};
use std::sync::Arc;
use std::sync::Mutex;

use foundation_core::io::ioutils::SharedByteBufferStream;

#[test]
fn probe_empty_returns_zero_on_eof() {
    let cursor = Cursor::new(Vec::<u8>::new());
    let mut stream = SharedByteBufferStream::ref_cell(cursor);

    assert_eq!(stream.probe().unwrap(), 0);
}

#[test]
fn probe_small_data_returns_correct_length() {
    let data = b"hello world";
    let cursor = Cursor::new(data.to_vec());
    let mut stream = SharedByteBufferStream::ref_cell(cursor);

    let len = stream.probe().unwrap();
    assert_eq!(len, data.len());
}

#[test]
fn probe_does_not_consume_buffered_data() {
    let data = b"hello";
    let cursor = Cursor::new(data.to_vec());
    let mut stream = SharedByteBufferStream::ref_cell(cursor);

    let len = stream.probe().unwrap();
    assert_eq!(len, 5);

    // Probe again — should still report 5 because data wasn't consumed.
    let len2 = stream.probe().unwrap();
    assert_eq!(len2, 5);
}

#[test]
fn probe_after_consume_triggers_refill() {
    let data = b"first_chunk_second";
    let cursor = Cursor::new(data.to_vec());
    let mut stream = SharedByteBufferStream::ref_cell(cursor);

    // Consume all buffered data via read.
    let mut buf = vec![0u8; 64];
    let n = stream.read(&mut buf).unwrap();

    // After consuming, probe should try to read more — but cursor is exhausted.
    let probe_len = stream.probe().unwrap();
    assert_eq!(probe_len, 0);

    // Verify we read all the data.
    assert_eq!(&buf[..n], data);
}

#[test]
fn probe_skips_fillup_when_data_available() {
    let data = b"hello world";
    let cursor = Cursor::new(data.to_vec());
    let mut stream = SharedByteBufferStream::ref_cell(cursor);

    let len1 = stream.probe().unwrap();
    assert_eq!(len1, data.len());

    // Consume only part of the data.
    let mut buf = vec![0u8; 5];
    let n = stream.read(&mut buf).unwrap();
    assert_eq!(n, 5);

    // Probe should see remaining data without reading more.
    let len2 = stream.probe().unwrap();
    assert_eq!(len2, data.len() - 5);
}

#[test]
fn probe_multiple_chunks_until_exhausted() {
    let data = b"chunk1chunk2chunk3";
    let cursor = Cursor::new(data.to_vec());
    let mut stream = SharedByteBufferStream::ref_cell(cursor);

    let mut total_read = 0;
    loop {
        let available = stream.probe().unwrap();
        if available == 0 {
            break;
        }
        total_read += available;
        let mut buf = vec![0u8; available];
        let n = stream.read_exact(&mut buf);
        match n {
            Ok(()) => {}
            Err(_) => break,
        }
    }
    assert_eq!(total_read, data.len());
}

#[test]
fn probe_sync_variant() {
    let data = b"sync_probe_test";
    let stream = SharedByteBufferStream::sync(&Arc::new(Mutex::new(Cursor::new(data.to_vec()))));
    let mut stream = stream;

    let len = stream.probe().unwrap();
    assert_eq!(len, data.len());
}

#[test]
fn probe_rwrite_variant() {
    let data = b"rwrite_probe_test";
    let mut stream = SharedByteBufferStream::rwrite(Cursor::new(data.to_vec()));

    let len = stream.probe().unwrap();
    assert_eq!(len, data.len());
}

#[test]
fn probe_large_data() {
    let data = vec![0xABu8; 4096];
    let cursor = Cursor::new(data.clone());
    let mut stream = SharedByteBufferStream::ref_cell(cursor);

    let len = stream.probe().unwrap();
    assert_eq!(len, data.len());
}
