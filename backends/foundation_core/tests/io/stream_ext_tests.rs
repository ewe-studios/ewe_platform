use std::io::Cursor;
use std::sync::Arc;

use foundation_core::io::buffer_pool::BytesPool;
use foundation_core::io::stream_ext::ReadStreamExt;

use bytes::BytesMut;

#[test]
/// WHY: read_into_bytes should read up to max_len bytes
/// WHAT: Data is read directly into BytesMut
fn test_read_into_bytes() {
    let mut cursor = Cursor::new(b"hello world");
    let mut buf = BytesMut::with_capacity(1024);

    let n = cursor.read_into_bytes(&mut buf, 5).unwrap();
    assert_eq!(n, 5);
    assert_eq!(&buf[..], b"hello");
}

#[test]
/// WHY: read_exact_into_bytes should read exactly len bytes
/// WHAT: Buffer contains exactly len bytes after read
fn test_read_exact_into_bytes() {
    let mut cursor = Cursor::new(b"hello world");
    let buf = cursor.read_exact_into_bytes(5).unwrap();

    assert_eq!(buf.len(), 5);
    assert_eq!(&buf[..], b"hello");
}

#[test]
/// WHY: read_byte should return single byte or None on EOF
/// WHAT: Returns Option<u8>
fn test_read_byte() {
    let mut cursor = Cursor::new(b"hello");

    assert_eq!(cursor.read_byte().unwrap(), Some(b'h'));
    assert_eq!(cursor.read_byte().unwrap(), Some(b'e'));

    // Read remaining
    cursor.read_byte().unwrap();
    cursor.read_byte().unwrap();
    cursor.read_byte().unwrap();

    // EOF
    assert_eq!(cursor.read_byte().unwrap(), None);
}

#[test]
/// WHY: read_into_bytes should handle empty buffer
/// WHAT: Returns 0 when no data available
fn test_read_into_bytes_empty() {
    let mut cursor = Cursor::new(b"");
    let mut buf = BytesMut::with_capacity(1024);

    let n = cursor.read_into_bytes(&mut buf, 5).unwrap();
    assert_eq!(n, 0);
    assert!(buf.is_empty());
}

#[test]
/// WHY: read_exact_into_bytes should error on insufficient data
/// WHAT: Returns UnexpectedEof
fn test_read_exact_into_bytes_eof() {
    let mut cursor = Cursor::new(b"hi");

    let result = cursor.read_exact_into_bytes(5);
    assert!(result.is_err());
}

#[test]
/// WHY: read_pooled_buffer should return pooled buffer
/// WHAT: Buffer is automatically returned to pool on drop
fn test_read_pooled_buffer() {
    let pool = Arc::new(BytesPool::new(1024, 1));
    let mut cursor = Cursor::new(b"hello world");

    let result = cursor.read_pooled_buffer(&pool, 1024).unwrap();
    assert!(result.is_some());

    let buf = result.unwrap();
    assert_eq!(&buf[..], b"hello world");
}

#[test]
/// WHY: read_pooled_buffer should return None on EOF
/// WHAT: Empty stream returns None
fn test_read_pooled_buffer_eof() {
    let pool = Arc::new(BytesPool::new(1024, 1));
    let mut cursor = Cursor::new(b"");

    let result = cursor.read_pooled_buffer(&pool, 1024).unwrap();
    assert!(result.is_none());
}

#[test]
/// WHY: PooledBuffer should return to pool on drop
/// WHAT: Pool hit count should increase after drop
fn test_pooled_buffer_return() {
    let pool = Arc::new(BytesPool::new(1024, 1));

    // First acquire (hit from pre-allocated)
    let _buf1 = pool.acquire();
    let stats1 = pool.stats();
    assert_eq!(stats1.pool_hits, 1);

    // Drop buf1 (returns to pool)
    drop(_buf1);

    // Second acquire (should hit pooled buffer)
    let _buf2 = pool.acquire();
    let stats2 = pool.stats();
    assert_eq!(stats2.pool_hits, 2);
}
