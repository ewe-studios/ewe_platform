//! `ByteBufferPointer` cursor, peek and fill tests, moved out of
//! `src/io/ioutils/mod.rs`. Read-path tests live in
//! `byte_buffer_pointer_read_tests.rs`.

use std::io::Cursor;
use std::sync::{Arc, Mutex};

use foundation_core::io::ioutils::{
    BufferedReader, BufferedWriter, ByteBufferPointer, OwnedReader, PeekState,
};

#[test]
fn fails_read_all_the_data_when_limit_is_reached() {
    let content = b"alexander_wonderbat";

    let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
    let buffer = Mutex::new(ByteBufferPointer::new(5, reader));

    let mut binding = buffer.lock().unwrap();

    let mut read_data = Vec::new();

    let read_length = binding.read_all(&mut read_data, Some(2));
    assert!(read_length.is_err())
}

#[test]
fn can_read_all_the_data() {
    let content = b"alexander_wonderbat";

    let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
    let buffer = Mutex::new(ByteBufferPointer::new(5, reader));

    let mut binding = buffer.lock().unwrap();

    let mut read_data = Vec::new();

    let read_length = binding
        .read_all(&mut read_data, None)
        .expect("read data successfully");
    assert_eq!(read_length, content.len());

    let data_4_str = str::from_utf8(&read_data[0..read_length]).expect("convert into string");
    assert_eq!(data_4_str, "alexander_wonderbat");
}

#[test]
fn can_read_bytes_until_found() {
    const NO_BAR: &[u8] = b"_";
    let content = b"alexander_wonderbat\n";

    let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
    let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

    let mut binding = buffer.lock().unwrap();

    let mut new_line = Vec::new();
    let read = binding.read_bytes_until(NO_BAR, &mut new_line);

    assert_eq!(10, read.unwrap());
    let data4 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
    assert_eq!(new_line, data4);
}

#[test]
fn can_read_bytes_until_not_found() {
    const NO_BAR: &[u8] = b"-";
    let content = b"alexander_wonderbat\n";

    let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
    let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

    let mut binding = buffer.lock().unwrap();

    let mut new_line = Vec::new();
    let read = binding.read_bytes_until(NO_BAR, &mut new_line);

    assert_eq!(content.len(), read.unwrap());
    let data4 = vec![
        97, 108, 101, 120, 97, 110, 100, 101, 114, 95, 119, 111, 110, 100, 101, 114, 98, 97, 116,
        10,
    ];
    assert_eq!(new_line, data4);
}

#[test]
fn can_peek_line() {
    let content = b"alexander_wonderbat\n";

    let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
    let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

    let mut binding = buffer.lock().unwrap();

    let mut new_line = String::new();
    let read = binding.next_line(&mut new_line);

    assert_eq!(content.len(), read.unwrap());
    assert_eq!(new_line, "alexander_wonderbat\n");

    let data4 = vec![
        97, 108, 101, 120, 97, 110, 100, 101, 114, 95, 119, 111, 110, 100, 101, 114, 98, 97, 116,
        10,
    ];
    assert_eq!(binding.scan(), &data4);
}

#[test]
fn can_read_line() {
    let content = b"alexander_wonderbat\n";

    let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
    let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

    let mut binding = buffer.lock().unwrap();

    let mut new_line = String::new();
    let read = binding.read_line(&mut new_line);

    assert_eq!(content.len(), read.unwrap());
    assert_eq!(new_line, "alexander_wonderbat\n");
}

#[test]
fn can_peek_until_a_signal_and_consume() {
    const SIGNAL: &[u8] = b"_";

    let content = b"alexander_wonderbat";

    let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
    let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

    let mut binding = buffer.lock().unwrap();
    assert!(matches!(
        binding.next_until(SIGNAL).expect("should peek"),
        PeekState::Request(_)
    ));

    let result = binding.consume();
    let content = result.unwrap();

    let data4 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
    assert_eq!(content, data4);

    let data_4_str = str::from_utf8(&data4).expect("convert into string");
    assert_eq!(data_4_str, "alexander_");
}

#[test]
fn can_peek_until_a_signal() {
    const SIGNAL: &[u8] = b"_";

    let content = b"alexander_wonderbat";

    let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
    let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

    let mut binding = buffer.lock().unwrap();
    let result = binding.next_until(SIGNAL);
    assert!(matches!(result, Ok(PeekState::Request(_))));

    let PeekState::Request(content) = result.unwrap() else {
        panic!("Failed expectation")
    };

    let data4 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
    assert_eq!(content, &data4);

    let data_4_str = str::from_utf8(&data4).expect("convert into string");
    assert_eq!(data_4_str, "alexander_");
}

#[test]
fn can_peek_until_a_signal_but_not_found() {
    const SIGNAL: &[u8] = b"-";

    let content = b"alexander_wonderbat";

    let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
    let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

    let mut binding = buffer.lock().unwrap();
    let result = binding.next_until(SIGNAL);
    assert!(matches!(result, Ok(PeekState::EndOfFile)));
}

#[test]
fn can_take_peeked_data_with_multi_mutex() {
    let content = b"alexander_wonderbat";

    let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
    let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

    assert_eq!(
        buffer.lock().unwrap().peek().expect("less than requested"),
        PeekState::LessThanRequested
    );

    buffer.lock().unwrap().fill_up().expect("should fill up");

    let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
    assert_eq!(
        buffer.lock().unwrap().peek_by(10).expect("capture 3"),
        PeekState::Request(data3.as_ref())
    );

    assert_eq!(
        buffer.lock().unwrap().forward_by(9).expect("capture 3"),
        PeekState::Continue
    );

    assert_eq!(buffer.lock().unwrap().scan(), &data3[0..9]);
    assert_eq!(buffer.lock().unwrap().full_scan(), &data3[0..10]);

    assert_eq!(
        buffer.lock().unwrap().forward_by(10).expect("capture 4"),
        PeekState::EndOfBuffered
    );

    let amt = buffer.lock().unwrap().fill_up().expect("should fill up");

    assert_eq!(
        buffer.lock().unwrap().forward_by(amt).expect("capture 3"),
        PeekState::Continue
    );

    let data4 = vec![
        97, 108, 101, 120, 97, 110, 100, 101, 114, 95, 119, 111, 110, 100, 101, 114, 98, 97, 116,
    ];
    assert_eq!(buffer.lock().unwrap().scan(), &data4);
}

#[test]
fn can_read_data_via_byte_buffer_pointer() {
    let content = b"alexander_wonderbat";
    let mut source = Cursor::new(content.to_vec());
    let reader = OwnedReader::atomic(&mut source);
    let mut buffer = ByteBufferPointer::new(128, reader);

    assert_eq!(
        buffer.peek().expect("less than requested"),
        PeekState::LessThanRequested
    );

    buffer.fill_up().expect("should fill up");

    let data = vec![97];

    assert_eq!(
        buffer.peek().expect("capture 1"),
        PeekState::Request(data.as_ref())
    );

    let data2 = vec![97, 108];
    assert_eq!(
        buffer.peek_by(2).expect("capture 2"),
        PeekState::Request(data2.as_ref())
    );

    let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
    assert_eq!(
        buffer.peek_by(10).expect("capture 3"),
        PeekState::Request(data3.as_ref())
    );
}

#[test]
fn can_take_peeked_data() {
    let content = b"alexander_wonderbat";
    let mut source = Cursor::new(content.to_vec());
    let reader = OwnedReader::atomic(&mut source);
    let mut buffer = ByteBufferPointer::new(128, reader);

    assert_eq!(
        buffer.peek().expect("less than requested"),
        PeekState::LessThanRequested
    );

    buffer.fill_up().expect("should fill up");

    let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
    assert_eq!(
        buffer.peek_by(10).expect("capture 3"),
        PeekState::Request(data3.as_ref())
    );
}

#[test]
fn can_take_unmove_data() {
    let content = b"alexander_wonderbat";
    let mut source = Cursor::new(content.to_vec());
    let reader = OwnedReader::atomic(&mut source);
    let mut buffer = ByteBufferPointer::new(128, reader);

    assert_eq!(
        buffer.peek().expect("less than requested"),
        PeekState::LessThanRequested
    );

    buffer.fill_up().expect("should fill up");

    let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
    assert_eq!(
        buffer.peek_by(10).expect("capture 3"),
        PeekState::Request(data3.as_ref())
    );

    assert_eq!(
        buffer.forward_by(10).expect("capture 3"),
        PeekState::Continue
    );

    assert_eq!(buffer.scan(), &data3[0..10]);

    assert_eq!(
        buffer.unforward_by(5).expect("capture 3"),
        PeekState::Continue
    );

    assert_eq!(buffer.scan(), &data3[0..5]);
}

#[test]
fn can_skip_data() {
    let content = b"alexander_wonderbat";
    let mut source = Cursor::new(content.to_vec());
    let reader = OwnedReader::atomic(&mut source);
    let mut buffer = ByteBufferPointer::new(128, reader);

    assert_eq!(
        buffer.peek().expect("less than requested"),
        PeekState::LessThanRequested
    );

    buffer.fill_up().expect("should fill up");

    let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
    assert_eq!(
        buffer.peek_by(10).expect("capture 3"),
        PeekState::Request(data3.as_ref())
    );

    assert_eq!(
        buffer.forward_by(10).expect("capture 3"),
        PeekState::Continue
    );

    assert_eq!(buffer.scan(), &data3[0..10]);

    assert_eq!(
        buffer.unforward_by(5).expect("capture 3"),
        PeekState::Continue
    );

    buffer.skip();

    let data4: Vec<u8> = vec![];
    assert_eq!(buffer.scan(), &data4);

    let data5: Vec<u8> = vec![110, 100, 101, 114, 95];
    assert_eq!(
        buffer.peek_by(5).expect("capture 3"),
        PeekState::Request(data5.as_ref())
    );
}

#[test]
fn can_use_buffered_reader_move_next_data() {
    let content = b"alexander_wonderbat";
    let mut reader = BufferedReader::new(BufferedWriter::new(Cursor::new(content.to_vec())));
    let mut buffer = ByteBufferPointer::new(128, OwnedReader::atomic(&mut reader));

    assert_eq!(
        buffer.peek().expect("less than requested"),
        PeekState::LessThanRequested
    );

    buffer.fill_up().expect("should fill up");

    let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
    assert_eq!(0, buffer.data_cursor());
    assert_eq!(0, buffer.peek_cursor());
    assert_eq!(
        buffer.peek_by(10).expect("capture 3"),
        PeekState::Request(data3.as_ref())
    );
    assert_eq!(
        buffer.forward_by(10).expect("capture 3"),
        PeekState::Continue
    );
    assert_eq!(buffer.scan(), &data3[0..10]);
    assert!(!buffer.greater_than_40_percent());
    assert_eq!(buffer.consume().expect("capture 3"), data3.clone());
    assert!(buffer.greater_than_40_percent());
}

#[test]
fn can_move_next_data() {
    let content = b"alexander_wonderbat";
    let mut source = Cursor::new(content.to_vec());
    let reader = OwnedReader::atomic(&mut source);
    let mut buffer = ByteBufferPointer::new(128, reader);

    assert_eq!(
        buffer.peek().expect("less than requested"),
        PeekState::LessThanRequested
    );

    buffer.fill_up().expect("should fill up");

    let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
    assert_eq!(0, buffer.data_cursor());
    assert_eq!(0, buffer.peek_cursor());
    assert_eq!(
        buffer.peek_by(10).expect("capture 3"),
        PeekState::Request(data3.as_ref())
    );
    assert_eq!(
        buffer.forward_by(10).expect("capture 3"),
        PeekState::Continue
    );
    assert_eq!(buffer.scan(), &data3[0..10]);
    assert!(!buffer.greater_than_40_percent());
    assert_eq!(buffer.consume().expect("capture 3"), data3.clone());
    assert!(buffer.greater_than_40_percent());
}

#[test]
fn can_pull_more_data() {
    let content = b"alexander_wonderbat";

    let mut source = Cursor::new(content.to_vec());
    let reader = OwnedReader::atomic(&mut source);
    let mut buffer = ByteBufferPointer::new(10, reader);

    assert_eq!(
        buffer.peek().expect("less than requested"),
        PeekState::LessThanRequested
    );

    buffer.fill_up().expect("should fill up");

    let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
    assert_eq!(
        buffer.peek_by(10).expect("capture 3"),
        PeekState::Request(data3.as_ref())
    );

    assert_eq!(
        buffer.forward_by(9).expect("capture 3"),
        PeekState::Continue
    );

    assert_eq!(buffer.scan(), &data3[0..9]);
    assert_eq!(buffer.full_scan(), &data3[0..10]);

    assert_eq!(
        buffer.forward_by(10).expect("capture 4"),
        PeekState::EndOfBuffered
    );

    let amt = buffer.fill_up().expect("should fill up");

    assert_eq!(
        buffer.forward_by(amt).expect("capture 3"),
        PeekState::Continue
    );

    let data4 = vec![
        97, 108, 101, 120, 97, 110, 100, 101, 114, 95, 119, 111, 110, 100, 101, 114, 98, 97, 116,
    ];
    assert_eq!(buffer.scan(), &data4);
}

#[test]
fn can_pull_more_data_with_mutex() {
    let content = b"alexander_wonderbat";

    let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
    let mut buffer = ByteBufferPointer::new(10, reader);

    assert_eq!(
        buffer.peek().expect("less than requested"),
        PeekState::LessThanRequested
    );

    buffer.fill_up().expect("should fill up");

    let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
    assert_eq!(
        buffer.peek_by(10).expect("capture 3"),
        PeekState::Request(data3.as_ref())
    );

    assert_eq!(
        buffer.forward_by(9).expect("capture 3"),
        PeekState::Continue
    );

    assert_eq!(buffer.scan(), &data3[0..9]);
    assert_eq!(buffer.full_scan(), &data3[0..10]);

    assert_eq!(
        buffer.forward_by(10).expect("capture 4"),
        PeekState::EndOfBuffered
    );

    let amt = buffer.fill_up().expect("should fill up");

    assert_eq!(
        buffer.forward_by(amt).expect("capture 3"),
        PeekState::Continue
    );

    let data4 = vec![
        97, 108, 101, 120, 97, 110, 100, 101, 114, 95, 119, 111, 110, 100, 101, 114, 98, 97, 116,
    ];
    assert_eq!(buffer.scan(), &data4);
}
