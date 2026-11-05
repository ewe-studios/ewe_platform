//! Thread-safe shared buffer for producer-consumer test patterns.
//!
//! This module provides a convenient shared buffer abstraction for testing
//! scenarios where one thread writes data and another reads it.
//!
//! # Examples
//!
//! ```rust
//! use foundation_testing::io::SharedBuffer;
//! use std::io::{Read, Write};
//!
//! // Split into writer and reader handles
//! let (mut writer, mut reader) = SharedBuffer::split();
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
/// use std::io::{Read, Write};
///
/// // Create writer and reader handles
/// let (mut writer, mut reader) = SharedBuffer::split();
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
    /// use std::io::{Read, Write};
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

    /// Consume the buffer and return writer and reader handles over its
    /// content.
    ///
    /// # Returns
    /// A `(SharedBufferWriter, SharedBufferReader)` pair that shares the
    /// buffer, with the reader at position 0.
    #[must_use]
    pub fn into_parts(self) -> (SharedBufferWriter, SharedBufferReader) {
        (
            SharedBufferWriter::new(Arc::clone(&self.inner)),
            SharedBufferReader::new(self.inner),
        )
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
