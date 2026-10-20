//! `SharedBuffer` and `SharedBufferReader::into_buffered_stream` tests, moved
//! out of `src/io/ioutils/mod.rs`.

use std::io::{Read, Write};

use foundation_core::io::ioutils::{SharedBuffer, SharedBufferReader, SharedBufferWriter};

#[test]
fn test_shared_buffer_write_and_read() {
    let (mut writer, mut reader) = SharedBuffer::split();

    writer.write_all(b"hello").unwrap();

    let mut buf = [0u8; 5];
    reader.read_exact(&mut buf).unwrap();

    assert_eq!(&buf, b"hello");
}

#[test]
fn test_shared_buffer_empty_read() {
    let (_, mut reader) = SharedBuffer::split();

    let mut buf = [0u8; 5];
    let bytes_read = reader.read(&mut buf).unwrap();

    assert_eq!(bytes_read, 0);
}

#[test]
fn test_shared_buffer_multiple_writes() {
    let (mut writer, mut reader) = SharedBuffer::split();

    writer.write_all(b"hello").unwrap();
    writer.write_all(b" ").unwrap();
    writer.write_all(b"world").unwrap();

    let mut buf = [0u8; 11];
    reader.read_exact(&mut buf).unwrap();

    assert_eq!(&buf, b"hello world");
}

#[test]
fn test_shared_buffer_partial_read() {
    let (mut writer, mut reader) = SharedBuffer::split();

    writer.write_all(b"hello world").unwrap();

    let mut buf = [0u8; 5];
    reader.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"hello");

    reader.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b" worl");
}

#[test]
fn test_shared_buffer_clone_writer() {
    let (mut writer, mut reader) = SharedBuffer::split();
    let mut writer2 = writer.clone();

    writer.write_all(b"hello").unwrap();
    writer2.write_all(b" ").unwrap();
    writer.write_all(b"world").unwrap();

    let mut buf = [0u8; 11];
    reader.read_exact(&mut buf).unwrap();

    assert_eq!(&buf, b"hello world");
}

#[test]
fn test_shared_buffer_clone_reader() {
    let (mut writer, mut reader) = SharedBuffer::split();
    let mut reader2 = reader.clone();

    writer.write_all(b"hello").unwrap();

    let mut buf1 = [0u8; 5];
    let mut buf2 = [0u8; 5];

    reader.read_exact(&mut buf1).unwrap();
    reader2.read_exact(&mut buf2).unwrap();

    assert_eq!(&buf1, b"hello");
    assert_eq!(&buf2, b"hello");
}

#[test]
fn test_shared_buffer_with_initial() {
    let buffer = SharedBuffer::with_initial(b"initial data");
    let (_, mut reader) = (
        SharedBufferWriter::new(buffer.clone_arc()),
        SharedBufferReader::new(buffer.clone_arc()),
    );

    let mut buf = [0u8; 12];
    reader.read_exact(&mut buf).unwrap();

    assert_eq!(&buf, b"initial data");
}

#[test]
fn test_shared_buffer_len() {
    let buffer = SharedBuffer::with_initial(b"hello");
    assert_eq!(buffer.len(), 5);

    let (mut writer, _) = SharedBuffer::split();
    assert_eq!(writer.clone_arc().lock().unwrap().len(), 0);

    writer.write_all(b"test").unwrap();
    assert_eq!(writer.clone_arc().lock().unwrap().len(), 4);
}

#[test]
fn test_shared_buffer_threaded() {
    use std::thread;

    let (mut writer, mut reader) = SharedBuffer::split();

    let handle = thread::spawn(move || {
        writer.write_all(b"from thread").unwrap();
    });

    thread::sleep(std::time::Duration::from_millis(10));

    let mut buf = [0u8; 11];
    reader.read_exact(&mut buf).unwrap();

    handle.join().unwrap();

    assert_eq!(&buf, b"from thread");
}

// ========================================================================
// Tests for into_buffered_stream() - SharedByteBufferStream integration
// ========================================================================

#[test]
fn test_into_buffered_stream_read() {
    let (mut writer, reader) = SharedBuffer::split();
    writer.write_all(b"hello world").unwrap();

    let mut stream = reader.into_buffered_stream();

    let mut buf = [0u8; 11];
    let bytes_read = stream.read(&mut buf).unwrap();

    assert_eq!(bytes_read, 11);
    assert_eq!(&buf, b"hello world");
}

#[test]
fn test_into_buffered_stream_read_line() {
    let (mut writer, reader) = SharedBuffer::split();
    writer.write_all(b"line 1\nline 2\nline 3\n").unwrap();

    let mut stream = reader.into_buffered_stream();

    let mut line = String::new();
    let bytes_read = stream.read_line(&mut line).unwrap();
    assert_eq!(bytes_read, 7); // "line 1\n"
    assert_eq!(line, "line 1\n");

    line.clear();
    let bytes_read = stream.read_line(&mut line).unwrap();
    assert_eq!(bytes_read, 7); // "line 2\n"
    assert_eq!(line, "line 2\n");

    line.clear();
    let bytes_read = stream.read_line(&mut line).unwrap();
    assert_eq!(bytes_read, 7); // "line 3\n"
    assert_eq!(line, "line 3\n");
}

#[test]
fn test_into_buffered_stream_sse_format() {
    // Simulate SSE format: "data: hello\n\n"
    let (mut writer, reader) = SharedBuffer::split();
    writer.write_all(b"data: hello\n\n").unwrap();

    let mut stream = reader.into_buffered_stream();

    // Read all data
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).unwrap();

    assert_eq!(buf, b"data: hello\n\n");
}

#[test]
fn test_into_buffered_stream_multiple_lines() {
    let (mut writer, reader) = SharedBuffer::split();
    writer.write_all(b"event: test\ndata: payload\n\n").unwrap();

    let mut stream = reader.into_buffered_stream();

    // Read line by line
    let mut line = String::new();

    stream.read_line(&mut line).unwrap();
    assert_eq!(line, "event: test\n");

    line.clear();
    stream.read_line(&mut line).unwrap();
    assert_eq!(line, "data: payload\n");

    line.clear();
    stream.read_line(&mut line).unwrap();
    assert_eq!(line, "\n");
}

#[test]
fn test_into_buffered_stream_empty_buffer() {
    let (_, reader) = SharedBuffer::split();
    let mut stream = reader.into_buffered_stream();

    let mut buf = [0u8; 5];
    let bytes_read = stream.read(&mut buf).unwrap();

    assert_eq!(bytes_read, 0);
}

#[test]
fn test_into_buffered_stream_partial_reads() {
    let (mut writer, reader) = SharedBuffer::split();
    writer.write_all(b"hello world").unwrap();

    let mut stream = reader.into_buffered_stream();

    // Read in chunks
    let mut buf = [0u8; 5];
    let bytes_read = stream.read(&mut buf).unwrap();
    assert_eq!(bytes_read, 5);
    assert_eq!(&buf, b"hello");

    let bytes_read = stream.read(&mut buf).unwrap();
    assert_eq!(bytes_read, 5);
    assert_eq!(&buf, b" worl");

    let bytes_read = stream.read(&mut buf).unwrap();
    assert_eq!(bytes_read, 1);
    assert_eq!(&buf[0..1], b"d");
}

#[test]
fn test_into_buffered_stream_read_exact() {
    let (mut writer, reader) = SharedBuffer::split();
    writer.write_all(b"exact data").unwrap();

    let mut stream = reader.into_buffered_stream();

    let mut buf = [0u8; 10];
    stream.read_exact(&mut buf).unwrap();

    assert_eq!(&buf, b"exact data");
}
