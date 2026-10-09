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
    ///
    /// # Errors
    ///
    /// Returns an error if reading from the underlying stream fails.
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
        let ptr = slice.as_mut_ptr().cast();

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
