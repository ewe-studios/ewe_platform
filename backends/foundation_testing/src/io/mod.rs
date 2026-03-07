//! Thread-safe shared buffer for producer-consumer test patterns.
//!
//! This module provides a convenient shared buffer abstraction for testing
//! scenarios where one thread writes data and another reads it.
//!
//! # Examples
//!
//! ```rust
//! use foundation_testing::io::SharedBuffer;
//! use std::io::Write;
//!
//! // Split into writer and reader handles
//! let (mut writer, reader) = SharedBuffer::split();
//!
//! // Write from producer thread
//! writer.write_all(b"hello").unwrap();
//!
//! // Read from consumer thread
//! let mut buf = [0u8; 5];
//! reader.read_exact(&mut buf).unwrap();
//! assert_eq!(&buf, b"hello");
//! ```

use std::io::{Read, Result, Write};
use std::sync::{Arc, Mutex};

use foundation_core::io::ioutils::SharedBufferReadStream;

/// Writer handle for `SharedBuffer`.
///
/// Holds a clone of the `Arc<Mutex<Vec<u8>>>` and implements `Write`.
///
/// # Thread Safety
/// This type is `Send + Sync` and can be safely shared across threads.
#[derive(Clone)]
pub struct SharedBufferWriter {
    inner: Arc<Mutex<Vec<u8>>>,
}

impl Write for SharedBufferWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut guard = self.inner.lock().expect("SharedBufferWriter lock poisoned");
        guard.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl SharedBufferWriter {
    /// Create a new writer from a shared Arc<Mutex<Vec<u8>>>.
    pub fn new(inner: Arc<Mutex<Vec<u8>>>) -> Self {
        Self { inner }
    }

    /// Get a clone of the underlying Arc.
    pub fn clone_arc(&self) -> Arc<Mutex<Vec<u8>>> {
        Arc::clone(&self.inner)
    }
}

/// Reader handle for `SharedBuffer`.
///
/// Holds a clone of the `Arc<Mutex<Vec<u8>>>` and implements `Read`.
/// Tracks read position independently from writers.
///
/// # Thread Safety
/// This type is `Send + Sync` and can be safely shared across threads.
#[derive(Clone)]
pub struct SharedBufferReader {
    inner: Arc<Mutex<Vec<u8>>>,
    position: usize,
}

impl Read for SharedBufferReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let guard = self.inner.lock().expect("SharedBufferReader lock poisoned");
        let available = guard.len().saturating_sub(self.position);
        if available == 0 {
            return Ok(0);
        }
        let to_read = buf.len().min(available);
        buf[..to_read].copy_from_slice(&guard[self.position..self.position + to_read]);
        drop(guard);
        self.position += to_read;
        Ok(to_read)
    }
}

impl SharedBufferReader {
    /// Create a new reader from a shared Arc<Mutex<Vec<u8>>>.
    pub fn new(inner: Arc<Mutex<Vec<u8>>>) -> Self {
        Self { inner, position: 0 }
    }

    /// Create a new reader with a specific starting position.
    pub fn with_position(inner: Arc<Mutex<Vec<u8>>>, position: usize) -> Self {
        Self { inner, position }
    }

    /// Get the current read position.
    pub fn position(&self) -> usize {
        self.position
    }

    /// Convert this reader into a `foundation_core::io::ioutils::SharedByteBufferStream`.
    pub fn into_buffered_stream(
        self,
    ) -> foundation_core::io::ioutils::SharedByteBufferStream<
        foundation_core::io::ioutils::SharedBufferReadStream,
    > {
        foundation_core::io::ioutils::SharedByteBufferStream::rwrite(SharedBufferReadStream::new(
            Arc::clone(&self.inner),
            self.position,
        ))
    }
}

/// A thread-safe shared buffer for producer-consumer patterns.
///
/// This type provides a convenient way to create a shared buffer with
/// separate writer and reader handles. The buffer is wrapped in
/// `Arc<Mutex<>>` for thread-safe access.
///
/// # Examples
///
/// ```rust
/// use foundation_testing::io::SharedBuffer;
/// use std::io::Write;
///
/// // Create writer and reader handles
/// let (mut writer, reader) = SharedBuffer::split();
///
/// // Write data
/// writer.write_all(b"hello").unwrap();
///
/// // Read data
/// let mut buf = [0u8; 5];
/// reader.read_exact(&mut buf).unwrap();
/// assert_eq!(&buf, b"hello");
/// ```
pub struct SharedBuffer {
    inner: Arc<Mutex<Vec<u8>>>,
}

impl SharedBuffer {
    /// Create a new SharedBuffer with separate writer and reader handles.
    ///
    /// # Returns
    /// A tuple of `(SharedBufferWriter, SharedBufferReader)` that share
    /// the same underlying buffer.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use foundation_testing::io::SharedBuffer;
    /// use std::io::Write;
    ///
    /// let (mut writer, mut reader) = SharedBuffer::split();
    /// writer.write_all(b"hello").unwrap();
    /// let mut buf = [0u8; 5];
    /// reader.read_exact(&mut buf).unwrap();
    /// assert_eq!(&buf, b"hello");
    /// ```
    pub fn split() -> (SharedBufferWriter, SharedBufferReader) {
        let inner = Arc::new(Mutex::new(Vec::new()));
        let writer = SharedBufferWriter::new(Arc::clone(&inner));
        let reader = SharedBufferReader::new(inner);
        (writer, reader)
    }

    /// Create a new SharedBuffer with initial content.
    ///
    /// # Arguments
    /// * `initial` - Initial bytes to populate the buffer with
    ///
    /// # Returns
    /// A `SharedBuffer` containing the initial data.
    pub fn with_initial(initial: &[u8]) -> Self {
        let inner = Arc::new(Mutex::new(initial.to_vec()));
        Self { inner }
    }

    /// Get the current length of the buffer.
    ///
    /// # Returns
    /// The number of bytes currently in the buffer.
    pub fn len(&self) -> usize {
        let guard = self.inner.lock().expect("SharedBuffer lock poisoned");
        guard.len()
    }

    /// Check if the buffer is empty.
    ///
    /// # Returns
    /// `true` if the buffer contains no bytes, `false` otherwise.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for SharedBuffer {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

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
            SharedBufferWriter::new(buffer.inner.clone()),
            SharedBufferReader::new(buffer.inner),
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
        assert_eq!(writer.inner.lock().unwrap().len(), 0);

        writer.write_all(b"test").unwrap();
        assert_eq!(writer.inner.lock().unwrap().len(), 4);
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
        use std::io::Read;

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
        use std::io::Read;

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
        use std::io::Read;

        let (_, reader) = SharedBuffer::split();
        let mut stream = reader.into_buffered_stream();

        let mut buf = [0u8; 5];
        let bytes_read = stream.read(&mut buf).unwrap();

        assert_eq!(bytes_read, 0);
    }

    #[test]
    fn test_into_buffered_stream_partial_reads() {
        use std::io::Read;

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
}
