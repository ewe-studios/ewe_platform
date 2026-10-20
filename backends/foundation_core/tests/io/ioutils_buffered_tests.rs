//! `BufferedReader` / `BufferedWriter` peek tests, moved out of
//! `src/io/ioutils/mod.rs`.

use std::io::Cursor;

use foundation_core::io::ioutils::{BufferedReader, BufferedWriter, PeekableReadStream};

#[test]
fn can_buffered_reader_peek() {
    let content = b"alexander_wonderbat";
    let mut reader = BufferedReader::new(&content[..]);

    let mut content_to_read = vec![0; 5];
    reader
        .peek(&mut content_to_read)
        .expect("should read data correctly");

    assert_eq!(b"alexa", &content_to_read[..]);

    assert_eq!(content, reader.buffer());
}

#[test]
fn can_buffered_writer_peek() {
    let content = b"alexander_wonderbat";
    let mut reader = BufferedReader::new(BufferedWriter::new(Cursor::new(content.to_vec())));

    let mut content_to_read = vec![0; 5];
    reader
        .peek(&mut content_to_read)
        .expect("should read data correctly");

    assert_eq!(b"alexa", &content_to_read[..]);

    assert_eq!(content, reader.buffer());
}
