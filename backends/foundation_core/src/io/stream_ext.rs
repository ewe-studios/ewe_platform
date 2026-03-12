//! Extension methods for reading into `bytes::BytesMut` and pooled buffers.
//!
//! # WHY
//!
//! WebSocket frame processing requires efficient buffer management:
//! - Zero-copy reads when caller owns the buffer
//! - Pool-based reads for automatic buffer reuse
//! - Exact-length reads for fixed-size frame headers
//!
//! # WHAT
//!
//! Provides extension traits for:
//! - `ReadBytesExt` - Extension for `Read` types to read into `BytesMut`
//! - Helper functions for pooled buffer reads
//!
//! # HOW
//!
//! Wraps standard `Read` trait methods with `bytes::BytesMut` integration.

use bytes::BytesMut;
use std::io::Read;
use std::sync::Arc;

use crate::io::buffer_pool::{BytesPool, PooledBuffer};

/// Extension trait for `Read` types providing bytes-aware read methods.
///
/// # Examples
///
/// ```no_run
/// use bytes::BytesMut;
/// use foundation_core::io::stream_ext::ReadBytesExt;
/// use std::io::Cursor;
///
/// let mut cursor = Cursor::new(b"hello world");
/// let mut buf = BytesMut::with_capacity(1024);
/// let n = cursor.read_into_bytes(&mut buf, 5).unwrap();
/// ```
pub trait ReadBytesExt: Read {
    /// Read into a user-supplied `BytesMut`.
    ///
    /// # WHY
    ///
    /// Allows zero-copy reads when caller owns the buffer. No intermediate
    /// allocation - data goes directly into caller's buffer.
    ///
    /// # Arguments
    ///
    /// * `buf` - Buffer to read into (must have capacity)
    /// * `max_len` - Maximum bytes to read
    ///
    /// # Returns
    ///
    /// Number of bytes read (0 = EOF, >0 = data available)
    ///
    /// # Errors
    ///
    /// Returns `io::Error` if reading fails.
    fn read_into_bytes(&mut self, buf: &mut BytesMut, max_len: usize) -> std::io::Result<usize>;

    /// Read exactly `len` bytes into a `BytesMut`.
    ///
    /// # WHY
    ///
    /// WebSocket frame headers have fixed size (14 bytes max). This method
    /// reads exactly the required bytes, simplifying header parsing.
    ///
    /// # Arguments
    ///
    /// * `len` - Exact number of bytes to read
    ///
    /// # Returns
    ///
    /// `BytesMut` containing exactly `len` bytes.
    ///
    /// # Errors
    ///
    /// Returns `io::Error` if reading fails or EOF is reached before `len` bytes.
    fn read_exact_into_bytes(&mut self, len: usize) -> std::io::Result<BytesMut>;

    /// Read a single byte.
    ///
    /// # Returns
    ///
    /// `Ok(Some(u8))` on success, `Ok(None)` on EOF, `Err` on error.
    fn read_byte(&mut self) -> std::io::Result<Option<u8>>;

    /// Read into a pooled buffer.
    ///
    /// # WHY
    ///
    /// Automatically manages buffer lifecycle. Buffer is returned to pool
    /// on drop, preventing leaks and enabling reuse.
    ///
    /// # Arguments
    ///
    /// * `pool` - Pool to acquire buffer from
    /// * `max_len` - Maximum bytes to read
    ///
    /// # Returns
    ///
    /// `Some(PooledBuffer)` with data, or `None` on EOF.
    ///
    /// # Errors
    ///
    /// Returns `io::Error` if reading fails.
    fn read_pooled_buffer(
        &mut self,
        pool: &Arc<BytesPool>,
        max_len: usize,
    ) -> std::io::Result<Option<PooledBuffer>>;
}

impl<R: Read> ReadBytesExt for R {
    fn read_into_bytes(&mut self, buf: &mut BytesMut, max_len: usize) -> std::io::Result<usize> {
        let available = buf.spare_capacity_mut();
        let to_read = max_len.min(available.len());

        if to_read == 0 {
            return Ok(0);
        }

        let slice = &mut available[..to_read];
        let ptr = slice.as_mut_ptr() as *mut u8;

        let n = unsafe { Read::read(self, std::slice::from_raw_parts_mut(ptr, to_read))? };

        unsafe {
            buf.set_len(buf.len() + n);
        }
        Ok(n)
    }

    fn read_exact_into_bytes(&mut self, len: usize) -> std::io::Result<BytesMut> {
        let mut buf = BytesMut::with_capacity(len);
        buf.resize(len, 0);
        self.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn read_byte(&mut self) -> std::io::Result<Option<u8>> {
        let mut byte = [0u8; 1];
        match self.read(&mut byte)? {
            0 => Ok(None),
            _ => Ok(Some(byte[0])),
        }
    }

    fn read_pooled_buffer(
        &mut self,
        pool: &Arc<BytesPool>,
        max_len: usize,
    ) -> std::io::Result<Option<PooledBuffer>> {
        let mut buf = pool.acquire_with_capacity(max_len);
        buf.resize(max_len, 0);

        let n = self.read(&mut buf)?;

        if n == 0 {
            return Ok(None);
        }

        buf.truncate(n);
        Ok(Some(buf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

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
}
