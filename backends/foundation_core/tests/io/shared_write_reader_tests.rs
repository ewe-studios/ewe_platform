//! `SharedWriteReader` tests, moved out of `src/io/ioutils/mod.rs`.

use std::io::{Cursor, Write};
use std::sync::{Arc, Mutex};

use foundation_core::io::ioutils::SharedWriteReader;

#[test]
fn test_shared_write_reader_write_and_read_position() {
    let cursor = Cursor::new(Vec::new());
    let shared = SharedWriteReader::new(cursor);

    let mut writer = shared.clone();
    writer
        .write_all(b"hello world")
        .expect("write should succeed");

    let arc = shared.clone_arc();
    let inner = arc.lock().expect("lock should not be poisoned");
    assert_eq!(inner.get_ref(), b"hello world");
}

#[test]
fn test_shared_write_reader_multiple_writers() {
    let cursor = Cursor::new(Vec::new());
    let shared = SharedWriteReader::new(cursor);

    let mut w1 = shared.clone();
    let mut w2 = shared.clone();

    w1.write_all(b"hello ").expect("write should succeed");
    w2.write_all(b"world").expect("write should succeed");

    let arc = shared.clone_arc();
    let inner = arc.lock().expect("lock should not be poisoned");
    let data = inner.get_ref();
    assert_eq!(data, b"hello world");
}

#[test]
fn test_shared_write_reader_from_arc() {
    let arc = Arc::new(Mutex::new(Cursor::new(Vec::new())));
    let shared = SharedWriteReader::from_arc(arc.clone());

    let mut writer = shared.clone();
    writer
        .write_all(b"test data")
        .expect("write should succeed");

    let guard = arc.lock().expect("lock should not be poisoned");
    let data = guard.get_ref();
    assert_eq!(data, b"test data");
}

#[test]
fn test_shared_write_reader_with_cursor() {
    let cursor = Cursor::new(Vec::new());
    let shared = SharedWriteReader::new(cursor);

    let mut w1 = shared.clone();
    w1.write_all(b"hello").expect("write should succeed");

    let r1 = shared.clone();
    let arc = r1.clone_arc();
    let inner = arc.lock().expect("lock not poisoned");
    assert_eq!(inner.get_ref(), b"hello");
}

#[test]
fn test_shared_write_reader_threaded() {
    let cursor = Cursor::new(Vec::new());
    let shared = SharedWriteReader::new(cursor);

    let mut handles = vec![];
    for i in 0..3 {
        let s = shared.clone();
        let handle = std::thread::spawn(move || {
            let mut writer = s.clone();
            let msg = format!("thread_{}", i);
            writer
                .write_all(msg.as_bytes())
                .expect("write should succeed");
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("thread should join");
    }

    let arc = shared.clone_arc();
    let inner = arc.lock().expect("lock not poisoned");
    let data = inner.get_ref();
    assert!(data.starts_with(b"thread_"));
}
